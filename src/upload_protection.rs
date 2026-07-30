use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::HttpRequest;
use rand::RngExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use super::error::FolioError;

const PASS_COOKIE_NAME: &str = "folio-upload-pass";
const TURNSTILE_HEADER: &str = "x-turnstile-token";
const MAX_TRACKED_CLIENTS: usize = 10_000;
const MAX_STORED_PASSES: usize = 10_000;
const MAX_REQUESTS_PER_CLIENT: usize = 100;

struct RequestState {
    clients: HashMap<String, VecDeque<Instant>>,
    next_cleanup: Instant,
}

struct PassState {
    entries: HashMap<[u8; 32], Instant>,
    next_cleanup: Instant,
}

pub struct UploadAdmission {
    _permit: OwnedSemaphorePermit,
    pub pass_cookie: Option<String>,
    pub pass_ttl: Duration,
}

pub struct UploadProtection {
    settings: UploadProtectionSettings,
    requests: Mutex<RequestState>,
    passes: Mutex<PassState>,
    max_tracked_clients: usize,
    max_stored_passes: usize,
    semaphore: Arc<Semaphore>,
    client: reqwest::Client,
}

#[derive(Clone)]
struct UploadProtectionSettings {
    soft_limit: usize,
    hard_limit: usize,
    window: Duration,
    pass_ttl: Duration,
    min_free_disk_bytes: u64,
    trust_cf_connecting_ip: bool,
    upload_token_hash: Option<[u8; 32]>,
    turnstile_site_key: Option<String>,
    turnstile_secret: Option<String>,
    turnstile_hostname: Option<String>,
    siteverify_url: String,
}

impl UploadProtection {
    pub fn from_env() -> Result<Self, String> {
        let (soft_limit, hard_limit) = validate_rate_limits(
            env_usize("FOLIO_UPLOAD_RATE_SOFT_LIMIT", 5)?,
            env_usize("FOLIO_UPLOAD_RATE_HARD_LIMIT", 20)?,
        )?;
        let (turnstile_site_key, turnstile_secret, turnstile_hostname) = validate_turnstile_config(
            nonempty_env("FOLIO_TURNSTILE_SITE_KEY")?,
            nonempty_env("FOLIO_TURNSTILE_SECRET")?,
            nonempty_env("FOLIO_TURNSTILE_HOSTNAME")?,
        )?;
        let window = validated_duration(
            "FOLIO_UPLOAD_RATE_WINDOW_SECS",
            env_u64("FOLIO_UPLOAD_RATE_WINDOW_SECS", 60)?,
        )?;
        let pass_ttl = validated_duration(
            "FOLIO_TURNSTILE_PASS_TTL_SECS",
            env_u64("FOLIO_TURNSTILE_PASS_TTL_SECS", 600)?,
        )?;
        let siteverify_url = env_string(
            "FOLIO_TURNSTILE_SITEVERIFY_URL",
            "https://challenges.cloudflare.com/turnstile/v0/siteverify",
        )?;
        reqwest::Url::parse(&siteverify_url).map_err(|error| {
            format!("FOLIO_TURNSTILE_SITEVERIFY_URL is not a valid URL: {error}")
        })?;
        let settings = UploadProtectionSettings {
            soft_limit,
            hard_limit,
            window,
            pass_ttl,
            min_free_disk_bytes: env_u64("FOLIO_MIN_FREE_DISK_BYTES", 1024 * 1024 * 1024)?,
            trust_cf_connecting_ip: env_bool("FOLIO_TRUST_CF_CONNECTING_IP", false)?,
            upload_token_hash: nonempty_env("FOLIO_UPLOAD_TOKEN")?.map(|value| hash_token(&value)),
            turnstile_site_key,
            turnstile_secret,
            turnstile_hostname,
            siteverify_url,
        };
        let max_concurrent =
            validate_max_concurrent(env_usize("FOLIO_MAX_CONCURRENT_UPLOADS", 4)?)?;
        Ok(Self::new(settings, max_concurrent))
    }

    #[cfg(test)]
    pub fn disabled_for_tests() -> Self {
        Self::new(
            UploadProtectionSettings {
                soft_limit: usize::MAX,
                hard_limit: usize::MAX,
                window: Duration::from_secs(60),
                pass_ttl: Duration::from_secs(600),
                min_free_disk_bytes: 0,
                trust_cf_connecting_ip: false,
                upload_token_hash: None,
                turnstile_site_key: None,
                turnstile_secret: None,
                turnstile_hostname: None,
                siteverify_url: "http://127.0.0.1:9".to_string(),
            },
            4,
        )
    }

