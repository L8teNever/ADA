use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    Json,
};
use serde_json::{json, Value};

use crate::{users::User, AppState};

/// Angemeldeter Account. Cloudflare Access setzt nach dem Login den Header
/// `Cf-Access-Authenticated-User-Email`; daraus wird der Account abgeleitet.
/// Wichtig: nur auf dem Portal-Host benutzen und den Container nie direkt
/// ins Internet stellen, sonst könnte jemand den Header selbst setzen.
pub struct Account(pub User);

pub const EMAIL_HEADER: &str = "cf-access-authenticated-user-email";

fn valid_email(s: &str) -> bool {
    s.len() <= 254 && s.contains('@') && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}

impl FromRequestParts<AppState> for Account {
    type Rejection = (StatusCode, Json<Value>);

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let from_header = parts
            .headers
            .get(EMAIL_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_ascii_lowercase());
        let email = from_header.or_else(|| state.cfg.dev_email.clone());
        match email {
            Some(e) if valid_email(&e) => Ok(Account(state.users.get_or_create(&e))),
            _ => Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Nicht angemeldet (Zero Trust Login fehlt)"})),
            )),
        }
    }
}
