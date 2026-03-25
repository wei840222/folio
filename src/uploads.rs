use std::path::PathBuf;
use std::time::Duration;

use rand::Rng;
use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::{Created, Custom};
use rocket::serde::{Serialize, json::Json};

use super::config;
use super::expiry::ExpiryStore;
use super::private_index::PrivateIndexStore;

#[derive(FromForm)]
pub struct UploadForm<'r> {
    pub file: TempFile<'r>,
    pub authorized_emails: Option<String>,
}

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
#[serde(crate = "rocket::serde")]
pub struct UploadResponse {
    message: String,
}

type UploadResult = Result<Created<Json<UploadResponse>>, Custom<Json<UploadResponse>>>;

#[post("/?<expire>", data = "<form>")]
pub async fn upload_file(
    config: &State<config::Folio>,
    expiry_store: &State<std::sync::Arc<ExpiryStore>>,
    private_store: &State<std::sync::Arc<PrivateIndexStore>>,
    mut form: Form<Strict<UploadForm<'_>>>,
    expire: Option<&str>,
) -> UploadResult {
    // Determine file extension
    let extension = {
        let ct_ext = form.file.content_type().and_then(|ct| ct.extension());
        let nm_ext = form.file.raw_name().and_then(|nm| {
            PathBuf::from(nm.as_str().unwrap_or("").to_string())
                .extension()
                .map(|os| os.to_string_lossy().to_string())
        });

        log::info!(
            "Upload extension check: content-type-ext={:?}, filename-ext={:?}",
            ct_ext,
            nm_ext
        );

        match (ct_ext, nm_ext) {
            // Priority: if filename has an extension, use it, especially if content-type is generic 'bin'
            (_, Some(ext)) if !ext.is_empty() => Some(ext),
            // Fallback to content-type extension if filename has none
            (Some(ext), None) => Some(ext.to_string()),
            // Otherwise nothing
            _ => None,
        }
    };
    let ext_ref = extension.as_deref();

    // Generate unique ID, retry if file already exists
    let id = loop {
        let candidate = UploadId::new(8);
        let file_name = candidate.file_name(ext_ref);
        let path = config.build_full_upload_path(&PathBuf::from(&file_name));
        if !path.exists() {
            break candidate;
        }
    };

    let file_name = id.file_name(ext_ref);
    let full_path = config.build_full_upload_path(&PathBuf::from(&file_name));

    // Persist file
    form.file.copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {}", e);
        log::error!("POST /uploads error: {}", message);
        Custom(
            Status::InternalServerError,
            Json(UploadResponse { message }),
        )
    })?;

    // Mark as private if authorized_emails is provided
    if let Some(emails_str) = &form.authorized_emails {
        let emails: Vec<String> = emails_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !emails.is_empty() {
            private_store
                .mark_private(&PathBuf::from(&file_name), emails)
                .map_err(|e| {
                    let message = format!("failed to mark file as private: {}", e);
                    log::error!("POST /uploads error: {}", message);
                    Custom(
                        Status::InternalServerError,
                        Json(UploadResponse { message }),
                    )
                })?;
        }
    }

    let ttl = match expire {
        Some(s) => parse_duration(s).unwrap_or(Duration::from_secs(168 * 3600)),
        None => Duration::from_secs(168 * 3600),
    };

    expiry_store.schedule(&full_path, ttl).map_err(|e| {
        let message = format!("failed to schedule expiration for {}: {}", file_name, e);
        log::error!("POST /uploads error: {}", message);
        Custom(
            Status::InternalServerError,
            Json(UploadResponse { message }),
        )
    })?;

    Ok(
        Created::new(format!("/files/{}", file_name)).body(Json(UploadResponse {
            message: "file uploaded successfully".to_string(),
        })),
    )
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    // Simple parser for now: supports 's', 'm', 'h', 'd'
    // e.g., "10s", "5m", "24h"
    let len = s.len();
    if len < 2 {
        return Err("Invalid duration format".to_string());
    }

    let unit = &s[len - 1..];
    let val_str = &s[..len - 1];
    let val: u64 = val_str.parse().map_err(|_| "Invalid number".to_string())?;

    match unit {
        "s" => Ok(Duration::from_secs(val)),
        "m" => Ok(Duration::from_secs(val * 60)),
        "h" => Ok(Duration::from_secs(val * 3600)),
        "d" => Ok(Duration::from_secs(val * 86400)),
        _ => Err("Unknown unit".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    mod upload_file_endpoint {
        use super::*;
        use rocket::http::{ContentType, Status};
        use rocket::local::blocking::Client;

        fn test_rocket() -> (rocket::Rocket<rocket::Build>, tempfile::TempDir) {
            let temp_dir = tempfile::tempdir().unwrap();
            let mut config = config::Folio::default();
            config.uploads_path = temp_dir.path().to_string_lossy().to_string();
            config.data_path = temp_dir.path().to_string_lossy().to_string();

            let expiry_store = std::sync::Arc::new(ExpiryStore::new(&config));
            let private_store = std::sync::Arc::new(PrivateIndexStore::new(&config));

            let rocket = rocket::build()
                .mount("/uploads", routes![upload_file])
                .manage(config)
                .manage(expiry_store)
                .manage(private_store);

            (rocket, temp_dir)
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

        fn multipart_content_type() -> ContentType {
            ContentType::new("multipart", "form-data").with_params([("boundary", "X-BOUNDARY")])
        }

        #[test]
        fn success_with_text_file() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client
                .post("/uploads")
                .header(multipart_content_type())
                .body(multipart_body(
                    "test.txt",
                    Some("text/plain"),
                    "test content",
                ))
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            let location = response.headers().get_one("Location").unwrap();
            assert!(location.starts_with("/files/"));
            assert!(location.ends_with(".txt"));

            // Verify file content
            let filename = location.strip_prefix("/files/").unwrap();
            let file_path = temp_dir.path().join(filename);
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "test content");
        }

        #[test]
        fn success_without_extension() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client
                .post("/uploads")
                .header(multipart_content_type())
                .body(multipart_body("noext", None, "test content"))
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            let location = response.headers().get_one("Location").unwrap();
            assert!(location.starts_with("/files/"));
            assert!(!location.contains("."));

            // Verify file content
            let filename = location.strip_prefix("/files/").unwrap();
            let file_path = temp_dir.path().join(filename);
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "test content");
        }

        #[test]
        fn success_with_authorized_emails() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let mut body = multipart_body("test.txt", Some("text/plain"), "private content");
            // Append authorized_emails field
            body = body.replace("--X-BOUNDARY--\r\n", 
                "--X-BOUNDARY\r\nContent-Disposition: form-data; name=\"authorized_emails\"\r\n\r\nbob@example.com, alice@example.com\r\n--X-BOUNDARY--\r\n");

            let response = client
                .post("/uploads")
                .header(multipart_content_type())
                .body(body)
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            let location = response.headers().get_one("Location").unwrap();
            let filename = location.strip_prefix("/files/").unwrap();
            
            // Verify private index has the entry
            let index_path = temp_dir.path().join("private-files.json");
            let raw = std::fs::read_to_string(index_path).unwrap();
            assert!(raw.contains(filename));
            assert!(raw.contains("bob@example.com"));
            assert!(raw.contains("alice@example.com"));
        }
    }
}
