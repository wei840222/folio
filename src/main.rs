mod config;
mod files;
mod uploads;

#[macro_use]
extern crate rocket;
extern crate log;
extern crate pretty_env_logger;

use figment::providers::{Env, Format, Serialized, Toml};
use rocket::fairing::AdHoc;
use rocket::fs::FileServer;

#[get("/")]
fn health() -> &'static str {
    "OK"
}

#[launch]
fn rocket() -> _ {
    pretty_env_logger::init();
    let figment = rocket::Config::figment()
        .merge(Serialized::defaults(config::Folio::default()))
        .merge(Toml::file("Folio.toml").nested())
        .merge(Env::prefixed("FOLIO_").global());

    let config: config::Folio = figment.extract().unwrap();
    log::info!("Using config: {:?}", config);

    rocket::custom(figment)
        .mount("/health", routes![health])
        .mount("/uploads", routes![uploads::upload_file])
        .mount("/files", routes![files::create_file])
        .mount("/files", FileServer::from(config.uploads_path).rank(5))
        .mount("/", FileServer::from(config.web_path))
        .attach(AdHoc::config::<config::Folio>())
}
