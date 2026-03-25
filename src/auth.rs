use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jsonwebtoken::{
    Algorithm, DecodingKey, Validation, decode, decode_header, errors::ErrorKind as JwtErrorKind,
};
use rocket::State;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AccessIdentity {
    pub sub: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
enum VerifyMode {
    Rs256Jwks { jwks_url: String },
    Hs256 { secret: String },
}

#[derive(Debug)]
pub struct AccessAuth {
    issuer: String,
    audience: String,
    verify_mode: VerifyMode,
    jwks_cache: Mutex<Option<(JwkSet, Instant)>>,
}

#[derive(Debug)]
pub enum AccessAuthError {
    Unauthorized { code: &'static str, message: String },
    Internal { code: &'static str, message: String },
}

impl AccessAuthError {
    fn unauthorized(code: &'static str, message: impl Into<String>) -> Self {
        Self::Unauthorized {
            code,
            message: message.into(),
        }
    }

    fn internal(code: &'static str, message: impl Into<String>) -> Self {
        Self::Internal {
            code,
            message: message.into(),
        }
    }

    pub fn status(&self) -> Status {
        match self {
            Self::Unauthorized { .. } => Status::Unauthorized,
            Self::Internal { .. } => Status::InternalServerError,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized { code, .. }
            | Self::Internal { code, .. } => code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::Unauthorized { message, .. }
            | Self::Internal { message, .. } => message,
        }
    }
}

impl AccessAuth {
    pub fn from_env() -> Self {
        let issuer = std::env::var("FOLIO_CF_ACCESS_ISSUER")
            .unwrap_or_else(|_| "https://example.cloudflareaccess.com".to_string());
        let audience = std::env::var("FOLIO_CF_ACCESS_AUD").unwrap_or_else(|_| "".to_string());

        let verify_mode = if let Ok(secret) = std::env::var("FOLIO_CF_ACCESS_HS256_SECRET") {
            VerifyMode::Hs256 { secret }
        } else {
            let jwks_url = std::env::var("FOLIO_CF_ACCESS_JWKS_URL").unwrap_or_else(|_| {
                format!("{}/cdn-cgi/access/certs", issuer.trim_end_matches('/'))
            });
            VerifyMode::Rs256Jwks { jwks_url }
        };

        Self {
            issuer,
            audience,
            verify_mode,
            jwks_cache: Mutex::new(None),
        }
    }

    fn get_jwks(&self, url: &str) -> Result<JwkSet, String> {
        let mut cache = self
            .jwks_cache
            .lock()
            .map_err(|_| "jwks cache lock poisoned".to_string())?;

        if let Some((jwks, timestamp)) = &*cache {
            if timestamp.elapsed() < Duration::from_secs(3600) {
                return Ok(jwks.clone());
            }
        }

        log::info!("fetching fresh jwks from {}", url);
        let jwks = fetch_jwks(url)?;
        *cache = Some((jwks.clone(), Instant::now()));
        Ok(jwks)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        issuer: &str,
        audience: &str,
        hs256_secret: Option<&str>,
    ) -> Self {
        let verify_mode = if let Some(secret) = hs256_secret {
            VerifyMode::Hs256 {
                secret: secret.to_string(),
            }
        } else {
            VerifyMode::Rs256Jwks {
                jwks_url: "https://example.cloudflareaccess.com/cdn-cgi/access/certs".to_string(),
            }
        };

        Self {
            issuer: issuer.to_string(),
            audience: audience.to_string(),
            verify_mode,
            jwks_cache: Mutex::new(None),
        }
    }

    pub fn verify_and_authorize(&self, jwt: &str) -> Result<AccessIdentity, AccessAuthError> {
        if self.audience.is_empty() {
            return Err(AccessAuthError::internal(
                "audience_not_configured",
                "Cloudflare Access audience is not configured",
            ));
        }

        let claims = self.verify_claims(jwt)?;

        let identity = AccessIdentity {
            sub: claims.sub,
            email: claims.email,
        };

        Ok(identity)
    }

    fn verify_claims(&self, jwt: &str) -> Result<AccessClaims, AccessAuthError> {
        match &self.verify_mode {
            VerifyMode::Hs256 { secret } => {
                let mut validation = Validation::new(Algorithm::HS256);
                validation.set_issuer(&[self.issuer.clone()]);
                validation.set_audience(&[self.audience.clone()]);

                decode::<AccessClaims>(
                    jwt,
                    &DecodingKey::from_secret(secret.as_bytes()),
                    &validation,
                )
                .map(|token| token.claims)
                .map_err(map_jwt_error)
            }
            VerifyMode::Rs256Jwks { jwks_url } => {
                let header = decode_header(jwt)
                    .map_err(|e| map_jwt_error_with_context("invalid_header", e))?;
                let kid = header.kid.clone();

                let jwks = self.get_jwks(jwks_url).map_err(|e| {
                    AccessAuthError::unauthorized("jwks_fetch_failed", format!("{}", e))
                })?;
                let key = select_key(&jwks.keys, kid.as_deref()).map_err(|e| {
                    AccessAuthError::unauthorized("jwk_selection_failed", format!("{}", e))
                })?;

                let mut validation = Validation::new(Algorithm::RS256);
                validation.set_issuer(&[self.issuer.clone()]);
                validation.set_audience(&[self.audience.clone()]);

                let decoding_key =
                    DecodingKey::from_rsa_components(&key.n, &key.e).map_err(|e| {
                        AccessAuthError::unauthorized(
                            "jwk_decode_failed",
                            format!("invalid jwk rsa key: {}", e),
                        )
                    })?;

                decode::<AccessClaims>(jwt, &decoding_key, &validation)
                    .map(|token| token.claims)
                    .map_err(map_jwt_error)
            }
        }
    }
}

pub struct VerifiedIdentity(pub AccessIdentity);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for VerifiedIdentity {
    type Error = String;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let auth = match request.guard::<&State<Arc<AccessAuth>>>().await {
            Outcome::Success(state) => state,
            Outcome::Error(_) | Outcome::Forward(_) => {
                return Outcome::Error((
                    Status::InternalServerError,
                    "auth state unavailable".to_string(),
                ));
            }
        };

        let token = request
            .headers()
            .get_one("Cf-Access-Jwt-Assertion")
            .or_else(|| {
                request
                    .headers()
                    .get_one("Authorization")
                    .and_then(|h| h.strip_prefix("Bearer ").or(h.strip_prefix("bearer ")))
            });

        let token = match token {
            Some(token) => token,
            None => {
                log::warn!(
                    "private auth deny: code=missing_token status=401 path={} method={}",
                    request.uri(),
                    request.method()
                );
                return Outcome::Error((
                    Status::Unauthorized,
                    "missing authorization header (Cf-Access-Jwt-Assertion or bearer_token)"
                        .to_string(),
                ));
            }
        };

        match auth.verify_and_authorize(token) {
            Ok(identity) => Outcome::Success(VerifiedIdentity(identity)),
            Err(err) => {
                log::warn!(
                    "private auth deny: code={} status={} path={} method={}",
                    err.code(),
                    err.status().code,
                    request.uri(),
                    request.method()
                );
                Outcome::Error((err.status(), err.message().to_string()))
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct AccessClaims {
    sub: String,
    email: Option<String>,
    groups: Option<Vec<String>>,
    exp: Option<usize>,
    iss: Option<String>,
    #[serde(deserialize_with = "deserialize_aud")]
    aud: Option<Vec<String>>,
}

fn deserialize_aud<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match Option::<StringOrVec>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrVec::String(s)) => Ok(Some(vec![s])),
        Some(StringOrVec::Vec(v)) => Ok(Some(v)),
    }
}

#[derive(Debug, Deserialize, Clone)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kid: Option<String>,
    n: String,
    e: String,
}

fn fetch_jwks(url: &str) -> Result<JwkSet, String> {
    let response = reqwest::blocking::get(url).map_err(|e| format!("fetch jwks failed: {}", e))?;
    if !response.status().is_success() {
        return Err(format!(
            "fetch jwks failed with status {}",
            response.status()
        ));
    }

    response
        .json::<JwkSet>()
        .map_err(|e| format!("parse jwks failed: {}", e))
}

fn select_key<'a>(keys: &'a [Jwk], kid: Option<&str>) -> Result<&'a Jwk, String> {
    if keys.is_empty() {
        return Err("jwks has no keys".to_string());
    }

    if let Some(target_kid) = kid {
        if let Some(key) = keys
            .iter()
            .find(|key| key.kid.as_deref() == Some(target_kid))
        {
            return Ok(key);
        }
    }

    Ok(&keys[0])
}

