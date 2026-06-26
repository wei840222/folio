mod auth;
mod config;
mod error;
mod expiry;
mod files;
mod path;
mod private_index;
mod store;
#[cfg(test)]
mod test_utils;
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
    log::info!("Using config: {:?}", config);

    // Ensure runtime data directories exist
    let uploads_dir = config.resolve_base(&config.uploads_path);
    let data_dir = config.resolve_base(&config.data_path);
    std::fs::create_dir_all(&uploads_dir).unwrap_or_else(|e| {
        panic!("Failed to create uploads directory {}: {}", uploads_dir.display(), e)
    });
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        panic!("Failed to create data directory {}: {}", data_dir.display(), e)
    });

    let expiry_store = Arc::new(expiry::ExpiryStore::new(&config));
    expiry_store.clone().spawn_sweeper(Duration::from_secs(60));

    let private_index_store = Arc::new(private_index::PrivateIndexStore::new(&config));
    let access_auth = Arc::new(auth::AccessAuth::from_env());

    let bind = (config.address.clone(), config.port);
    let web_path = config.web_path.clone();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config::Folio {
                address: config.address.clone(),
                port: config.port,
                web_path: config.web_path.clone(),
                uploads_path: config.uploads_path.clone(),
                data_path: config.data_path.clone(),
                max_upload_size: config.max_upload_size,
            }))
            .app_data(web::Data::new(expiry_store.clone()))
            .app_data(web::Data::new(private_index_store.clone()))
            .app_data(web::Data::new(access_auth.clone()))
            .service(health)
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

fn load_config() -> config::Folio {
    Figment::from(Serialized::defaults(config::Folio::default()))
        .merge(Toml::file("Folio.toml"))
        .merge(Env::prefixed("FOLIO_").global())
        .extract()
        .unwrap()
}

fn apply_rocket_compat_env(config: &mut config::Folio) {
    if std::env::var_os("FOLIO_ADDRESS").is_none()
        && let Ok(address) = std::env::var("ROCKET_ADDRESS")
    {
        config.address = address;
    }

    if std::env::var_os("FOLIO_PORT").is_none()
        && let Ok(port) = std::env::var("ROCKET_PORT")
        && let Ok(port) = port.parse()
    {
        config.port = port;
    }
}
