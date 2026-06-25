use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use actix_multipart::{Field, Multipart};
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, Responder, post, web};
use futures_util::StreamExt;
use rand::Rng;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use super::config;
use super::error::FolioError;
use super::expiry::ExpiryStore;
use super::private_index::PrivateIndexStore;

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

#[derive(Serialize)]
pub struct UploadResponse {
    message: String,
}

#[derive(Default)]
struct UploadParts {
    file_name: Option<String>,
    authorized_emails: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UploadQuery {
    expire: Option<String>,
}

#[post("/uploads")]
pub async fn upload_file(
    config: web::Data<config::Folio>,
    expiry_store: web::Data<Arc<ExpiryStore>>,
    private_store: web::Data<Arc<PrivateIndexStore>>,
    payload: Multipart,
    query: web::Query<UploadQuery>,
) -> Result<impl Responder, FolioError> {
    let mut parts = UploadParts::default();
    save_upload_payload(payload, &config, &mut parts).await?;
    let file_name = parts.file_name.ok_or_else(|| FolioError::BadRequest {
        reason: "multipart form is missing file field".to_string(),
    })?;
    let full_path = config.build_full_upload_path(&PathBuf::from(&file_name));

    if let Some(emails_str) = &parts.authorized_emails {
        let emails: Vec<String> = emails_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !emails.is_empty() {
            private_store
                .mark_private(&PathBuf::from(&file_name), emails)
                .await
                .map_err(|e| {
                    let message = format!("failed to mark file as private: {}", e);
                    log::error!("POST /uploads error: {}", message);
                    FolioError::Internal {
                        source: message,
                        context: None,
                    }
                })?;
        }
    }

    let ttl = match query.expire.as_deref() {
        Some(s) => parse_duration(s).unwrap_or(Duration::from_secs(168 * 3600)),
        None => Duration::from_secs(168 * 3600),
    };

    expiry_store.schedule(&full_path, ttl).await.map_err(|e| {
        let message = format!("failed to schedule expiration for {}: {}", file_name, e);
        log::error!("POST /uploads error: {}", message);
        FolioError::Internal {
            source: message,
            context: None,
        }
    })?;

    Ok(HttpResponse::build(StatusCode::CREATED)
        .append_header(("Location", format!("/files/{}", file_name)))
        .json(UploadResponse {
            message: "file uploaded successfully".to_string(),
        }))
}

async fn save_upload_payload(
    mut payload: Multipart,
    config: &config::Folio,
    parts: &mut UploadParts,
) -> Result<(), FolioError> {
    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| FolioError::BadRequest {
            reason: format!("invalid multipart payload: {}", e),
        })?;

        match field.name() {
            Some("file") => {
                let filename_extension = filename_extension(&field);
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

                let id = generate_unique_upload_id(config, extension.as_deref())?;
                let file_name = id.file_name(extension.as_deref());
                let full_path = config.build_full_upload_path(&PathBuf::from(&file_name));
                save_field_to_path(&mut field, &full_path).await?;
                parts.file_name = Some(file_name);
            }
            Some("authorized_emails") => {
                parts.authorized_emails = Some(read_text_field(&mut field).await?);
            }
            _ => drain_field(&mut field).await?,
        }
    }

    Ok(())
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
) -> Result<UploadId, FolioError> {
    let mut attempts = 0u32;
    loop {
        let candidate = UploadId::new(8);
        let file_name = candidate.file_name(extension);
        let path = config.build_full_upload_path(&PathBuf::from(&file_name));

        if !path.exists() {
            return Ok(candidate);
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

async fn save_field_to_path(field: &mut Field, full_path: &Path) -> Result<(), FolioError> {
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

    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|e| FolioError::BadRequest {
            reason: format!("invalid multipart file field: {}", e),
        })?;
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

async fn read_text_field(field: &mut Field) -> Result<String, FolioError> {
    let mut value = Vec::new();
    while let Some(chunk) = field.next().await {
        let data = chunk.map_err(|e| FolioError::BadRequest {
            reason: format!("invalid multipart text field: {}", e),
        })?;
        value.extend_from_slice(&data);
    }

    String::from_utf8(value).map_err(|e| FolioError::BadRequest {
        reason: format!("multipart text field is not utf-8: {}", e),
    })
}

async fn drain_field(field: &mut Field) -> Result<(), FolioError> {
    while let Some(chunk) = field.next().await {
        chunk.map_err(|e| FolioError::BadRequest {
            reason: format!("invalid multipart field: {}", e),
        })?;
    }
    Ok(())
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    const MAX_VALUE: u64 = 10_000_000;

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

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, http::header, test as awtest};

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

    fn test_state() -> (
        config::Folio,
        Arc<ExpiryStore>,
        Arc<PrivateIndexStore>,
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

        (config, expiry_store, private_store, temp_dir)
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
    async fn success_with_text_file() {
        let (config, expiry_store, private_store, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
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
            .unwrap();
        assert!(location.starts_with("/files/"));
        assert!(location.ends_with(".txt"));

        let filename = location.strip_prefix("/files/").unwrap();
        let content = std::fs::read_to_string(temp_dir.path().join(filename)).unwrap();
        assert_eq!(content, "test content");
    }

    #[actix_web::test]
    async fn success_without_extension() {
        let (config, expiry_store, private_store, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
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
        let (config, expiry_store, private_store, temp_dir) = test_state();
        let app = awtest::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(expiry_store))
                .app_data(web::Data::new(private_store))
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
}
