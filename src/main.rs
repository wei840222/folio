mod auth;
mod config;
mod couchdb;
mod error;
mod expiry;
mod files;
mod path;
mod private_index;
mod store;
#[cfg(test)]
mod test_utils;
mod upload_protection;
mod uploads;

use std::sync::Arc;
use std::time::Duration;

use actix_files::Files;
use actix_web::{App, HttpResponse, HttpServer, get, web};
use figment::Figment;
use figment::providers::{Env, Format, Serialized, Toml};

#[get("/health")]
async fn health() -> &'static str {
    "OK"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    pretty_env_logger::init();

    let mut config = load_config();
    apply_rocket_compat_env(&mut config);
    config
        .validate()
        .map_err(|reason| std::io::Error::new(std::io::ErrorKind::InvalidInput, reason))?;
    log::info!("Using config: {:?}", config);

    // Ensure runtime data directories exist
    let uploads_dir = config.resolve_base(&config.uploads_path);
    let data_dir = config.resolve_base(&config.data_path);
    std::fs::create_dir_all(&uploads_dir).unwrap_or_else(|e| {
        panic!(
            "Failed to create uploads directory {}: {}",
            uploads_dir.display(),
            e
        )
    });
    uploads::cleanup_staging_dir(&config).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!(
                "failed to clean upload staging directory under {}: {}",
                uploads_dir.display(),
                error
            ),
        )
    })?;
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        panic!(
            "Failed to create data directory {}: {}",
            data_dir.display(),
            e
        )
    });

    let expiry_store = Arc::new(expiry::ExpiryStore::new(&config));
    let private_index_store = Arc::new(private_index::PrivateIndexStore::new(&config));
    let removed_expiry_entries = expiry_store
        .reconcile_missing_files()
        .await
        .map_err(std::io::Error::other)?;
    let removed_private_entries = private_index_store
        .reconcile_missing_files()
        .await
        .map_err(std::io::Error::other)?;
    if removed_expiry_entries > 0 || removed_private_entries > 0 {
        log::warn!(
            "startup removed {} expiry and {} private metadata entries without published files",
            removed_expiry_entries,
            removed_private_entries
        );
    }
    expiry_store.clone().spawn_sweeper(Duration::from_secs(60));

    let access_auth = Arc::new(auth::AccessAuth::from_env());
    let upload_protection = Arc::new(
        upload_protection::UploadProtection::from_env()
            .map_err(|reason| std::io::Error::new(std::io::ErrorKind::InvalidInput, reason))?,
    );

    let couchdb_store = couchdb::CouchDbStore::new(&config).map(Arc::new);
    if let Some(ref cs) = couchdb_store {
        log::info!("Initializing CouchDB backend at {:?}", config.couchdb_url);
        cs.init_db()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        cs.clone().spawn_sweeper(Duration::from_secs(60));
    }

    let bind = (config.address.clone(), config.port);
    let web_path = config.web_path.clone();

    HttpServer::new(move || {
        let mut app = App::new()
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(expiry_store.clone()))
            .app_data(web::Data::new(private_index_store.clone()))
            .app_data(web::Data::new(access_auth.clone()))
            .app_data(web::Data::new(upload_protection.clone()));

        if let Some(ref cs) = couchdb_store {
            app = app.app_data(web::Data::new(cs.clone()));
        }

        app.service(health)
            .service(uploads::upload_file)
            .service(files::get_file)
            .service(files::create_file)
            .service(files::upsert_file)
            .service(files::delete_file)
            .service(files::get_private_file)
            .service(
                Files::new("/", web_path.clone())
                    .index_file("index.html")
                    .default_handler(web::to(|| async { HttpResponse::NotFound().finish() })),
            )
    })
    .bind(bind)?
    .run()
    .await
}

fn load_config() -> config::Agenfact {
    Figment::from(Serialized::defaults(config::Agenfact::default()))
        .merge(Toml::file("Agenfact.toml"))
        .merge(Env::prefixed("AGENFACT_").global())
        .extract()
        .unwrap()
}

fn apply_rocket_compat_env(config: &mut config::Agenfact) {
    if std::env::var_os("AGENFACT_ADDRESS").is_none()
        && let Ok(address) = std::env::var("ROCKET_ADDRESS")
    {
        config.address = address;
    }

    if std::env::var_os("AGENFACT_PORT").is_none()
        && let Ok(port) = std::env::var("ROCKET_PORT")
        && let Ok(port) = port.parse()
    {
        config.port = port;
    }
}
