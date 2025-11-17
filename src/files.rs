use std::ops::Deref;
use std::path::PathBuf;

use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::http::uri::Segments;
use rocket::request::FromSegments;
use rocket::response::status::Custom;
use rocket::serde::{Serialize, json::Json};

use super::config;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FileResponse {
    message: String,
}

type FileResult = Result<Custom<Json<FileResponse>>, Custom<Json<FileResponse>>>;

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
            Err("path contains '..'")
        } else {
            Ok(ValidatedPath(path))
        }
    }
}

/// Helper to create error response
fn error_response(status: Status, message: String) -> Custom<Json<FileResponse>> {
    Custom(status, Json(FileResponse { message }))
}

/// Helper to create success response
fn success_response(status: Status, message: &str) -> Custom<Json<FileResponse>> {
    Custom(
        status,
        Json(FileResponse {
            message: message.into(),
        }),
    )
}

/// Ensure parent directories exist
fn ensure_parent_dirs(
    path: &PathBuf,
    display_path: &PathBuf,
) -> Result<(), Custom<Json<FileResponse>>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            log::error!(
                "failed to create directories for path {}: {}",
                display_path.display(),
                e
            );
            error_response(
                Status::InternalServerError,
                format!(
                    "failed to create directories for path {}: {}",
                    display_path.display(),
                    e
                ),
            )
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
        log::warn!("file already exists: {}", path.display());
        return Err(error_response(
            Status::Conflict,
            format!("file already exists: {}", path.display()),
        ));
    }

    ensure_parent_dirs(&full_path, &path)?;

    // Persist file
    file.persist_to(&full_path).await.map_err(|e| {
        log::error!("failed to save file: {}", e);
        error_response(
            Status::InternalServerError,
            format!("failed to save file: {}", e),
        )
    })?;

    Ok(success_response(
        Status::Created,
        "file created successfully",
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

    ensure_parent_dirs(&full_path, &path)?;

    // Persist file (overwrites if exists)
    file.persist_to(&full_path).await.map_err(|e| {
        log::error!("failed to save file: {}", e);
        error_response(
            Status::InternalServerError,
            format!("failed to save file: {}", e),
        )
    })?;

    if file_exists {
        log::info!("file updated: {}", path.display());
        Ok(success_response(Status::Ok, "file updated successfully"))
    } else {
        log::info!("file created: {}", path.display());
        Ok(success_response(
            Status::Created,
            "file created successfully",
        ))
    }
}

#[delete("/<path..>", rank = 5)]
pub async fn delete_file(config: &State<config::Folio>, path: ValidatedPath) -> FileResult {
    let full_path = config.build_full_upload_path(&path);

    // Check if file exists
    if !full_path.exists() {
        log::warn!("file not found: {}", path.display());
        return Err(error_response(
            Status::NotFound,
            format!("file not found: {}", path.display()),
        ));
    }

    // Check if it's a file (not a directory)
    if !full_path.is_file() {
        log::warn!("path is not a file: {}", path.display());
        return Err(error_response(
            Status::BadRequest,
            format!("path is not a file: {}", path.display()),
        ));
    }

    // Delete file
    std::fs::remove_file(&full_path).map_err(|e| {
        log::error!("failed to delete file: {}", e);
        error_response(
            Status::InternalServerError,
            format!("failed to delete file: {}", e),
        )
    })?;

    log::info!("file deleted: {}", path.display());
    Ok(success_response(Status::Ok, "file deleted successfully"))
}
