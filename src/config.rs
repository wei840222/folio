use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct Folio {
    pub web_path: String,
    pub uploads_path: String,
    pub garbage_collection_pattern: Vec<String>,
}

impl<'r> Default for Folio {
    fn default() -> Folio {
        Folio {
            web_path: String::from("./web/dist"),
            uploads_path: String::from("./uploads"),
            garbage_collection_pattern: vec![
                String::from(r#"^\._.+"#),
                String::from(r#"^\.DS_Store$"#),
            ],
        }
    }
}