fn map_jwt_error(error: jsonwebtoken::errors::Error) -> AccessAuthError {
    map_jwt_error_with_context("jwt_invalid", error)
}

fn map_jwt_error_with_context(
    default_code: &'static str,
    error: jsonwebtoken::errors::Error,
) -> AccessAuthError {
    let code = match error.kind() {
        JwtErrorKind::ExpiredSignature => "jwt_expired",
        JwtErrorKind::InvalidAudience => "jwt_invalid_audience",
        JwtErrorKind::InvalidIssuer => "jwt_invalid_issuer",
        JwtErrorKind::InvalidSignature => "jwt_invalid_signature",
        JwtErrorKind::InvalidToken => "jwt_invalid_token",
        _ => default_code,
    };

    AccessAuthError::unauthorized(code, format!("jwt verification failed: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_utils::make_hs256_token;

    #[test]
    fn verify_hs256_success() {
        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        let token = make_hs256_token(
            secret,
            "user-1",
            Some("allowed@example.com"),
            &["team-a"],
            "https://issuer.example.com",
            "folio-app",
            3600,
        );

        let identity = auth.verify_and_authorize(&token).unwrap();
        assert_eq!(identity.sub, "user-1");
    }

    #[test]
    fn verify_hs256_invalid_signature_returns_401() {
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some("secret-a"),
        );

        let token = make_hs256_token(
            "secret-b",
            "user-1",
            Some("u@example.com"),
            &[],
            "https://issuer.example.com",
            "folio-app",
            3600,
        );

        let err = auth.verify_and_authorize(&token).unwrap_err();
        assert_eq!(err.status(), Status::Unauthorized);
        assert_eq!(err.code(), "jwt_invalid_signature");
    }

    #[test]
    fn verify_hs256_wrong_issuer_returns_401() {
        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        let token = make_hs256_token(
            secret,
            "user-1",
            Some("u@example.com"),
            &[],
            "https://wrong-issuer.example.com",
            "folio-app",
            3600,
        );

        let err = auth.verify_and_authorize(&token).unwrap_err();
        assert_eq!(err.status(), Status::Unauthorized);
        assert_eq!(err.code(), "jwt_invalid_issuer");
    }

    #[test]
    fn verify_hs256_wrong_audience_returns_401() {
        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        let token = make_hs256_token(
            secret,
            "user-1",
            Some("u@example.com"),
            &[],
            "https://issuer.example.com",
            "other-app",
            3600,
        );

        let err = auth.verify_and_authorize(&token).unwrap_err();
        assert_eq!(err.status(), Status::Unauthorized);
        assert_eq!(err.code(), "jwt_invalid_audience");
    }

    #[test]
    fn verify_hs256_expired_returns_401() {
        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        let token = make_hs256_token(
            secret,
            "user-1",
            Some("u@example.com"),
            &[],
            "https://issuer.example.com",
            "folio-app",
            -3600,
        );

        let err = auth.verify_and_authorize(&token).unwrap_err();
        assert_eq!(err.status(), Status::Unauthorized);
        assert_eq!(err.code(), "jwt_expired");
    }

    #[test]
    fn verify_hs256_aud_array_success() {
        use crate::test_utils::make_hs256_token_with_aud_array;

        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        // Cloudflare Access sends aud as array
        let token = make_hs256_token_with_aud_array(
            secret,
            "user-1",
            Some("allowed@example.com"),
            &["team-a"],
            "https://issuer.example.com",
            &["folio-app"],
            3600,
        );

        let identity = auth.verify_and_authorize(&token).unwrap();
        assert_eq!(identity.sub, "user-1");
        assert_eq!(identity.email, Some("allowed@example.com".to_string()));
    }

    #[test]
    fn verify_hs256_aud_array_wrong_audience_returns_401() {
        use crate::test_utils::make_hs256_token_with_aud_array;

        let secret = "test-secret";
        let auth = AccessAuth::from_parts(
            "https://issuer.example.com",
            "folio-app",
            Some(secret),
        );

        let token = make_hs256_token_with_aud_array(
            secret,
            "user-1",
            Some("u@example.com"),
            &[],
            "https://issuer.example.com",
            &["other-app", "another-app"],
            3600,
        );

        let err = auth.verify_and_authorize(&token).unwrap_err();
        assert_eq!(err.status(), Status::Unauthorized);
        assert_eq!(err.code(), "jwt_invalid_audience");
    }
}
