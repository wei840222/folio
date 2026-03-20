mod config;
mod expiry;
mod files;
mod uploads;

use figment::providers::{Env, Format, Serialized, Toml};
use rocket::fs::FileServer;
use std::sync::Arc;
use std::time::Duration;

#[macro_use]
extern crate rocket;
extern crate log;
extern crate pretty_env_logger;

#[get("/")]
fn health() -> &'static str {
    "OK"
}

#[launch]
async fn rocket() -> _ {
    pretty_env_logger::init();
    let figment = rocket::Config::figment()
        .merge(Serialized::defaults(config::Folio::default()))
        .merge(Toml::file("Folio.toml").nested())
        .merge(Env::prefixed("FOLIO_").global());

    let config: config::Folio = figment.extract().unwrap();
    log::info!("Using config: {:?}", config);

    let expiry_store = Arc::new(expiry::ExpiryStore::new(&config));
    expiry_store.clone().spawn_sweeper(Duration::from_secs(60));

    rocket::custom(figment)
        .mount("/health", routes![health])
        .mount("/uploads", routes![uploads::upload_file])
        .mount(
            "/files",
            routes![files::create_file, files::upsert_file, files::delete_file],
        )
        .mount(
            "/files",
            FileServer::from(config.uploads_path.to_string()).rank(5),
        )
        .mount("/", FileServer::from(config.web_path.to_string()))
        .manage(config)
        .manage(expiry_store)
}
