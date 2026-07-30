use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use actix_multipart::{Field, Multipart, MultipartError};
use actix_web::cookie::{Cookie, SameSite};
use actix_web::error::PayloadError;
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse, Responder, post, web};
use futures_util::StreamExt;
use rand::RngExt;
use serde_json::json;
use tokio::io::AsyncWriteExt;

use super::config;
use super::error::FolioError;
use super::expiry::ExpiryStore;
use super::private_index::PrivateIndexStore;
use super::upload_protection::UploadProtection;

const MAX_MULTIPART_PARTS: usize = 2;
const MAX_MULTIPART_OVERHEAD_BYTES: usize = 32 * 1024;
const UPLOAD_BODY_TIMEOUT: Duration = Duration::from_secs(120);

/// A _probably_ unique upload id.
pub struct UploadId(String);

impl UploadId {
    /// Generate a _probably_ unique id with `size` characters. For readability,
    /// the characters used are from the sets [0-9], [A-Z], [a-z]. The
    /// probability of a collision depends on the value of `size` and the number
    /// of ids generated thus far.
    pub fn new(size: usize) -> UploadId {
        const BASE62: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

        let mut rng = rand::rng();
        let id: String = (0..size)
            .map(|_| BASE62[rng.random_range(0..BASE62.len())] as char)
            .collect();

        UploadId(id)
    }

    /// Returns the file name corresponding to this ID.
    pub fn file_name(&self, extension: Option<&str>) -> String {
        extension.map_or_else(|| self.0.clone(), |ext| format!("{}.{}", self.0, ext))
    }
}

