use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{auth::Account, games::GameInfo, AppState};

type ApiError = (StatusCode, Json<Value>);
type ApiResult<T> = Result<Json<T>, ApiError>;

fn err(code: StatusCode, msg: &str) -> ApiError {
    (code, Json(json!({"error": msg})))
}

fn with_link(st: &AppState, mut info: Value) -> Value {
    if let Some(token) = info.get("token").and_then(Value::as_str).map(str::to_string) {
        info["link"] = json!(format!("{}/{}", st.cfg.play_base_url, token));
    }
    info
}

// ---------- Portal (Zero Trust) ----------

pub async fn me(Account(user): Account) -> Json<Value> {
    Json(json!({"id": user.id, "email": user.email, "name": user.name}))
}

#[derive(Deserialize)]
pub struct RenameBody {
    name: String,
}

pub async fn rename(State(st): State<AppState>, Account(user): Account, Json(b): Json<RenameBody>) -> ApiResult<Value> {
    let u = st
        .users
        .rename(&user.email, &b.name)
        .map_err(|e| err(StatusCode::BAD_REQUEST, &e))?;
    Ok(Json(json!({"id": u.id, "email": u.email, "name": u.name})))
}

pub async fn games(State(st): State<AppState>, _a: Account) -> Json<Vec<GameInfo>> {
    Json(st.hub.games())
}

#[derive(Deserialize)]
pub struct CreateBody {
    game: String,
    #[serde(default)]
    options: Value,
}

pub async fn create_room(State(st): State<AppState>, Account(user): Account, Json(b): Json<CreateBody>) -> ApiResult<Value> {
    let token = st
        .hub
        .create_room(&b.game, &user.email, b.options)
        .map_err(|e| err(StatusCode::BAD_REQUEST, &e))?;
    Ok(Json(json!({
        "token": token,
        "link": format!("{}/{}", st.cfg.play_base_url, token),
    })))
}

pub async fn my_rooms(State(st): State<AppState>, Account(user): Account) -> Json<Vec<Value>> {
    Json(
        st.hub
            .rooms_of(&user.email)
            .into_iter()
            .map(|i| with_link(&st, i))
            .collect(),
    )
}

pub async fn room_info(State(st): State<AppState>, _a: Account, Path(token): Path<String>) -> ApiResult<Value> {
    match st.hub.info(&token) {
        Some(i) => Ok(Json(with_link(&st, i))),
        None => Err(err(StatusCode::NOT_FOUND, "Raum nicht gefunden")),
    }
}

pub async fn delete_room(State(st): State<AppState>, Account(user): Account, Path(token): Path<String>) -> ApiResult<Value> {
    if st.hub.delete_room(&token, &user.email) {
        Ok(Json(json!({"ok": true})))
    } else {
        Err(err(StatusCode::NOT_FOUND, "Raum nicht gefunden"))
    }
}

// ---------- Öffentliche Spieleseite ----------

pub async fn public_room_info(State(st): State<AppState>, Path(token): Path<String>) -> ApiResult<Value> {
    match st.hub.info(&token) {
        Some(i) => Ok(Json(i)),
        None => Err(err(StatusCode::NOT_FOUND, "Raum nicht gefunden")),
    }
}
