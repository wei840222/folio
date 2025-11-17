use std::path::PathBuf;

use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{Serialize, json::Json};

use super::config;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct FileResponse {
    message: String,
}

#[post("/<path..>", data = "<file>", rank = 5)]
pub async fn create_file(
    config: &State<config::Folio>,
    path: PathBuf,
    mut file: Form<Strict<TempFile<'_>>>,
) -> Result<Custom<Json<FileResponse>>, Custom<Json<FileResponse>>> {
    // Validate path
    if path.to_string_lossy().contains("..") {
        let error_message = format!("invalid file path: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Build full file path
    let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(&config.uploads_path)
        .join(&path);

    // Check if file already exists
    if full_path.exists() {
        let error_message = format!("file already exists: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::Conflict,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Create parent directories if they don't exist
    if let Some(parent) = full_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            let error_message = format!(
                "failed to create directories for path {}: {}",
                path.display(),
                e
            );
            log::error!("{}", error_message);
            return Err(Custom(
                Status::InternalServerError,
                Json(FileResponse {
                    message: error_message,
                }),
            ));
        }
    }

    // Persist file
    if let Err(e) = file.persist_to(&full_path).await {
        let error_message = format!("failed to save file: {}", e);
        log::error!("{}", error_message);
        return Err(Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    Ok(Custom(
        Status::Created,
        Json(FileResponse {
            message: format!("file created successfully"),
        }),
    ))
}

#[put("/<path..>", data = "<file>", rank = 5)]
pub async fn upsert_file(
    config: &State<config::Folio>,
    path: PathBuf,
    mut file: Form<Strict<TempFile<'_>>>,
) -> Result<Custom<Json<FileResponse>>, Custom<Json<FileResponse>>> {
    // Validate path
    if path.to_string_lossy().contains("..") {
        let error_message = format!("invalid file path: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Build full file path
    let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(&config.uploads_path)
        .join(&path);

    let file_exists = full_path.exists();

    // Create parent directories if they don't exist
    if let Some(parent) = full_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            let error_message = format!(
                "failed to create directories for path {}: {}",
                path.display(),
                e
            );
            log::error!("{}", error_message);
            return Err(Custom(
                Status::InternalServerError,
                Json(FileResponse {
                    message: error_message,
                }),
            ));
        }
    }

    // Persist file (overwrites if exists)
    if let Err(e) = file.persist_to(&full_path).await {
        let error_message = format!("failed to save file: {}", e);
        log::error!("{}", error_message);
        return Err(Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    if file_exists {
        Ok(Custom(
            Status::Ok,
            Json(FileResponse {
                message: format!("file updated successfully"),
            }),
        ))
    } else {
        Ok(Custom(
            Status::Created,
            Json(FileResponse {
                message: format!("file created successfully"),
            }),
        ))
    }
}

#[delete("/<path..>", rank = 5)]
pub async fn delete_file(
    config: &State<config::Folio>,
    path: PathBuf,
) -> Result<Custom<Json<FileResponse>>, Custom<Json<FileResponse>>> {
    // Validate path
    if path.to_string_lossy().contains("..") {
        let error_message = format!("invalid file path: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Build full file path
    let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(&config.uploads_path)
        .join(&path);

    // Check if file exists
    if !full_path.exists() {
        let error_message = format!("file not found: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::NotFound,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Check if it's a file (not a directory)
    if !full_path.is_file() {
        let error_message = format!("path is not a file: {}", path.display());
        log::warn!("{}", error_message);
        return Err(Custom(
            Status::BadRequest,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    // Delete file
    if let Err(e) = std::fs::remove_file(&full_path) {
        let error_message = format!("failed to delete file: {}", e);
        log::error!("{}", error_message);
        return Err(Custom(
            Status::InternalServerError,
            Json(FileResponse {
                message: error_message,
            }),
        ));
    }

    Ok(Custom(
        Status::Ok,
        Json(FileResponse {
            message: format!("file deleted successfully"),
        }),
    ))
}
