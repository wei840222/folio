use std::path::PathBuf;

use rand::Rng;
use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::{Created, Custom};
use rocket::serde::{Serialize, json::Json};

use super::config;

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

#[post("/?<expire>", data = "<file>")]
pub async fn upload_file(
    config: &State<config::Folio>,
    mut file: Form<Strict<TempFile<'_>>>,
    expire: Option<&str>,
) -> UploadResult {
    log::info!("expire: {:?}", expire.unwrap_or("168h"));

    // Determine file extension
    let extension = file
        .content_type()
        .and_then(|ct| ct.extension())
        .map(|s| s.to_string());
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
    file.move_copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {}", e);
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
            let config = config::Folio {
                web_path: "".to_string(),
                uploads_path: temp_dir.path().to_string_lossy().to_string(),
                garbage_collection_pattern: vec![],
            };

            let rocket = rocket::build()
                .mount("/uploads", routes![upload_file])
                .manage(config);

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
        fn creates_unique_filenames() {
            let (rocket, _temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Upload first file
            let response1 = client
                .post("/uploads")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "content 1"))
                .dispatch();

            let location1 = response1.headers().get_one("Location").unwrap();

            // Upload second file
            let response2 = client
                .post("/uploads")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "content 2"))
                .dispatch();

            let location2 = response2.headers().get_one("Location").unwrap();

            // Should have different paths (different IDs)
            assert_ne!(location1, location2);
        }
    }
}