    fn new(settings: UploadProtectionSettings, max_concurrent: usize) -> Self {
        Self::new_with_state_limits(
            settings,
            max_concurrent,
            MAX_TRACKED_CLIENTS,
            MAX_STORED_PASSES,
        )
    }

    fn new_with_state_limits(
        settings: UploadProtectionSettings,
        max_concurrent: usize,
        max_tracked_clients: usize,
        max_stored_passes: usize,
    ) -> Self {
        let now = Instant::now();
        Self {
            requests: Mutex::new(RequestState {
                clients: HashMap::new(),
                next_cleanup: now + settings.window,
            }),
            passes: Mutex::new(PassState {
                entries: HashMap::new(),
                next_cleanup: now + settings.pass_ttl,
            }),
            settings,
            max_tracked_clients,
            max_stored_passes,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            client: reqwest::Client::new(),
        }
    }

    pub async fn admit(
        &self,
        request: &HttpRequest,
        uploads_path: &Path,
    ) -> Result<UploadAdmission, FolioError> {
        let client_key = self.client_key(request);
        let request_count = self.record_request(&client_key).await;
        if request_count > self.settings.hard_limit {
            return Err(FolioError::TooManyRequests {
                reason: "upload rate limit exceeded".to_string(),
                code: "rate_limited",
                turnstile_site_key: None,
            });
        }

        let trusted_cli = self.has_valid_upload_token(request);
        let valid_pass = self.has_valid_pass(request).await;
        let mut pass_cookie = None;
        if request_count > self.settings.soft_limit
            && !trusted_cli
            && !valid_pass
            && self.turnstile_enabled()
        {
            if let Some(token) = request
                .headers()
                .get(TURNSTILE_HEADER)
                .and_then(|value| value.to_str().ok())
            {
                self.verify_turnstile(token, request.peer_addr().map(|addr| addr.ip()))
                    .await?;
                let pass = generate_token(32);
                self.store_pass(hash_token(&pass), Instant::now() + self.settings.pass_ttl)
                    .await;
                pass_cookie = Some(pass);
            } else {
                return Err(FolioError::TooManyRequests {
                    reason: "additional upload verification is required".to_string(),
                    code: "challenge_required",
                    turnstile_site_key: self.settings.turnstile_site_key.clone(),
                });
            }
        }

        self.check_disk_space(uploads_path)?;
        let permit = self.semaphore.clone().try_acquire_owned().map_err(|_| {
            FolioError::TooManyRequests {
                reason: "too many uploads are currently in progress".to_string(),
                code: "upload_busy",
                turnstile_site_key: None,
            }
        })?;

        Ok(UploadAdmission {
            _permit: permit,
            pass_cookie,
            pass_ttl: self.settings.pass_ttl,
        })
    }

    fn turnstile_enabled(&self) -> bool {
        self.settings.turnstile_site_key.is_some()
            && self.settings.turnstile_secret.is_some()
            && self.settings.turnstile_hostname.is_some()
    }

    async fn record_request(&self, client_key: &str) -> usize {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.settings.window).unwrap_or(now);
        let mut state = self.requests.lock().await;
        if now >= state.next_cleanup {
            state.clients.retain(|_, timestamps| {
                while timestamps
                    .front()
                    .is_some_and(|timestamp| *timestamp < cutoff)
                {
                    timestamps.pop_front();
                }
                !timestamps.is_empty()
            });
            state.next_cleanup = now + self.settings.window;
        }

        if !state.clients.contains_key(client_key)
            && state.clients.len() >= self.max_tracked_clients
        {
            return self.settings.hard_limit.saturating_add(1);
        }

