use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ts() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
}

pub fn make_hs256_token(
    secret: &str,
    sub: &str,
    email: Option<&str>,
    groups: &[&str],
    iss: &str,
    aud: &str,
    exp_offset_secs: i64,
) -> String {
    make_hs256_token_with_aud(secret, sub, email, groups, iss, aud, exp_offset_secs)
}

pub fn make_hs256_token_with_aud_array(
    secret: &str,
    sub: &str,
    email: Option<&str>,
    groups: &[&str],
    iss: &str,
    aud: &[&str],
    exp_offset_secs: i64,
) -> String {
    let exp = if exp_offset_secs >= 0 {
        now_ts() + exp_offset_secs as usize
    } else {
        now_ts().saturating_sub((-exp_offset_secs) as usize)
    };

    let claims = rocket::serde::json::json!({
        "sub": sub,
        "email": email,
        "groups": groups,
        "iss": iss,
        "aud": aud,
        "exp": exp,
    });

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

fn make_hs256_token_with_aud(
    secret: &str,
    sub: &str,
    email: Option<&str>,
    groups: &[&str],
    iss: &str,
    aud: &str,
    exp_offset_secs: i64,
) -> String {
    let exp = if exp_offset_secs >= 0 {
        now_ts() + exp_offset_secs as usize
    } else {
        now_ts().saturating_sub((-exp_offset_secs) as usize)
    };

    let claims = rocket::serde::json::json!({
        "sub": sub,
        "email": email,
        "groups": groups,
        "iss": iss,
        "aud": aud,
        "exp": exp,
    });

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}