#[derive(Default)]
struct UploadParts {
    file_name: Option<String>,
    staged_path: Option<PathBuf>,
    reservation_path: Option<PathBuf>,
    authorized_emails: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UploadQuery {
    expire: Option<String>,
}

pub fn cleanup_staging_dir(config: &config::Folio) -> std::io::Result<()> {
    let staging_dir = config
        .resolve_base(&config.uploads_path)
        .join(config::UPLOAD_STAGING_DIR);
    let metadata = match std::fs::symlink_metadata(&staging_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    if metadata.is_dir() {
        std::fs::remove_dir_all(staging_dir)
    } else {
        std::fs::remove_file(staging_dir)
    }
}

#[post("/uploads")]
pub async fn upload_file(
    request: HttpRequest,
    config: web::Data<config::Folio>,
    expiry_store: web::Data<Arc<ExpiryStore>>,
    private_store: web::Data<Arc<PrivateIndexStore>>,
    protection: web::Data<Arc<UploadProtection>>,
    payload: web::Payload,
    query: web::Query<UploadQuery>,
) -> Result<impl Responder, FolioError> {
    let raw_body_limit = raw_upload_body_limit(&config);
    validate_content_length(&request, raw_body_limit)?;
    let admission = protection
        .admit(&request, &config.resolve_base(&config.uploads_path))
        .await?;
    let ttl = upload_ttl(&config, query.expire.as_deref())
        .map_err(|reason| FolioError::BadRequest { reason })?;

    let payload = raw_limited_multipart(&request, payload, raw_body_limit);
    let mut parts = UploadParts::default();
    if let Err(error) =
        save_upload_payload_with_deadline(payload, &config, &mut parts, UPLOAD_BODY_TIMEOUT).await
    {
        cleanup_upload_artifacts(
            parts.staged_path.as_deref(),
            parts.reservation_path.as_deref(),
        )
        .await;
        return Err(error);
    }
    let file_name = parts
        .file_name
        .take()
        .ok_or_else(|| FolioError::BadRequest {
            reason: "multipart form is missing file field".to_string(),
        })?;
    let staged_path = parts
        .staged_path
        .take()
        .ok_or_else(|| FolioError::Internal {
            source: "uploaded file is missing its staging path".to_string(),
            context: None,
        })?;
    let reservation_path = parts
        .reservation_path
        .take()
        .ok_or_else(|| FolioError::Internal {
            source: "uploaded file is missing its name reservation".to_string(),
            context: None,
        })?;
    let full_path = config.build_full_upload_path(&PathBuf::from(&file_name));
    let mut private_mutation = None;

    if let Some(emails_str) = &parts.authorized_emails {
        let emails: Vec<String> = emails_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if emails.len() > config.max_authorized_emails {
            cleanup_upload_artifacts(Some(&staged_path), Some(&reservation_path)).await;
            return Err(FolioError::BadRequest {
                reason: format!(
                    "authorized email count exceeds maximum of {}",
                    config.max_authorized_emails
                ),
            });
        }

        if !emails.is_empty() {
            match private_store
                .mark_private(&PathBuf::from(&file_name), emails)
                .await
            {
                Ok(mutation) => private_mutation = Some(mutation),
                Err(error) => {
                    cleanup_upload_artifacts(Some(&staged_path), Some(&reservation_path)).await;
                    let message = format!("failed to mark file as private: {}", error);
                    log::error!("POST /uploads error: {}", message);
                    return Err(FolioError::Internal {
                        source: message,
                        context: None,
                    });
                }
            }
        }
    }

    let expires_at = match expiry_store.publish(&staged_path, &full_path, ttl).await {
        Ok(expires_at) => expires_at,
        Err(error) => {
            if let Some(mutation) = private_mutation
                && let Err(cleanup_error) = private_store
                    .restore_private(&PathBuf::from(&file_name), mutation)
                    .await
            {
                log::error!(
                    "failed to roll back private metadata for {}: {}",
                    file_name,
                    cleanup_error
                );
            }
            cleanup_upload_artifacts(Some(&staged_path), Some(&reservation_path)).await;
            let message = format!("failed to publish uploaded file {}: {}", file_name, error);
            log::error!("POST /uploads error: {}", message);
            return Err(FolioError::Internal {
                source: message,
                context: None,
            });
        }
    };
    cleanup_staged_file(Some(&reservation_path)).await;

    let mut response = HttpResponse::build(StatusCode::CREATED);
    response.append_header(("Location", format!("/files/{}", file_name)));
    if let Some(pass) = admission.pass_cookie {
        response.cookie(
            Cookie::build("folio-upload-pass", pass)
                .path("/uploads")
                .secure(true)
                .http_only(true)
                .same_site(SameSite::Strict)
                .max_age(actix_web::cookie::time::Duration::seconds(
                    admission.pass_ttl.as_secs().try_into().unwrap_or(i64::MAX),
                ))
                .finish(),
        );
    }
    Ok(response.json(json!({
        "message": "file uploaded successfully",
        "expires_at": expires_at,
    })))
}

fn raw_upload_body_limit(config: &config::Folio) -> usize {
    config
        .max_upload_size
        .saturating_add(config.max_upload_text_field_size)
        .saturating_add(MAX_MULTIPART_OVERHEAD_BYTES)
}

fn validate_content_length(request: &HttpRequest, max_bytes: usize) -> Result<(), FolioError> {
    let Some(value) = request
        .headers()
        .get(actix_web::http::header::CONTENT_LENGTH)
    else {
        return Ok(());
    };
    let length = value
        .to_str()
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| FolioError::BadRequest {
            reason: "invalid Content-Length header".to_string(),
        })?;
    if length > max_bytes as u64 {
        return Err(FolioError::PayloadTooLarge {
            reason: format!("upload request exceeds maximum of {max_bytes} bytes"),
        });
    }
    Ok(())
}

fn raw_limited_multipart(
    request: &HttpRequest,
    payload: web::Payload,
    max_bytes: usize,
) -> Multipart {
    let mut bytes_received = 0usize;
    let payload = payload.map(move |result| {
        result.and_then(|bytes| {
            bytes_received = bytes_received
                .checked_add(bytes.len())
                .ok_or(PayloadError::Overflow)?;
            if bytes_received > max_bytes {
                return Err(PayloadError::Overflow);
            }
            Ok(bytes)
        })
    });
    Multipart::new(request.headers(), payload)
}

async fn save_upload_payload_with_deadline(
    payload: Multipart,
    config: &config::Folio,
    parts: &mut UploadParts,
    deadline: Duration,
) -> Result<(), FolioError> {
    tokio::time::timeout(deadline, save_upload_payload(payload, config, parts))
        .await
        .map_err(|_| FolioError::RequestTimeout {
            reason: "upload body deadline exceeded".to_string(),
        })?
}

fn map_multipart_error(error: MultipartError, context: &str) -> FolioError {
    if matches!(&error, MultipartError::Payload(PayloadError::Overflow)) {
        FolioError::PayloadTooLarge {
            reason: "multipart request exceeds maximum size".to_string(),
        }
    } else {
        FolioError::BadRequest {
            reason: format!("invalid multipart {context}: {error}"),
        }
    }
}

async fn save_upload_payload(
    mut payload: Multipart,
    config: &config::Folio,
    parts: &mut UploadParts,
) -> Result<(), FolioError> {
    let max_request_size = config
        .max_upload_size
        .saturating_add(config.max_upload_text_field_size);
    let mut request_bytes = 0usize;
    let mut part_count = 0usize;
    while let Some(field) = payload.next().await {
        part_count += 1;
        if part_count > MAX_MULTIPART_PARTS {
            return Err(FolioError::BadRequest {
                reason: format!("multipart form exceeds maximum of {MAX_MULTIPART_PARTS} fields"),
            });
        }
        let mut field = field.map_err(|error| map_multipart_error(error, "payload"))?;

        match field.name() {
            Some("file") => {
                if parts.file_name.is_some() {
                    return Err(FolioError::BadRequest {
                        reason: "multipart form must contain exactly one file field".to_string(),
                    });
                }
                let filename_extension = filename_extension(&field).and_then(normalize_extension);
                let content_type_extension = content_type_extension(&field);
                log::info!(
                    "Upload extension check: content-type-ext={:?}, filename-ext={:?}",
                    content_type_extension,
                    filename_extension
                );
                let extension = match (content_type_extension, filename_extension) {
                    (Some(ext), _) if ext == "bin" => None,
                    (_, Some(ext)) if !ext.is_empty() => Some(ext),
                    (Some(ext), None) => Some(ext),
                    _ => None,
                };

                let (id, reservation_path) =
                    generate_unique_upload_id(config, extension.as_deref())?;
                let file_name = id.file_name(extension.as_deref());
                let staged_name = format!("{}.{}.part", file_name, UploadId::new(16).0);
                let staged_path = config.build_upload_staging_path(Path::new(&staged_name));
                if let Some(parent) = staged_path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| {
                        FolioError::Internal {
                            source: error.to_string(),
                            context: Some("create upload staging directory".to_string()),
                        }
                    })?;
                }
                parts.file_name = Some(file_name);
                parts.staged_path = Some(staged_path.clone());
                parts.reservation_path = Some(reservation_path);
                save_field_to_path(
                    &mut field,
                    &staged_path,
                    config.max_upload_size,
                    &mut request_bytes,
                    max_request_size,
                )
                .await?;
            }
            Some("authorized_emails") => {
                if parts.authorized_emails.is_some() {
                    return Err(FolioError::BadRequest {
                        reason: "multipart form must not repeat authorized_emails".to_string(),
                    });
                }
                parts.authorized_emails = Some(
                    read_text_field(
                        &mut field,
                        config.max_upload_text_field_size,
                        &mut request_bytes,
                        max_request_size,
                    )
                    .await?,
                );
            }
            Some(name) => {
                return Err(FolioError::BadRequest {
                    reason: format!("multipart form contains unknown field {name:?}"),
                });
            }
            None => {
                return Err(FolioError::BadRequest {
                    reason: "multipart field is missing a name".to_string(),
                });
            }
        }
    }

    Ok(())
}

