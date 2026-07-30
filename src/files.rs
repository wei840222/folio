use std::path::{Path, PathBuf};
use std::sync::Arc;

use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::http::StatusCode;
use actix_web::http::header::{ContentDisposition, DispositionType, HeaderName, HeaderValue};
use actix_web::{HttpRequest, HttpResponse, Responder, delete, get, post, put, web};
use futures_util::StreamExt;
use serde_json::json;
use tokio::io::AsyncWriteExt;

use super::auth::{AccessAuth, VerifiedIdentity};
use super::config;
use super::error::AgenfactError;
use super::path::SafePath;
use super::private_index::PrivateIndexStore;

/// Ensure parent directories exist.
fn ensure_parent_dirs(path: &Path) -> Result<(), AgenfactError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            let message = format!("failed to create directories: {:?}", e);
            log::error!("{}, path: {}", message, path.display());
            AgenfactError::Internal {
                source: message,
                context: Some(format!("create directories for: {}", path.display())),
            }
        })?;
    }
    Ok(())
}

fn validate_path(path: web::Path<String>) -> Result<SafePath, AgenfactError> {
    let path = SafePath::from_user_input(Path::new(path.as_str()))?;
    if path
        .as_path()
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == config::UPLOAD_STAGING_DIR)
    {
        return Err(AgenfactError::NotFound {
            path: path.to_string(),
        });
    }
    Ok(path)
}

async fn save_file_field(mut payload: Multipart, full_path: &Path) -> Result<(), AgenfactError> {
    let mut found_file = false;

    while let Some(field) = payload.next().await {
        let mut field = field.map_err(|e| AgenfactError::BadRequest {
            reason: format!("invalid multipart payload: {}", e),
        })?;

        if field.name() != Some("file") {
            while let Some(chunk) = field.next().await {
                chunk.map_err(|e| AgenfactError::BadRequest {
                    reason: format!("invalid multipart field: {}", e),
                })?;
            }
            continue;
        }

        found_file = true;
        ensure_parent_dirs(full_path)?;
        let mut output = tokio::fs::File::create(full_path).await.map_err(|e| {
            let message = format!("failed to create file: {:?}", e);
            log::error!("multipart save error: {}", message);
            AgenfactError::Internal {
                source: message,
                context: Some("create uploaded file".to_string()),
            }
        })?;

        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|e| AgenfactError::BadRequest {
                reason: format!("invalid multipart file field: {}", e),
            })?;
            output.write_all(&data).await.map_err(|e| {
                let message = format!("failed to save file: {:?}", e);
                log::error!("multipart save error: {}", message);
                AgenfactError::Internal {
                    source: message,
                    context: Some("save uploaded file".to_string()),
                }
            })?;
        }

        output.flush().await.map_err(|e| {
            let message = format!("failed to flush file: {:?}", e);
            log::error!("multipart save error: {}", message);
            AgenfactError::Internal {
                source: message,
                context: Some("flush uploaded file".to_string()),
            }
        })?;
    }

    if !found_file {
        return Err(AgenfactError::BadRequest {
            reason: "multipart form is missing file field".to_string(),
        });
    }

    Ok(())
}

#[get("/files/{path:.*}")]
pub async fn get_file(
    req: HttpRequest,
    config: web::Data<config::Agenfact>,
    private_index: web::Data<Arc<PrivateIndexStore>>,
    path: web::Path<String>,
) -> Result<HttpResponse, AgenfactError> {
    let path = validate_path(path)?;
    let is_private = private_index
        .is_private(path.as_path())
        .await
        .map_err(|e| AgenfactError::store_error(e, "check private index"))?;

    if is_private {
        return Ok(HttpResponse::Found()
            .append_header(("Location", format!("/private-files/{}", path)))
            .finish());
    }

    open_upload_file(&req, &config, &path).await
}

#[get("/private-files/{path:.*}")]
pub async fn get_private_file(
    req: HttpRequest,
    config: web::Data<config::Agenfact>,
    private_index: web::Data<Arc<PrivateIndexStore>>,
    access_auth: web::Data<Arc<AccessAuth>>,
    path: web::Path<String>,
) -> Result<HttpResponse, AgenfactError> {
    let path = validate_path(path)?;
    let identity = VerifiedIdentity::from_request(&req, &access_auth)
        .await
        .map_err(|err| AgenfactError::Unauthorized {
            reason: err.message().to_string(),
        })?;

    let entry = private_index
        .get_entry(path.as_path())
        .await
        .map_err(|e| AgenfactError::store_error(e, "check private index"))?;

    match entry {
        Some(e) => {
            let email = identity.0.email.as_deref().unwrap_or("");
            if !e.authorized_emails.iter().any(|em| em == email) {
                log::warn!(
                    "private file access denied: email '{}' not authorized for path '{}'",
                    email,
                    path
                );
                return Err(AgenfactError::Forbidden {
                    reason: "email not authorized for this file".to_string(),
                });
            }
        }
        None => {
            log::warn!("accessing /private-files/ for non-private path: {}", path);
        }
    }

    log::info!(
        "private file access granted: sub={}, email={:?}, path={}",
        identity.0.sub,
        identity.0.email,
        path
    );

    open_upload_file(&req, &config, &path).await
}

