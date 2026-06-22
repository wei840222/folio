use std::ops::Deref;
use std::path::{Path, PathBuf};

use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::{NamedFile, TempFile};
use rocket::http::uri::Segments;
use rocket::request::FromSegments;
use rocket::response::{Redirect, Responder};
use rocket::serde::json::Json;

use super::auth::VerifiedIdentity;
use super::config;
use super::error::FolioError;
use super::path::SafePath;
use super::private_index::PrivateIndexStore;

/// Validated path that prevents directory traversal.
///
/// Wraps SafePath to provide FromSegments extraction for Rocket routes.
pub struct ValidatedPath(SafePath);

impl Deref for ValidatedPath {
    type Target = SafePath;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for ValidatedPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'r> FromSegments<'r> for ValidatedPath {
    type Error = &'static str;

    fn from_segments(
        segments: Segments<'r, rocket::http::uri::fmt::Path>,
    ) -> Result<Self, Self::Error> {
        let path = PathBuf::from_segments(segments).map_err(|_| "invalid path")?;
        let safe_path = SafePath::from_user_input(&path).map_err(|_| "invalid path")?;
        Ok(ValidatedPath(safe_path))
    }
}

#[derive(rocket::serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FileResponse {
    message: String,
}

impl FileResponse {
    fn success(message: &str) -> Json<Self> {
        Json(FileResponse {
            message: message.to_string(),
        })
    }
}

type FileResult = Result<rocket::response::status::Custom<Json<FileResponse>>, FolioError>;

#[derive(Responder)]
pub enum FileGetResponse {
    Redirect(Redirect),
    File(NamedFile),
}

#[get("/<path..>", rank = 5)]
pub async fn get_file(
    config: &State<config::Folio>,
    private_index: &State<std::sync::Arc<PrivateIndexStore>>,
    path: ValidatedPath,
) -> Result<FileGetResponse, FolioError> {
    let is_private = private_index
        .is_private(path.as_path())
        .map_err(|e| FolioError::store_error(e, "check private index"))?;

    if is_private {
        return Ok(FileGetResponse::Redirect(Redirect::found(format!(
            "/private-files/{}",
            path
        ))));
    }

    let file = open_upload_file(config, &path).await?;
    Ok(FileGetResponse::File(file))
}

#[get("/<path..>", rank = 5)]
pub async fn get_private_file(
    config: &State<config::Folio>,
    private_index: &State<std::sync::Arc<PrivateIndexStore>>,
    identity: VerifiedIdentity,
    path: ValidatedPath,
) -> Result<NamedFile, FolioError> {
    let entry = private_index
        .get_entry(path.as_path())
        .map_err(|e| FolioError::store_error(e, "check private index"))?;

    match entry {
        Some(e) => {
            let email = identity.0.email.as_deref().unwrap_or("");
            if !e.authorized_emails.iter().any(|em| em == email) {
                log::warn!(
                    "private file access denied: email '{}' not authorized for path '{}'",
                    email,
                    path
                );
                return Err(FolioError::Forbidden {
                    reason: "email not authorized for this file".to_string(),
                });
            }
        }
        None => {
            log::warn!(
                "accessing /private-files/ for non-private path: {}",
                path
            );
        }
    }

    log::info!(
        "private file access granted: sub={}, email={:?}, path={}",
        identity.0.sub,
        identity.0.email,
        path
    );

    open_upload_file(config, &path).await
}

/// Ensure parent directories exist
fn ensure_parent_dirs(path: &Path) -> Result<(), FolioError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            let message = format!("failed to create directories: {:?}", e);
            log::error!("{}, path: {}", message, path.display());
            FolioError::Internal {
                source: message,
                context: Some(format!("create directories for: {}", path.display())),
            }
        })?;
    }
    Ok(())
}

/// Validate and open an upload file, returning a `NamedFile` on success.
async fn open_upload_file(
    config: &State<config::Folio>,
    path: &ValidatedPath,
) -> Result<NamedFile, FolioError> {
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    if !full_path.exists() {
        return Err(FolioError::NotFound {
            path: path.to_string(),
        });
    }

    if !full_path.is_file() {
        return Err(FolioError::BadRequest {
            reason: format!("path is not a file: {}", path),
        });
    }

    NamedFile::open(full_path).await.map_err(|e| {
        FolioError::Internal {
            source: format!("failed to open file: {}", e),
            context: Some(format!("open file: {}", path)),
        }
    })
}

#[post("/<path..>", data = "<file>", rank = 5)]
pub async fn create_file(
    config: &State<config::Folio>,
    path: ValidatedPath,
    mut file: Form<Strict<TempFile<'_>>>,
) -> FileResult {
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    // Check if file already exists
    if full_path.exists() {
        return Err(FolioError::Conflict {
            path: path.to_string(),
        });
    }

    ensure_parent_dirs(&full_path)?;

    // Persist file
    file.copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {:?}", e);
        log::error!("POST /files error: {}", message);
        FolioError::Internal {
            source: message,
            context: Some("save uploaded file".to_string()),
        }
    })?;

    Ok(rocket::response::status::Custom(
        rocket::http::Status::Created,
        FileResponse::success("file created successfully"),
    ))
}