async fn cleanup_staged_file(path: Option<&Path>) {
    let Some(path) = path else {
        return;
    };
    if let Err(error) = tokio::fs::remove_file(path).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        log::error!("failed to clean up upload {}: {}", path.display(), error);
    }
    if let Some(parent) = path.parent()
        && let Err(error) = tokio::fs::remove_dir(parent).await
        && !matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
        )
    {
        log::error!(
            "failed to clean up upload staging directory {}: {}",
            parent.display(),
            error
        );
    }
}

async fn cleanup_upload_artifacts(staged_path: Option<&Path>, reservation_path: Option<&Path>) {
    cleanup_staged_file(staged_path).await;
    cleanup_staged_file(reservation_path).await;
}

fn filename_extension(field: &Field) -> Option<String> {
    field
        .content_disposition()
        .and_then(|cd| cd.get_filename())
        .and_then(|filename| {
            PathBuf::from(filename)
                .extension()
                .map(|os| os.to_string_lossy().to_string())
        })
}

fn normalize_extension(extension: String) -> Option<String> {
    let extension = extension.to_ascii_lowercase();
    if extension.is_empty()
        || extension.len() > 16
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(extension)
}

fn content_type_extension(field: &Field) -> Option<String> {
    field
        .content_type()
        .and_then(|mime| {
            mime_guess::get_mime_extensions(mime).and_then(|exts| exts.first().copied())
        })
        .map(str::to_string)
}

