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

/// Ensure parent directories exist
fn ensure_parent_dirs(path: &PathBuf) -> Result<(), Custom<Json<FileResponse>>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            let message = format!("failed to create directories: {}", e);
            log::error!("{}, path: {}", e, path.to_string_lossy());
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
    file.persist_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {}", e);
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
    file.persist_to(&full_path).await.map_err(|e| {
        let message = format!("failed to save file: {}", e);
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
        let message = format!("failed to delete file: {}", e);
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