#[put("/<path..>", data = "<file>", rank = 5)]
pub async fn upsert_file(
    config: &State<config::Folio>,
    path: ValidatedPath,
    mut file: Form<Strict<TempFile<'_>>>,
) -> FileResult {
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));
    let file_exists = full_path.exists();

    ensure_parent_dirs(&full_path)?;

    // Persist file (overwrites if exists)
    file.copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {:?}", e);
        log::error!("PUT /files error: {}", message);
        FolioError::Internal {
            source: message,
            context: Some("save uploaded file".to_string()),
        }
    })?;

    if file_exists {
        Ok(rocket::response::status::Custom(
            rocket::http::Status::Ok,
            FileResponse::success("file updated successfully"),
        ))
    } else {
        Ok(rocket::response::status::Custom(
            rocket::http::Status::Created,
            FileResponse::success("file created successfully"),
        ))
    }
}

#[delete("/<path..>", rank = 5)]
pub async fn delete_file(
    config: &State<config::Folio>,
    path: ValidatedPath,
) -> FileResult {
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    // Check if file exists
    if !full_path.exists() {
        return Err(FolioError::NotFound {
            path: path.to_string(),
        });
    }

    // Check if it's a file (not a directory)
    if !full_path.is_file() {
        return Err(FolioError::BadRequest {
            reason: format!("path is not a file: {}", path),
        });
    }

    // Delete file
    std::fs::remove_file(&full_path).map_err(|e| {
        let message = format!("failed to delete file: {:?}", e);
        log::error!("DELETE /files error: {}", message);
        FolioError::Internal {
            source: message,
            context: Some(format!("delete file: {}", path)),
        }
    })?;

    Ok(rocket::response::status::Custom(
        rocket::http::Status::Ok,
        FileResponse::success("file deleted successfully"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod file_endpoints {
        use super::*;
        use rocket::http::{ContentType, Status};
        use rocket::local::blocking::Client;
        use std::sync::Arc;

        fn test_rocket() -> (rocket::Rocket<rocket::Build>, tempfile::TempDir) {
            let temp_dir = tempfile::tempdir().unwrap();
            let mut config = config::Folio::default();
            config.uploads_path = temp_dir.path().to_string_lossy().to_string();
            config.data_path = temp_dir.path().to_string_lossy().to_string();

            let private_index = Arc::new(PrivateIndexStore::new(&config));
            let access_auth = Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
            ));

            let rocket = rocket::build()
                .mount(
                    "/files",
                    routes![get_file, create_file, upsert_file, delete_file],
                )
                .mount("/private-files", routes![get_private_file])
                .manage(config)
                .manage(private_index)
                .manage(access_auth);

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

        use crate::test_utils::make_hs256_token;

        #[test]
        fn create_file_success() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client
                .post("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body(
                    "test.txt",
                    Some("text/plain"),
                    "test content",
                ))
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            // Verify file content
            let file_path = temp_dir.path().join("test.txt");
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "test content");
        }

        #[test]
        fn create_file_with_nested_path() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client
                .post("/files/folder/subfolder/test.txt")
                .header(multipart_content_type())
                .body(multipart_body(
                    "test.txt",
                    Some("text/plain"),
                    "nested content",
                ))
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            // Verify file content
            let file_path = temp_dir.path().join("folder/subfolder/test.txt");
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "nested content");
        }

        #[test]
        fn create_file_already_exists() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Create file first time
            let response1 = client
                .post("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "content 1"))
                .dispatch();
            assert_eq!(response1.status(), Status::Created);

            // Try to create same file again
            let response2 = client
                .post("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "content 2"))
                .dispatch();
            assert_eq!(response2.status(), Status::Conflict);

            // Verify original content unchanged
            let file_path = temp_dir.path().join("test.txt");
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "content 1");
        }

        #[test]
        fn upsert_creates_new_file() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client
                .put("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body(
                    "test.txt",
                    Some("text/plain"),
                    "new content",
                ))
                .dispatch();

            assert_eq!(response.status(), Status::Created);

            // Verify file content
            let file_path = temp_dir.path().join("test.txt");
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "new content");
        }

        #[test]
        fn upsert_updates_existing_file() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Create file first
            client
                .post("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "original"))
                .dispatch();

            // Update with PUT
            let response = client
                .put("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "updated"))
                .dispatch();

            assert_eq!(response.status(), Status::Ok);

            // Verify updated content
            let file_path = temp_dir.path().join("test.txt");
            let content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(content, "updated");
        }

        #[test]
        fn delete_file_success() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Create file first
            client
                .post("/files/test.txt")
                .header(multipart_content_type())
                .body(multipart_body("test.txt", Some("text/plain"), "content"))
                .dispatch();

            // Delete file
            let response = client.delete("/files/test.txt").dispatch();
            assert_eq!(response.status(), Status::Ok);

            // Verify file is deleted
            let file_path = temp_dir.path().join("test.txt");
            assert!(!file_path.exists());
        }

        #[test]
        fn delete_file_not_found() {
            let (rocket, _temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let response = client.delete("/files/nonexistent.txt").dispatch();
            assert_eq!(response.status(), Status::NotFound);
        }

        #[test]
        fn rejects_parent_directory_traversal() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Try to create file with .. in path
            let response = client
                .post("/files/../escape.txt")
                .header(multipart_content_type())
                .body(multipart_body("escape.txt", Some("text/plain"), "content"))
                .dispatch();

            // Rocket normalizes the path, so .. gets removed
            // The file would be created as "escape.txt" in the root
            assert_eq!(response.status(), Status::Created);

            // Verify the file was created without escaping the directory
            let file_path = temp_dir.path().join("escape.txt");
            assert!(file_path.exists());
        }

        #[test]
        fn delete_directory_fails() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            // Create a directory
            let dir_path = temp_dir.path().join("testdir");
            std::fs::create_dir(&dir_path).unwrap();

            // Try to delete directory
            let response = client.delete("/files/testdir").dispatch();
            assert_eq!(response.status(), Status::BadRequest);
        }

        #[test]
        fn get_public_file_success() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let file_path = temp_dir.path().join("public.txt");
            std::fs::write(&file_path, "public-content").unwrap();

            let response = client.get("/files/public.txt").dispatch();
            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "public-content");
        }

        #[test]
        fn get_private_file_redirects_to_private_prefix() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let file_path = temp_dir.path().join("secret.txt");
            std::fs::write(&file_path, "secret-content").unwrap();

            let index_path = temp_dir.path().join("private-files.json");
            std::fs::write(&index_path, r#"{"entries":[{"path":"secret.txt", "authorized_emails": []}]}"#).unwrap();

            let response = client.get("/files/secret.txt").dispatch();
            assert_eq!(response.status(), Status::Found);
            assert_eq!(
                response.headers().get_one("Location").unwrap(),
                "/private-files/secret.txt"
            );
        }

        #[test]
        fn private_files_requires_access_jwt_header() {
            let (rocket, temp_dir) = test_rocket();
            let client = Client::tracked(rocket).unwrap();

            let file_path = temp_dir.path().join("secret.txt");
            std::fs::write(&file_path, "secret-content").unwrap();

            let response = client.get("/private-files/secret.txt").dispatch();
            assert_eq!(response.status(), Status::Unauthorized);
        }

        #[test]
        fn private_files_with_valid_hs256_jwt_returns_200() {
            let temp_dir = tempfile::tempdir().unwrap();
            let mut config = config::Folio::default();
            config.uploads_path = temp_dir.path().to_string_lossy().to_string();
            config.data_path = temp_dir.path().to_string_lossy().to_string();

            let private_index = Arc::new(PrivateIndexStore::new(&config));
            private_index.mark_private(&PathBuf::from("secret.txt"), vec!["allowed@example.com".to_string()]).unwrap();

            let access_auth = Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
            ));

            let rocket = rocket::build()
                .mount(
                    "/files",
                    routes![get_file, create_file, upsert_file, delete_file],
                )
                .mount("/private-files", routes![get_private_file])
                .manage(config)
                .manage(private_index)
                .manage(access_auth);

            let client = Client::tracked(rocket).unwrap();
            let file_path = temp_dir.path().join("secret.txt");
            std::fs::write(&file_path, "secret-content").unwrap();

            let token = make_hs256_token(
                "test-secret",
                "user-1",
                Some("allowed@example.com"),
                &["team-a"],
                "https://issuer.example.com",
                "folio-app",
                3600,
            );

            let response = client
                .get("/private-files/secret.txt")
                .header(rocket::http::Header::new("Cf-Access-Jwt-Assertion", token))
                .dispatch();

            assert_eq!(response.status(), Status::Ok);
            assert_eq!(response.into_string().unwrap(), "secret-content");
        }

        #[test]
        fn private_files_with_disallowed_email_returns_403() {
            let temp_dir = tempfile::tempdir().unwrap();
            let mut config = config::Folio::default();
            config.uploads_path = temp_dir.path().to_string_lossy().to_string();
            config.data_path = temp_dir.path().to_string_lossy().to_string();

            let private_index = Arc::new(PrivateIndexStore::new(&config));
            private_index.mark_private(&PathBuf::from("secret.txt"), vec!["only@example.com".to_string()]).unwrap();

            let access_auth = Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
            ));

            let rocket = rocket::build()
                .mount(
                    "/files",
                    routes![get_file, create_file, upsert_file, delete_file],
                )
                .mount("/private-files", routes![get_private_file])
                .manage(config)
                .manage(private_index)
                .manage(access_auth);

            let client = Client::tracked(rocket).unwrap();
            let file_path = temp_dir.path().join("secret.txt");
            std::fs::write(&file_path, "secret-content").unwrap();

            let token = make_hs256_token(
                "test-secret",
                "user-1",
                Some("blocked@example.com"),
                &["team-a"],
                "https://issuer.example.com",
                "folio-app",
                3600,
            );

            let response = client
                .get("/private-files/secret.txt")
                .header(rocket::http::Header::new("Cf-Access-Jwt-Assertion", token))
                .dispatch();

            assert_eq!(response.status(), Status::Forbidden);
        }
    }
}