fn generate_unique_upload_id(
    config: &config::Folio,
    extension: Option<&str>,
) -> Result<(UploadId, PathBuf), FolioError> {
    let mut attempts = 0u32;
    loop {
        let candidate = UploadId::new(8);
        let file_name = candidate.file_name(extension);
        if let Some(reservation_path) = reserve_upload_name(config, &file_name)? {
            return Ok((candidate, reservation_path));
        }

        attempts += 1;
        if attempts >= 10 {
            return Err(FolioError::Internal {
                source: "failed to generate unique upload id after 10 attempts".to_string(),
                context: None,
            });
        }
    }
}

fn reserve_upload_name(
    config: &config::Folio,
    file_name: &str,
) -> Result<Option<PathBuf>, FolioError> {
    let final_path = config.build_full_upload_path(Path::new(file_name));
    if final_path.exists() {
        return Ok(None);
    }

    let reservation_name = format!("{}.reserve", file_name);
    let reservation_path = config.build_upload_staging_path(Path::new(&reservation_name));
    if let Some(parent) = reservation_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| FolioError::Internal {
            source: error.to_string(),
            context: Some("create upload staging directory".to_string()),
        })?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&reservation_path)
    {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(None),
        Err(error) => {
            return Err(FolioError::Internal {
                source: error.to_string(),
                context: Some("reserve upload name".to_string()),
            });
        }
    }

    if final_path.exists() {
        let _ = std::fs::remove_file(&reservation_path);
        return Ok(None);
    }
    Ok(Some(reservation_path))
}

