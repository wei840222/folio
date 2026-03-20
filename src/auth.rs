use std::collections::HashSet;
use std::sync::Arc;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use rocket::State;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AccessIdentity {
    pub sub: String,
    pub email: Option<String>,
    pub groups: Vec<String>,
}

#[derive(Debug)]
pub struct AccessAuth {
    issuer: String,
    audience: String,
    jwks_url: String,
    allowed_emails: HashSet<String>,
    allowed_groups: HashSet<String>,
}

impl AccessAuth {
    pub fn from_env() -> Self {
        let issuer = std::env::var("FOLIO_CF_ACCESS_ISSUER")
            .unwrap_or_else(|_| "https://example.cloudflareaccess.com".to_string());
        let audience = std::env::var("FOLIO_CF_ACCESS_AUD").unwrap_or_else(|_| "".to_string());
        let jwks_url = std::env::var("FOLIO_CF_ACCESS_JWKS_URL")
            .unwrap_or_else(|_| format!("{}/cdn-cgi/access/certs", issuer.trim_end_matches('/')));

        let allowed_emails = split_csv_env("FOLIO_CF_ACCESS_ALLOWED_EMAILS");
        let allowed_groups = split_csv_env("FOLIO_CF_ACCESS_ALLOWED_GROUPS");

        Self {
            issuer,
            audience,
            jwks_url,
            allowed_emails,
            allowed_groups,
        }
    }

    pub fn verify_and_authorize(&self, jwt: &str) -> Result<AccessIdentity, String> {
        if self.audience.is_empty() {
            return Err("Cloudflare Access audience is not configured".to_string());
        }

        let header = decode_header(jwt).map_err(|e| format!("invalid JWT header: {}", e))?;
        let kid = header.kid.clone();

        let jwks = fetch_jwks(&self.jwks_url)?;
        let key = select_key(&jwks.keys, kid.as_deref())?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[self.issuer.clone()]);
        validation.set_audience(&[self.audience.clone()]);

        let token_data = decode::<AccessClaims>(
            jwt,
            &DecodingKey::from_rsa_components(&key.n, &key.e)
                .map_err(|e| format!("invalid jwk rsa key: {}", e))?,
            &validation,
        )
        .map_err(|e| format!("jwt verification failed: {}", e))?;

        let claims = token_data.claims;
        let identity = AccessIdentity {
            sub: claims.sub,
            email: claims.email,
            groups: claims.groups.unwrap_or_default(),
        };

        if !self.allowed_emails.is_empty() {
            let email = identity.email.clone().unwrap_or_default();
            if !self.allowed_emails.contains(email.as_str()) {
                return Err("email is not authorized".to_string());
            }
        }

        if !self.allowed_groups.is_empty()
            && !identity
                .groups
                .iter()
                .any(|group| self.allowed_groups.contains(group))
        {
            return Err("group is not authorized".to_string());
        }

        Ok(identity)
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

        let token = match request.headers().get_one("Cf-Access-Jwt-Assertion") {
            Some(token) => token,
            None => {
                return Outcome::Error((
                    Status::Unauthorized,
                    "missing Cf-Access-Jwt-Assertion".to_string(),
                ));
            }
        };

        match auth.verify_and_authorize(token) {
            Ok(identity) => Outcome::Success(VerifiedIdentity(identity)),
            Err(err) => Outcome::Error((Status::Unauthorized, err)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AccessClaims {
    sub: String,
    email: Option<String>,
    groups: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
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

fn split_csv_env(key: &str) -> HashSet<String> {
    std::env::var(key)
        .ok()
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default()
}