async fn open_upload_file(
    request: &HttpRequest,
    config: &config::Agenfact,
    path: &SafePath,
) -> Result<HttpResponse, AgenfactError> {
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    if !full_path.exists() {
        return Err(AgenfactError::NotFound {
            path: path.to_string(),
        });
    }

    if !full_path.is_file() {
        return Err(AgenfactError::BadRequest {
            reason: format!("path is not a file: {}", path),
        });
    }

    let file = NamedFile::open_async(&full_path)
        .await
        .map_err(|e| AgenfactError::Internal {
            source: format!("failed to open file: {}", e),
            context: Some(format!("open file: {}", path)),
        })?;

    let mut response = file
        .set_content_disposition(ContentDisposition {
            disposition: DispositionType::Inline,
            parameters: Vec::new(),
        })
        .into_response(request);
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

#[post("/files/{path:.*}")]
pub async fn create_file(
    config: web::Data<config::Agenfact>,
    path: web::Path<String>,
    payload: Multipart,
) -> Result<impl Responder, AgenfactError> {
    let path = validate_path(path)?;
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    if full_path.exists() {
        return Err(AgenfactError::Conflict {
            path: path.to_string(),
        });
    }

    save_file_field(payload, &full_path).await?;

    Ok(HttpResponse::build(StatusCode::CREATED).json(json!({
        "message": "file created successfully"
    })))
}

#[put("/files/{path:.*}")]
pub async fn upsert_file(
    config: web::Data<config::Agenfact>,
    path: web::Path<String>,
    payload: Multipart,
) -> Result<impl Responder, AgenfactError> {
    let path = validate_path(path)?;
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));
    let file_exists = full_path.exists();

    save_file_field(payload, &full_path).await?;

    let status = if file_exists {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let message = if file_exists {
        "file updated successfully"
    } else {
        "file created successfully"
    };

    Ok(HttpResponse::build(status).json(json!({ "message": message })))
}

