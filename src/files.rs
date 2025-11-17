use std::path::PathBuf;

use rocket::State;
use rocket::form::{Form, Strict};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::{Created, Custom};
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
) -> Result<Created<Json<FileResponse>>, Custom<Json<FileResponse>>> {
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

    Ok(
        Created::new(format!("/files/{}", path.display())).body(Json(FileResponse {
            message: format!("file created successfully"),
        })),
    )
}