async fn save_field_to_path(
    field: &mut Field,
    full_path: &Path,
    max_size: usize,
    request_bytes: &mut usize,
    max_request_size: usize,
) -> Result<(), FolioError> {
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| FolioError::Internal {
                source: format!("failed to create upload directory: {}", e),
                context: Some(format!("create directories for: {}", full_path.display())),
            })?;
    }

    let mut output = tokio::fs::File::create(full_path).await.map_err(|e| {
        let message = format!("failed to save file: {}", e);
        log::error!("POST /uploads error: {}", message);
        FolioError::Internal {
            source: message,
            context: None,
        }
    })?;

    let mut bytes_written: usize = 0;
    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|error| map_multipart_error(error, "file field"))?;
        consume_request_budget(request_bytes, data.len(), max_request_size)?;
        bytes_written += data.len();
        if bytes_written > max_size {
            let message = format!(
                "file too large: {} bytes exceeds {} byte limit",
                bytes_written, max_size
            );
            log::error!("POST /uploads error: {}", message);
            drop(output);
            let _ = tokio::fs::remove_file(full_path).await;
            return Err(FolioError::PayloadTooLarge { reason: message });
        }
        output.write_all(&data).await.map_err(|e| {
            let message = format!("failed to save file: {}", e);
            log::error!("POST /uploads error: {}", message);
            FolioError::Internal {
                source: message,
                context: None,
            }
        })?;
    }

    output.flush().await.map_err(|e| {
        let message = format!("failed to flush file: {}", e);
        log::error!("POST /uploads error: {}", message);
        FolioError::Internal {
            source: message,
            context: Some(format!("flush upload file: {}", full_path.display())),
        }
    })?;

    Ok(())
}

async fn read_text_field(
    field: &mut Field,
    max_size: usize,
    request_bytes: &mut usize,
    max_request_size: usize,
) -> Result<String, FolioError> {
    let mut value = Vec::new();
    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|error| map_multipart_error(error, "text field"))?;
        consume_request_budget(request_bytes, data.len(), max_request_size)?;
        if value.len().saturating_add(data.len()) > max_size {
            return Err(FolioError::PayloadTooLarge {
                reason: format!("multipart text field exceeds {} byte limit", max_size),
            });
        }
        value.extend_from_slice(&data);
    }

    String::from_utf8(value).map_err(|e| FolioError::BadRequest {
        reason: format!("multipart text field is not utf-8: {}", e),
    })
}

fn consume_request_budget(
    bytes_seen: &mut usize,
    chunk_size: usize,
    max_size: usize,
) -> Result<(), FolioError> {
    *bytes_seen = bytes_seen.saturating_add(chunk_size);
    if *bytes_seen > max_size {
        return Err(FolioError::PayloadTooLarge {
            reason: format!("upload fields exceed {} byte request limit", max_size),
        });
    }
    Ok(())
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    const MAX_VALUE: u64 = 10_000_000;

    let s = s.trim();
    if s.is_empty() {
        return Err("Invalid duration format".to_string());
    }

    // Direct plain seconds (pure number)
    if let Ok(val) = s.parse::<u64>() {
        if val > MAX_VALUE {
            return Err(format!(
                "Duration value {} exceeds maximum allowed {}",
                val, MAX_VALUE
            ));
        }
        return Ok(Duration::from_secs(val));
    }

    let len = s.len();
    if len < 2 {
        return Err("Invalid duration format".to_string());
    }

    let unit = &s[len - 1..];
    let val_str = &s[..len - 1];
    let val: u64 = val_str.parse().map_err(|_| "Invalid number".to_string())?;

    if val > MAX_VALUE {
        return Err(format!(
            "Duration value {} exceeds maximum allowed {}",
            val, MAX_VALUE
        ));
    }

    match unit {
        "s" => Ok(Duration::from_secs(val)),
        "m" => Ok(Duration::from_secs(val.saturating_mul(60))),
        "h" => Ok(Duration::from_secs(val.saturating_mul(3_600))),
        "d" => Ok(Duration::from_secs(val.saturating_mul(86_400))),
        _ => Err("Unknown unit".to_string()),
    }
}