#[delete("/files/{path:.*}")]
pub async fn delete_file(
    config: web::Data<config::Agenfact>,
    path: web::Path<String>,
) -> Result<impl Responder, AgenfactError> {
    let path = validate_path(path)?;
    let full_path = config.build_full_upload_path(&PathBuf::from(path.as_path()));

    if !full_path.exists() {
        return Err(AgenfactError::NotFound {
            path: path.to_string(),
        });
    }

    if !full_path.is_file() {
        return Err(AgenfactError::BadRequest {
            reason: format!("path is not a file: {}", path),
        });
    }

    std::fs::remove_file(&full_path).map_err(|e| {
        let message = format!("failed to delete file: {:?}", e);
        log::error!("DELETE /files error: {}", message);
        AgenfactError::Internal {
            source: message,
            context: Some(format!("delete file: {}", path)),
        }
    })?;

    Ok(HttpResponse::Ok().json(json!({
        "message": "file deleted successfully"
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, http::header, test};

    use crate::test_utils::make_hs256_token;

    fn test_state() -> (
        config::Agenfact,
        Arc<PrivateIndexStore>,
        Arc<AccessAuth>,
        tempfile::TempDir,
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = config::Agenfact {
            uploads_path: temp_dir.path().to_string_lossy().to_string(),
            data_path: temp_dir.path().to_string_lossy().to_string(),
            ..config::Agenfact::default()
        };

        let private_index = Arc::new(PrivateIndexStore::new(&config));
        let access_auth = Arc::new(crate::auth::AccessAuth::from_parts(
            "https://issuer.example.com",
            "agenfact-app",
            Some("test-secret"),
        ));

        (config, private_index, access_auth, temp_dir)
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
    async fn create_file_success() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(create_file),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/files/test.txt")
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
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let content = std::fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
        assert_eq!(content, "test content");
    }

    #[actix_web::test]
    async fn create_file_with_nested_path() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(create_file),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/files/folder/subfolder/test.txt")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body(
                "test.txt",
                Some("text/plain"),
                "nested content",
            ))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let content =
            std::fs::read_to_string(temp_dir.path().join("folder/subfolder/test.txt")).unwrap();
        assert_eq!(content, "nested content");
    }

    #[actix_web::test]
    async fn create_file_already_exists() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("test.txt"), "content 1").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(create_file),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/files/test.txt")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body("test.txt", Some("text/plain"), "content 2"))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let content = std::fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
        assert_eq!(content, "content 1");
    }

    #[actix_web::test]
    async fn upsert_creates_new_file() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(upsert_file),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/files/test.txt")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body(
                "test.txt",
                Some("text/plain"),
                "new content",
            ))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::CREATED);
        let content = std::fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
        assert_eq!(content, "new content");
    }

    #[actix_web::test]
    async fn upsert_updates_existing_file() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("test.txt"), "original").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(upsert_file),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/files/test.txt")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body("test.txt", Some("text/plain"), "updated"))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::OK);
        let content = std::fs::read_to_string(temp_dir.path().join("test.txt")).unwrap();
        assert_eq!(content, "updated");
    }

    #[actix_web::test]
    async fn delete_file_success() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("test.txt"), "content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(delete_file),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/files/test.txt")
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!temp_dir.path().join("test.txt").exists());
    }

    #[actix_web::test]
    async fn delete_file_not_found() {
        let (config, private_index, access_auth, _temp_dir) = test_state();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(delete_file),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/files/nonexistent.txt")
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn rejects_parent_directory_traversal() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(create_file),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/files/../escape.txt")
            .insert_header((
                header::CONTENT_TYPE,
                "multipart/form-data; boundary=X-BOUNDARY",
            ))
            .set_payload(multipart_body("escape.txt", Some("text/plain"), "content"))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert!(matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
        ));
        assert!(!temp_dir.path().join("escape.txt").exists());
    }

    #[actix_web::test]
    async fn delete_directory_fails() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::create_dir(temp_dir.path().join("testdir")).unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(delete_file),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri("/files/testdir")
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn get_public_file_success() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("public.txt"), "public-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_file),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/files/public.txt")
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert_eq!(body, "public-content");
    }

    #[actix_web::test]
    async fn get_public_file_supports_byte_ranges() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("public.txt"), "public-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_file),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/files/public.txt")
            .insert_header((header::RANGE, "bytes=0-5"))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let body = test::read_body(response).await;
        assert_eq!(body, "public");
    }

    #[actix_web::test]
    async fn internal_upload_staging_files_are_not_served() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        let staging_dir = temp_dir.path().join(config::UPLOAD_STAGING_DIR);
        std::fs::create_dir(&staging_dir).unwrap();
        std::fs::write(staging_dir.join("partial.txt"), "partial-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_file),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/files/{}/partial.txt", config::UPLOAD_STAGING_DIR))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn get_private_file_redirects_to_private_prefix() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("secret.txt"), "secret-content").unwrap();
        private_index
            .mark_private(&PathBuf::from("secret.txt"), vec![])
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_file),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/files/secret.txt")
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/private-files/secret.txt"
        );
    }

    #[actix_web::test]
    async fn private_files_requires_access_jwt_header() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        std::fs::write(temp_dir.path().join("secret.txt"), "secret-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_private_file),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/private-files/secret.txt")
            .to_request();
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn private_files_with_valid_hs256_jwt_returns_200() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        private_index
            .mark_private(
                &PathBuf::from("secret.txt"),
                vec!["allowed@example.com".to_string()],
            )
            .await
            .unwrap();
        std::fs::write(temp_dir.path().join("secret.txt"), "secret-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_private_file),
        )
        .await;

        let token = make_hs256_token(
            "test-secret",
            "user-1",
            Some("allowed@example.com"),
            &["team-a"],
            "https://issuer.example.com",
            "agenfact-app",
            3600,
        );
        let req = test::TestRequest::get()
            .uri("/private-files/secret.txt")
            .insert_header(("Cf-Access-Jwt-Assertion", token))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body = test::read_body(response).await;
        assert_eq!(body, "secret-content");
    }

    #[actix_web::test]
    async fn private_files_with_disallowed_email_returns_403() {
        let (config, private_index, access_auth, temp_dir) = test_state();
        private_index
            .mark_private(
                &PathBuf::from("secret.txt"),
                vec!["only@example.com".to_string()],
            )
            .await
            .unwrap();
        std::fs::write(temp_dir.path().join("secret.txt"), "secret-content").unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(config))
                .app_data(web::Data::new(private_index))
                .app_data(web::Data::new(access_auth))
                .service(get_private_file),
        )
        .await;

        let token = make_hs256_token(
            "test-secret",
            "user-1",
            Some("blocked@example.com"),
            &["team-a"],
            "https://issuer.example.com",
            "agenfact-app",
            3600,
        );
        let req = test::TestRequest::get()
            .uri("/private-files/secret.txt")
            .insert_header(("Cf-Access-Jwt-Assertion", token))
            .to_request();
        let response = test::call_service(&app, req).await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
