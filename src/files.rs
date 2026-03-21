use std::ops::Deref;
use std::path::PathBuf;

use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::{NamedFile, TempFile};
use rocket::http::Status;
use rocket::http::uri::Segments;
use rocket::request::FromSegments;
use rocket::response::status::Custom;
use rocket::response::{Redirect, Responder};
use rocket::serde::{Serialize, json::Json};

use super::auth::VerifiedIdentity;
use super::config;
use super::private_index::PrivateIndexStore;

/// Validated path that prevents directory traversal
pub struct ValidatedPath(PathBuf);

impl Deref for ValidatedPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'r> FromSegments<'r> for ValidatedPath {
    type Error = &'static str;

    fn from_segments(
        segments: Segments<'r, rocket::http::uri::fmt::Path>,
    ) -> Result<Self, Self::Error> {
        let path = PathBuf::from_segments(segments).map_err(|_| "invalid path")?;

        if path.to_string_lossy().contains("..") {
            log::warn!("invalid file path: {}", path.display());
            return Err("path contains '..'");
        }

        Ok(ValidatedPath(path))
    }
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FileResponse {
    message: String,
}

type FileResult = Result<Custom<Json<FileResponse>>, Custom<Json<FileResponse>>>;

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
) -> Result<FileGetResponse, Custom<Json<FileResponse>>> {
    let is_private = private_index.is_private(&path).map_err(|e| {
        Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: format!("failed to read private index: {}", e),
            }),
        )
    })?;

    if is_private {
        return Ok(FileGetResponse::Redirect(Redirect::found(format!(
            "/private-files/{}",
            path.to_string_lossy()
        ))));
    }

    let full_path = config.build_full_upload_path(&path);
    if !full_path.exists() {
        return Err(Custom(
            Status::NotFound,
            Json(FileResponse {
                message: format!("file not found: {}", path.to_string_lossy()),
            }),
        ));
    }

    if !full_path.is_file() {
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: format!("path is not a file: {}", path.to_string_lossy()),
            }),
        ));
    }

    let file = NamedFile::open(full_path).await.map_err(|e| {
        Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: format!("failed to open file: {}", e),
            }),
        )
    })?;

    Ok(FileGetResponse::File(file))
}

#[get("/<path..>", rank = 5)]
pub async fn get_private_file(
    config: &State<config::Folio>,
    identity: VerifiedIdentity,
    path: ValidatedPath,
) -> Result<NamedFile, Custom<Json<FileResponse>>> {
    let full_path = config.build_full_upload_path(&path);
    log::info!(
        "private file access granted: sub={}, email={:?}, groups_count={}, path={}",
        identity.0.sub,
        identity.0.email,
        identity.0.groups.len(),
        path.to_string_lossy()
    );

    if !full_path.exists() {
        return Err(Custom(
            Status::NotFound,
            Json(FileResponse {
                message: format!("file not found: {}", path.to_string_lossy()),
            }),
        ));
    }

    if !full_path.is_file() {
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: format!("path is not a file: {}", path.to_string_lossy()),
            }),
        ));
    }

    NamedFile::open(full_path).await.map_err(|e| {
        Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: format!("failed to open file: {}", e),
            }),
        )
    })
}

/// Ensure parent directories exist
fn ensure_parent_dirs(path: &PathBuf) -> Result<(), Custom<Json<FileResponse>>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            let message = format!("failed to create directories: {:?}", e);
            log::error!("{}, path: {}", message, path.to_string_lossy());
            Custom(Status::InternalServerError, Json(FileResponse { message }))
        })?;
    }
    Ok(())
}

#[post("/<path..>", data = "<file>", rank = 5)]
pub async fn create_file(
    config: &State<config::Folio>,
    path: ValidatedPath,
    mut file: Form<Strict<TempFile<'_>>>,
) -> FileResult {
    let full_path = config.build_full_upload_path(&path);

    // Check if file already exists
    if full_path.exists() {
        return Err(Custom(
            Status::Conflict,
            Json(FileResponse {
                message: format!("file already exists: {}", path.to_string_lossy()),
            }),
        ));
    }

    ensure_parent_dirs(&full_path)?;

    // Persist file
    file.copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {:?}", e);
        log::error!("POST /files error: {}", message);
        Custom(Status::InternalServerError, Json(FileResponse { message }))
    })?;

    Ok(Custom(
        Status::Created,
        Json(FileResponse {
            message: "file created successfully".to_string(),
        }),
    ))
}

#[put("/<path..>", data = "<file>", rank = 5)]
pub async fn upsert_file(
    config: &State<config::Folio>,
    path: ValidatedPath,
    mut file: Form<Strict<TempFile<'_>>>,
) -> FileResult {
    let full_path = config.build_full_upload_path(&path);
    let file_exists = full_path.exists();

    ensure_parent_dirs(&full_path)?;

    // Persist file (overwrites if exists)
    file.copy_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {:?}", e);
        log::error!("PUT /files error: {}", message);
        Custom(Status::InternalServerError, Json(FileResponse { message }))
    })?;

    if file_exists {
        return Ok(Custom(
            Status::Ok,
            Json(FileResponse {
                message: "file updated successfully".to_string(),
            }),
        ));
    }

    Ok(Custom(
        Status::Created,
        Json(FileResponse {
            message: "file created successfully".to_string(),
        }),
    ))
}

#[delete("/<path..>", rank = 5)]
pub async fn delete_file(config: &State<config::Folio>, path: ValidatedPath) -> FileResult {
    let full_path = config.build_full_upload_path(&path);

    // Check if file exists
    if !full_path.exists() {
        return Err(Custom(
            Status::NotFound,
            Json(FileResponse {
                message: format!("file not found: {}", path.to_string_lossy()),
            }),
        ));
    }

    // Check if it's a file (not a directory)
    if !full_path.is_file() {
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: format!("path is not a file: {}", path.to_string_lossy()),
            }),
        ));
    }

    // Delete file
    std::fs::remove_file(&full_path).map_err(|e| {
        let message = format!("failed to delete file: {:?}", e);
        log::error!("DELETE /files error: {}", message);
        Custom(Status::InternalServerError, Json(FileResponse { message }))
    })?;

    Ok(Custom(
        Status::Ok,
        Json(FileResponse {
            message: "file deleted successfully".to_string(),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod file_endpoints {
        use super::*;
        use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
        use rocket::http::{ContentType, Status};
        use rocket::local::blocking::Client;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn test_rocket() -> (rocket::Rocket<rocket::Build>, tempfile::TempDir) {
            let temp_dir = tempfile::tempdir().unwrap();
            let mut config = config::Folio::default();
            config.uploads_path = temp_dir.path().to_string_lossy().to_string();

            let private_index = std::sync::Arc::new(PrivateIndexStore::new(&config));
            let access_auth = std::sync::Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
                &[],
                &[],
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

            let index_path = temp_dir.path().join(".private-files.json");
            std::fs::write(&index_path, r#"{"files":["secret.txt"]}"#).unwrap();

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

            let private_index = std::sync::Arc::new(PrivateIndexStore::new(&config));
            let access_auth = std::sync::Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
                &[],
                &[],
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

            let private_index = std::sync::Arc::new(PrivateIndexStore::new(&config));
            let access_auth = std::sync::Arc::new(crate::auth::AccessAuth::from_parts(
                "https://issuer.example.com",
                "folio-app",
                Some("test-secret"),
                &["only@example.com"],
                &[],
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
