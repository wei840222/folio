#[macro_use]
extern crate rocket;

#[get("/")]
fn health() -> &'static str {
    "OK"
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/health", routes![health])
        .mount("/files", rocket::fs::FileServer::from("./uploads").rank(5))
        .mount("/", rocket::fs::FileServer::from("./web/dist"))
}