        let timestamps = state.clients.entry(client_key.to_string()).or_default();
        while timestamps
            .front()
            .is_some_and(|timestamp| *timestamp < cutoff)
        {
            timestamps.pop_front();
        }
        let max_timestamps = self
            .settings
            .hard_limit
            .min(MAX_REQUESTS_PER_CLIENT)
            .saturating_add(1);
        if timestamps.len() >= max_timestamps {
            timestamps.pop_front();
        }
        timestamps.push_back(now);
        timestamps.len()
    }

    async fn store_pass(&self, hash: [u8; 32], expires_at: Instant) {
        let now = Instant::now();
        let mut state = self.passes.lock().await;
        if now >= state.next_cleanup {
            state.entries.retain(|_, expiry| *expiry > now);
            state.next_cleanup = now + self.settings.pass_ttl;
        }
        if !state.entries.contains_key(&hash) && state.entries.len() >= self.max_stored_passes {
            let earliest = state
                .entries
                .iter()
                .min_by_key(|(_, expiry)| **expiry)
                .map(|(hash, _)| *hash);
            if let Some(earliest) = earliest {
                state.entries.remove(&earliest);
            }
        }
        state.entries.insert(hash, expires_at);
    }

    fn client_key(&self, request: &HttpRequest) -> String {
        if self.settings.trust_cf_connecting_ip
            && let Some(value) = request
                .headers()
                .get("cf-connecting-ip")
                .and_then(|value| value.to_str().ok())
            && let Ok(ip) = value.parse::<IpAddr>()
        {
            return ip.to_string();
        }
        request
            .peer_addr()
            .map(|address| address.ip().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn has_valid_upload_token(&self, request: &HttpRequest) -> bool {
        let Some(expected) = self.settings.upload_token_hash else {
            return false;
        };
        request
            .headers()
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|token| constant_time_eq(&hash_token(token), &expected))
    }

    async fn has_valid_pass(&self, request: &HttpRequest) -> bool {
        let Some(cookie) = request.cookie(PASS_COOKIE_NAME) else {
            return false;
        };
        let now = Instant::now();
        let hash = hash_token(cookie.value());
        let mut state = self.passes.lock().await;
        if now >= state.next_cleanup {
            state.entries.retain(|_, expires_at| *expires_at > now);
            state.next_cleanup = now + self.settings.pass_ttl;
        }
        state
            .entries
            .get(&hash)
            .is_some_and(|expires_at| *expires_at > now)
    }

    fn check_disk_space(&self, uploads_path: &Path) -> Result<(), FolioError> {
        if self.settings.min_free_disk_bytes == 0 {
            return Ok(());
        }
        let available =
            fs2::available_space(uploads_path).map_err(|error| FolioError::Internal {
                source: error.to_string(),
                context: Some("check upload disk capacity".to_string()),
            })?;
        if available < self.settings.min_free_disk_bytes {
            return Err(FolioError::InsufficientStorage {
                reason: "upload storage is below its configured safety reserve".to_string(),
            });
        }
        Ok(())
    }

    async fn verify_turnstile(
        &self,
        token: &str,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), FolioError> {
        let secret =
            self.settings
                .turnstile_secret
                .as_deref()
                .ok_or_else(|| FolioError::Internal {
                    source: "Turnstile secret is not configured".to_string(),
                    context: None,
                })?;
        let mut form = vec![
            ("secret", secret.to_string()),
            ("response", token.to_string()),
        ];
        if let Some(ip) = remote_ip {
            form.push(("remoteip", ip.to_string()));
        }
        let response = self
            .client
            .post(&self.settings.siteverify_url)
            .timeout(Duration::from_secs(5))
            .form(&form)
            .send()
            .await
            .map_err(|error| FolioError::Forbidden {
                reason: format!("upload verification failed: {}", error),
            })?
            .error_for_status()
            .map_err(|error| FolioError::Forbidden {
                reason: format!("upload verification failed: {}", error),
            })?
            .json::<TurnstileResponse>()
            .await
            .map_err(|error| FolioError::Forbidden {
                reason: format!("invalid upload verification response: {}", error),
            })?;

        let expected_hostname = self.settings.turnstile_hostname.as_deref();
        if !response.success
            || response.action.as_deref() != Some("upload")
            || response.hostname.as_deref() != expected_hostname
        {
            return Err(FolioError::Forbidden {
                reason: "upload verification was rejected".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct TurnstileResponse {
    success: bool,
    hostname: Option<String>,
    action: Option<String>,
}

fn generate_token(size: usize) -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..size)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn read_env(name: &str) -> Result<Option<String>, String> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} contains non-Unicode data")),
    }
}

fn nonempty_env(name: &str) -> Result<Option<String>, String> {
    Ok(read_env(name)?.filter(|value| !value.is_empty()))
}

type TurnstileConfig = (Option<String>, Option<String>, Option<String>);

fn validate_rate_limits(soft_limit: usize, hard_limit: usize) -> Result<(usize, usize), String> {
    if soft_limit > hard_limit {
        return Err(format!(
            "FOLIO_UPLOAD_RATE_SOFT_LIMIT ({soft_limit}) must not exceed FOLIO_UPLOAD_RATE_HARD_LIMIT ({hard_limit})"
        ));
    }
    if hard_limit > MAX_REQUESTS_PER_CLIENT {
        return Err(format!(
            "FOLIO_UPLOAD_RATE_HARD_LIMIT ({hard_limit}) exceeds the maximum supported value ({MAX_REQUESTS_PER_CLIENT})"
        ));
    }
    Ok((soft_limit, hard_limit))
}

fn validate_turnstile_config(
    site_key: Option<String>,
    secret: Option<String>,
    hostname: Option<String>,
) -> Result<TurnstileConfig, String> {
    let configured = usize::from(site_key.is_some())
        + usize::from(secret.is_some())
        + usize::from(hostname.is_some());
    if configured == 0 || configured == 3 {
        Ok((site_key, secret, hostname))
    } else {
        Err(
            "Turnstile requires FOLIO_TURNSTILE_SITE_KEY, FOLIO_TURNSTILE_SECRET, and FOLIO_TURNSTILE_HOSTNAME to be configured together"
                .to_string(),
        )
    }
}

fn parse_env_value<T: std::str::FromStr>(name: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{name} has invalid value {value:?}"))
}

fn env_value<T: std::str::FromStr + Copy>(name: &str, default: T) -> Result<T, String> {
    match read_env(name)? {
        Some(value) => parse_env_value(name, &value),
        None => Ok(default),
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64, String> {
    env_value(name, default)
}

fn env_usize(name: &str, default: usize) -> Result<usize, String> {
    env_value(name, default)
}

fn env_bool(name: &str, default: bool) -> Result<bool, String> {
    env_value(name, default)
}

fn env_string(name: &str, default: &str) -> Result<String, String> {
    Ok(read_env(name)?.unwrap_or_else(|| default.to_string()))
}

fn validated_duration(name: &str, seconds: u64) -> Result<Duration, String> {
    if seconds == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    let duration = Duration::from_secs(seconds);
    if Instant::now().checked_add(duration).is_none() {
        return Err(format!("{name} is too large"));
    }
    Ok(duration)
}

fn validate_max_concurrent(value: usize) -> Result<usize, String> {
    if value == 0 {
        return Err("FOLIO_MAX_CONCURRENT_UPLOADS must be greater than zero".to_string());
    }
    if value > Semaphore::MAX_PERMITS {
        return Err(format!(
            "FOLIO_MAX_CONCURRENT_UPLOADS ({value}) exceeds the maximum supported value ({})",
            Semaphore::MAX_PERMITS
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn settings() -> UploadProtectionSettings {
        UploadProtectionSettings {
            soft_limit: 2,
            hard_limit: 3,
            window: Duration::from_secs(60),
            pass_ttl: Duration::from_secs(600),
            min_free_disk_bytes: 0,
            trust_cf_connecting_ip: false,
            upload_token_hash: None,
            turnstile_site_key: None,
            turnstile_secret: None,
            turnstile_hostname: None,
            siteverify_url: "http://127.0.0.1:9".to_string(),
        }
    }

    #[test]
    fn configuration_parsers_reject_invalid_explicit_values() {
        assert!(parse_env_value::<usize>("FOLIO_UPLOAD_RATE_HARD_LIMIT", "many").is_err());
        assert!(parse_env_value::<u64>("FOLIO_UPLOAD_RATE_WINDOW_SECS", "-1").is_err());
        assert!(parse_env_value::<bool>("FOLIO_TRUST_CF_CONNECTING_IP", "yes").is_err());
    }

    #[test]
    fn runtime_bounds_reject_zero_and_overflowing_values() {
        assert!(validated_duration("FOLIO_UPLOAD_RATE_WINDOW_SECS", 0).is_err());
        assert!(validated_duration("FOLIO_TURNSTILE_PASS_TTL_SECS", u64::MAX).is_err());
        assert!(validate_max_concurrent(0).is_err());
        assert!(validate_max_concurrent(Semaphore::MAX_PERMITS + 1).is_err());
    }

    #[test]
    fn turnstile_configuration_is_all_or_none() {
        assert!(validate_turnstile_config(None, None, None).is_ok());
        assert!(
            validate_turnstile_config(
                Some("site-key".to_string()),
                Some("secret".to_string()),
                Some("folio.example".to_string()),
            )
            .is_ok()
        );

        let partial_configurations = [
            (Some("site-key".to_string()), None, None),
            (None, Some("secret".to_string()), None),
            (None, None, Some("folio.example".to_string())),
            (
                Some("site-key".to_string()),
                Some("secret".to_string()),
                None,
            ),
            (
                Some("site-key".to_string()),
                None,
                Some("folio.example".to_string()),
            ),
            (
                None,
                Some("secret".to_string()),
                Some("folio.example".to_string()),
            ),
        ];
        for (site_key, secret, hostname) in partial_configurations {
            assert!(validate_turnstile_config(site_key, secret, hostname).is_err());
        }
    }

    #[test]
    fn rate_limit_configuration_has_a_fixed_memory_bound() {
        assert_eq!(validate_rate_limits(5, 20), Ok((5, 20)));
        assert!(validate_rate_limits(21, 20).is_err());
        assert!(validate_rate_limits(5, MAX_REQUESTS_PER_CLIENT + 1).is_err());
    }

    #[tokio::test]
    async fn rate_counter_is_bounded_by_window_and_key() {
        let protection = UploadProtection::new(settings(), 1);
        assert_eq!(protection.record_request("one").await, 1);
        assert_eq!(protection.record_request("one").await, 2);
        assert_eq!(protection.record_request("two").await, 1);
    }

    #[tokio::test]
    async fn rate_counter_bounds_each_client_queue() {
        let protection = UploadProtection::new(settings(), 1);
        for _ in 0..20 {
            protection.record_request("one").await;
        }

        let requests = protection.requests.lock().await;
        assert_eq!(requests.clients["one"].len(), 4);
    }

    #[tokio::test]
    async fn rate_counter_fails_closed_when_client_capacity_is_full() {
        let protection = UploadProtection::new_with_state_limits(settings(), 1, 2, 2);
        assert_eq!(protection.record_request("one").await, 1);
        assert_eq!(protection.record_request("two").await, 1);
        assert_eq!(protection.record_request("three").await, 4);

        let requests = protection.requests.lock().await;
        assert_eq!(requests.clients.len(), 2);
        assert!(!requests.clients.contains_key("three"));
    }

    #[tokio::test]
    async fn turnstile_passes_evict_the_earliest_expiry_at_capacity() {
        let protection = UploadProtection::new_with_state_limits(settings(), 1, 2, 2);
        let now = Instant::now();
        protection
            .store_pass([1; 32], now + Duration::from_secs(30))
            .await;
        protection
            .store_pass([2; 32], now + Duration::from_secs(20))
            .await;
        protection
            .store_pass([3; 32], now + Duration::from_secs(40))
            .await;

        let passes = protection.passes.lock().await;
        assert_eq!(passes.entries.len(), 2);
        assert!(!passes.entries.contains_key(&[2; 32]));
        assert!(passes.entries.contains_key(&[1; 32]));
        assert!(passes.entries.contains_key(&[3; 32]));
    }

    #[test]
    fn token_hash_comparison_is_exact() {
        assert!(constant_time_eq(
            &hash_token("secret"),
            &hash_token("secret")
        ));
        assert!(!constant_time_eq(
            &hash_token("secret"),
            &hash_token("other")
        ));
    }

    #[tokio::test]
    async fn soft_limit_requires_configured_challenge() {
        let mut settings = settings();
        settings.soft_limit = 0;
        settings.turnstile_site_key = Some("site-key".to_string());
        settings.turnstile_secret = Some("secret".to_string());
        settings.turnstile_hostname = Some("folio.example".to_string());
        let protection = UploadProtection::new(settings, 1);
        let temp_dir = tempfile::tempdir().unwrap();
        let request = TestRequest::default()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .to_http_request();

        let error = match protection.admit(&request, temp_dir.path()).await {
            Ok(_) => panic!("expected challenge"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            FolioError::TooManyRequests {
                code: "challenge_required",
                turnstile_site_key: Some(ref key),
                ..
            } if key == "site-key"
        ));
    }

    #[tokio::test]
    async fn cli_token_bypasses_soft_limit_but_not_hard_limit() {
        let mut settings = settings();
        settings.soft_limit = 0;
        settings.hard_limit = 1;
        settings.upload_token_hash = Some(hash_token("cli-secret"));
        let protection = UploadProtection::new(settings, 1);
        let temp_dir = tempfile::tempdir().unwrap();
        let request = TestRequest::default()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .insert_header(("Authorization", "Bearer cli-secret"))
            .to_http_request();

        assert!(protection.admit(&request, temp_dir.path()).await.is_ok());
        let error = match protection.admit(&request, temp_dir.path()).await {
            Ok(_) => panic!("expected hard rate limit"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            FolioError::TooManyRequests {
                code: "rate_limited",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn valid_upload_pass_bypasses_soft_challenge() {
        let mut settings = settings();
        settings.soft_limit = 0;
        settings.turnstile_site_key = Some("site-key".to_string());
        settings.turnstile_secret = Some("secret".to_string());
        settings.turnstile_hostname = Some("folio.example".to_string());
        let protection = UploadProtection::new(settings, 1);
        protection
            .store_pass(
                hash_token("valid-pass"),
                Instant::now() + Duration::from_secs(60),
            )
            .await;
        let temp_dir = tempfile::tempdir().unwrap();
        let request = TestRequest::default()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .cookie(actix_web::cookie::Cookie::new(
                PASS_COOKIE_NAME,
                "valid-pass",
            ))
            .to_http_request();

        assert!(protection.admit(&request, temp_dir.path()).await.is_ok());
    }

    #[tokio::test]
    async fn concurrent_limit_fails_before_body_processing() {
        let protection = UploadProtection::new(settings(), 1);
        let _permit = protection.semaphore.clone().try_acquire_owned().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let request = TestRequest::default()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .to_http_request();

        let error = match protection.admit(&request, temp_dir.path()).await {
            Ok(_) => panic!("expected busy response"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            FolioError::TooManyRequests {
                code: "upload_busy",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn disk_reserve_rejects_upload() {
        let mut settings = settings();
        settings.min_free_disk_bytes = u64::MAX;
        let protection = UploadProtection::new(settings, 1);
        let temp_dir = tempfile::tempdir().unwrap();
        let request = TestRequest::default().to_http_request();

        let error = match protection.admit(&request, temp_dir.path()).await {
            Ok(_) => panic!("expected storage rejection"),
            Err(error) => error,
        };
        assert!(matches!(error, FolioError::InsufficientStorage { .. }));
    }

    #[test]
    fn cloudflare_ip_is_used_only_when_trusted() {
        let mut trusted_settings = settings();
        trusted_settings.trust_cf_connecting_ip = true;
        let trusted = UploadProtection::new(trusted_settings, 1);
        let untrusted = UploadProtection::new(settings(), 1);
        let request = TestRequest::default()
            .peer_addr("127.0.0.1:1234".parse().unwrap())
            .insert_header(("CF-Connecting-IP", "203.0.113.8"))
            .to_http_request();

        assert_eq!(trusted.client_key(&request), "203.0.113.8");
        assert_eq!(untrusted.client_key(&request), "127.0.0.1");
    }

    async fn siteverify_server(response_body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0u8; 4096];
            let _ = stream.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn siteverify_accepts_only_expected_hostname_and_action() {
        let mut settings = settings();
        settings.turnstile_secret = Some("secret".to_string());
        settings.turnstile_hostname = Some("folio.example".to_string());
        settings.siteverify_url =
            siteverify_server(r#"{"success":true,"hostname":"folio.example","action":"upload"}"#)
                .await;
        let protection = UploadProtection::new(settings, 1);

        assert!(protection.verify_turnstile("token", None).await.is_ok());
    }

    #[tokio::test]
    async fn siteverify_rejects_wrong_action() {
        let mut settings = settings();
        settings.turnstile_secret = Some("secret".to_string());
        settings.turnstile_hostname = Some("folio.example".to_string());
        settings.siteverify_url =
            siteverify_server(r#"{"success":true,"hostname":"folio.example","action":"other"}"#)
                .await;
        let protection = UploadProtection::new(settings, 1);

        assert!(matches!(
            protection.verify_turnstile("token", None).await,
            Err(FolioError::Forbidden { .. })
        ));
    }
}