fn upload_ttl(config: &config::Folio, expire: Option<&str>) -> Result<Duration, String> {
    let ttl = match expire {
        Some(value) => parse_duration(value)?,
        None => Duration::from_secs(config.default_upload_ttl_secs),
    };
    if ttl.as_secs() > config.max_upload_ttl_secs {
        return Err(format!(
            "expiration exceeds maximum of {} seconds",
            config.max_upload_ttl_secs
        ));
    }
    Ok(ttl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpMessage, http::header, test as awtest};

    mod upload_id {
        use super::*;

        #[test]
        fn generates_correct_length() {
            let id = UploadId::new(8);
            assert_eq!(id.0.len(), 8);

            let id_16 = UploadId::new(16);
            assert_eq!(id_16.0.len(), 16);
        }

        #[test]
        fn generates_unique_ids() {
            let id1 = UploadId::new(8);
            let id2 = UploadId::new(8);
            assert_ne!(id1.0, id2.0);
        }

        #[test]
        fn contains_base62_characters() {
            let id = UploadId::new(100);
            for c in id.0.chars() {
                assert!(c.is_ascii_alphanumeric(), "Character '{}' is not BASE62", c);
            }
        }

        #[test]
        fn file_name_with_extension() {
            let id = UploadId("test123".to_string());
            let filename = id.file_name(Some("txt"));
            assert_eq!(filename, "test123.txt");
        }

        #[test]
        fn file_name_without_extension() {
            let id = UploadId("test123".to_string());
            let filename = id.file_name(None);
            assert_eq!(filename, "test123");
        }

        #[test]
        fn file_name_with_multiple_extensions() {
            let id = UploadId("abc123".to_string());
            let filename = id.file_name(Some("tar.gz"));
            assert_eq!(filename, "abc123.tar.gz");
        }
    }

    #[test]
    fn parses_plain_seconds_and_unit_durations() {
        assert_eq!(parse_duration("604800").unwrap(), Duration::from_secs(604800));
        assert_eq!(parse_duration("3600").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("7d").unwrap(), Duration::from_secs(604800));
        assert_eq!(parse_duration("1h").unwrap(), Duration::from_secs(3600));
        assert_eq!(parse_duration("30m").unwrap(), Duration::from_secs(1800));
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert!(parse_duration("invalid").is_err());
    }

    #[test]
    fn missing_expire_uses_default_ttl_instead_of_maximum() {
        let config = config::Folio {
            default_upload_ttl_secs: 60,
            max_upload_ttl_secs: 120,
            ..config::Folio::default()
        };

        assert_eq!(upload_ttl(&config, None).unwrap(), Duration::from_secs(60));
    }

    #[test]
    fn upload_name_reservation_prevents_concurrent_metadata_owners() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config::Folio {
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            ..config::Folio::default()
        };
        let file_name = "same-id.txt";

        let first = reserve_upload_name(&config, file_name).unwrap().unwrap();
        let second = reserve_upload_name(&config, file_name).unwrap();

        assert!(second.is_none());
        assert!(first.exists());
    }

    #[actix_web::test]
    async fn stalled_body_hits_the_upload_deadline() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("multipart/form-data; boundary=X-BOUNDARY"),
        );
        let stream =
            futures_util::stream::pending::<Result<web::Bytes, actix_web::error::PayloadError>>();
        let payload = Multipart::new(&headers, stream);
        let mut parts = UploadParts::default();

        let error = save_upload_payload_with_deadline(
            payload,
            &config::Folio::default(),
            &mut parts,
            Duration::from_millis(5),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, FolioError::RequestTimeout { .. }));
    }

    #[test]
    fn startup_cleanup_removes_crash_stranded_staging_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config::Folio {
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            ..config::Folio::default()
        };
        let staging_dir = temp_dir.path().join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir_all(&staging_dir).unwrap();
        std::fs::write(staging_dir.join("orphan.part"), "orphan").unwrap();

        cleanup_staging_dir(&config).unwrap();

        assert!(!staging_dir.exists());
    }

    fn test_state() -> (
        config::Folio,
        Arc<ExpiryStore>,
        Arc<PrivateIndexStore>,
        Arc<UploadProtection>,
        tempfile::TempDir,
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config::Folio {
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            data_path: temp_dir.path().to_string_lossy().to_string(),
            ..config::Folio::default()
        };

        let expiry_store = Arc::new(ExpiryStore::new(&config));
        let private_store = Arc::new(PrivateIndexStore::new(&config));
        let protection = Arc::new(UploadProtection::disabled_for_tests());

        (config, expiry_store, private_store, protection, temp_dir)
    }

    fn multipart_body(filename: &str, content_type: Option<&str>, content: &str) -> String {
        let content_type_header = content_type
            .map(|ct| format!("Content-Type: {}\r\n", ct))
            .unwrap_or_default();

        format!(
            "--X-BOUNDARY\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n\
             {}\
             \r\n\
             {}\r\n\
             --X-BOUNDARY--\r\n",
            filename, content_type_header, content
        )
    }

    #[actix_web::test]
    async fn raw_multipart_headers_count_toward_the_request_limit() {
        let (mut config, expiry_store, private_store, protection, _temp_dir) = test_state();
        config.max_upload_size = 1;
        config.max_upload_text_field_size = 1;
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;
        let body = format!(
            "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"unknown\"\r\nX-Fill: {}\r\n\r\n\r\n--X-BOUNDARY--\r\n",
            "a".repeat(MAX_MULTIPART_OVERHEAD_BYTES + 1024)
        );
        let mut request = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(body)
            .to_request();
        request.headers_mut().remove(header::CONTENT_LENGTH);
        assert!(!request.headers().contains_key(header::CONTENT_LENGTH));

        let response = awtest::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[actix_web::test]
    async fn oversized_content_length_fails_before_concurrency_admission() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let admission_request = awtest::TestRequest::post().to_http_request();
        let mut held_admissions = Vec::new();
        loop {
            match protection.admit(&admission_request, temp_dir.path()).await {
                Ok(admission) => held_admissions.push(admission),
                Err(FolioError::TooManyRequests {
                    code: "upload_busy",
                    ..
                }) => break,
                Err(error) => panic!("unexpected admission error: {error:?}"),
            }
        }
        assert!(!held_admissions.is_empty());

        let raw_body_limit = raw_upload_body_limit(&config);
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;
        let request = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .insert_header((header::CONTENT_LENGTH, raw_body_limit + 1))
            .to_request();

        let response = awtest::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[actix_web::test]
    async fn rejects_many_zero_length_unknown_parts_without_uploading() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;
        let mut body = String::new();
        for index in 0..100 {
            body.push_str(&format!(
                "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"unknown-{index}\"\r\n\r\n\r\n"
            ));
        }
        body.push_str("--X-BOUNDARY--\r\n");
        let request = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(body)
            .to_request();

        let response = awtest::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!temp_dir.path().join(config::UPLOAD_STAGING_DIR).exists());
    }

    #[actix_web::test]
    async fn success_with_text_file() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;

        let req = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body(
                "test.txt",
                Some("text/plain"),
                "test content",
            ))
            .to_request();
        let response = awtest::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(location.starts_with("/files/"));
        assert!(location.ends_with(".txt"));

        let filename = location.strip_prefix("/files/").unwrap();
        let content = std::fs::read_to_string(temp_dir.path().join(filename)).unwrap();
        assert_eq!(content, "test content");
        let body: serde_json::Value = awtest::read_body_json(response).await;
        assert!(body["expires_at"].as_u64().unwrap() > 0);
    }

    #[actix_web::test]
    async fn success_without_extension() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;

        let req = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body("noext", None, "test content"))
            .to_request();
        let response = awtest::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.starts_with("/files/"));
        assert!(!location.strip_prefix("/files/").unwrap().contains('.'));

        let filename = location.strip_prefix("/files/").unwrap();
        let content = std::fs::read_to_string(temp_dir.path().join(filename)).unwrap();
        assert_eq!(content, "test content");
    }

    #[actix_web::test]
    async fn success_with_authorized_emails() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;

        let mut body = multipart_body("test.txt", Some("text/plain"), "private content");
        body = body.replace(
            "--X-BOUNDARY--\r\n",
            "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"authorized_emails\"\r\n\r\nbob@example.com, alice@example.com\r\n--X-BOUNDARY--\r\n",
        );

        let req = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(body)
            .to_request();
        let response = awtest::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        let filename = location.strip_prefix("/files/").unwrap();

        let raw = std::fs::read_to_string(temp_dir.path().join("private-files.json")).unwrap();
        assert!(raw.contains(filename));
        assert!(raw.contains("bob@example.com"));
        assert!(raw.contains("alice@example.com"));
    }

    #[actix_web::test]
    async fn rejects_second_file_field_without_leaving_uploads() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;

        let body = "--X-BOUNDARY\r\n\
                    Content-Disposition: form-data; name=\"file\"; filename=\"one.txt\"\r\n\
                    Content-Type: text/plain\r\n\r\n\
                    one\r\n\
                    --X-BOUNDARY\r\n\
                    Content-Disposition: form-data; name=\"file\"; filename=\"two.txt\"\r\n\
                    Content-Type: text/plain\r\n\r\n\
                    two\r\n\
                    --X-BOUNDARY--\r\n";
        let req = awtest::TestRequest::post()
            .uri("/uploads")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(body)
            .to_request();

        let response = awtest::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }

    #[actix_web::test]
    async fn rejects_ttl_above_configured_maximum_and_removes_file() {
        let (mut config, expiry_store, private_store, protection, temp_dir) = test_state();
        config.max_upload_ttl_secs = 7 * 24 * 60 * 60;
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;

        let req = awtest::TestRequest::post()
            .uri("/uploads?expire=8d")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body("test.txt", Some("text/plain"), "content"))
            .to_request();

        let response = awtest::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }

    #[actix_web::test]
    async fn rejects_oversized_authorized_emails_and_removes_file() {
        let (mut config, expiry_store, private_store, protection, temp_dir) = test_state();
        config.max_upload_text_field_size = 8;
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;
        let body = multipart_body("test.txt", Some("text/plain"), "content").replace(
            "--X-BOUNDARY--\r\n",
            "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"authorized_emails\"\r\n\r\nlong@example.com\r\n--X-BOUNDARY--\r\n",
        );

        let response = awtest::call_service(
            &app,
            awtest::TestRequest::post()
                .uri("/uploads")
                .insert_header((
                    header::CONTENT_TYPE,
                    "multipart/form-data; boundary=X-BOUNDARY",
                ))
                .set_payload(body)
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }

    #[actix_web::test]
    async fn expiry_failure_rolls_back_file_and_private_entry() {
        let (config, expiry_store, private_store, protection, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("expiry-index.json"), "not json").unwrap();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store.clone()))
                .app_data(web::Data::new(protection))
                .service(upload_file),
        )
        .await;
        let body = multipart_body("test.txt", Some("text/plain"), "content").replace(
            "--X-BOUNDARY--\r\n",
            "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"authorized_emails\"\r\n\r\na@example.com\r\n--X-BOUNDARY--\r\n",
        );

        let response = awtest::call_service(
            &app,
            awtest::TestRequest::post()
                .uri("/uploads")
                .insert_header((
                    header::CONTENT_TYPE,
                    "multipart/form-data; boundary=X-BOUNDARY",
                ))
                .set_payload(body)
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let private_raw =
            std::fs::read_to_string(temp_dir.path().join("private-files.json")).unwrap();
        let private_json: serde_json::Value = serde_json::from_str(&private_raw).unwrap();
        assert_eq!(private_json["entries"].as_array().unwrap().len(), 0);
        let files = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                !matches!(
                    entry.file_name().to_string_lossy().as_ref(),
                    "expiry-index.json" | "private-files.json"
                )
            })
            .count();
        assert_eq!(files, 0);
    }
}
