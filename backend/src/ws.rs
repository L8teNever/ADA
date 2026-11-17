use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;

use crate::{
    auth::Account,
    rooms::{ClientMsg, Hub},
    users::clean_name,
    AppState,
};

enum Ident {
    /// Über Zero Trust angemeldet: Account steht fest
    Account { email: String, name: String },
    /// Gast über den öffentlichen Link: Name + Schlüssel kommen vom Client
    Guest,
}

/// Schutz vor fremden Webseiten: Origin muss zum Host passen.
fn origin_ok(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("");
    origin
        .split("://")
        .nth(1)
        .map_or(false, |o| o.eq_ignore_ascii_case(host))
}

pub async fn portal_ws(
    State(st): State<AppState>,
    Account(user): Account,
    Path(token): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let ident = Ident::Account {
        email: user.email,
        name: user.name,
    };
    let hub = st.hub.clone();
    ws.on_upgrade(move |socket| run_socket(hub, token, socket, ident))
}

pub async fn play_ws(
    State(st): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let hub = st.hub.clone();
    ws.on_upgrade(move |socket| run_socket(hub, token, socket, Ident::Guest))
}

async fn run_socket(hub: Arc<Hub>, token: String, socket: WebSocket, ident: Ident) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(m) = rx.recv().await {
            if sink.send(Message::Text(m.into())).await.is_err() {
                break;
            }
        }
    });

    let conn = hub.next_conn();
    let mut pid = None;
    let fail = |tx: &mpsc::UnboundedSender<String>, message: &str| {
        let _ = tx.send(json!({"type": "error", "fatal": true, "message": message}).to_string());
    };

    loop {
        let next = if pid.is_none() {
            match tokio::time::timeout(Duration::from_secs(20), stream.next()).await {
                Ok(v) => v,
                Err(_) => break,
            }
        } else {
            stream.next().await
        };
        let Some(Ok(msg)) = next else { break };
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let Ok(parsed) = serde_json::from_str::<ClientMsg>(text.as_str()) else {
            continue;
        };

        match (pid, parsed) {
            (None, ClientMsg::Join { name, key }) => {
                let (key, name, account) = match &ident {
                    Ident::Account { email, name } => (format!("acct:{email}"), name.clone(), true),
                    Ident::Guest => {
                        let key = key.unwrap_or_default();
                        let valid = (16..=64).contains(&key.len()) && key.chars().all(|c| c.is_ascii_alphanumeric());
                        if !valid {
                            fail(&tx, "Ungültige Sitzung – lade die Seite neu");
                            break;
                        }
                        match clean_name(&name.unwrap_or_default()) {
                            Ok(n) => (format!("guest:{key}"), n, false),
                            Err(e) => {
                                fail(&tx, &e);
                                break;
                            }
                        }
                    }
                };
                match hub.join(&token, key, name, account, conn, tx.clone()) {
                    Ok(id) => pid = Some(id),
                    Err(e) => {
                        fail(&tx, &e);
                        break;
                    }
                }
            }
            (Some(id), m) => {
                let leaving = matches!(m, ClientMsg::Leave);
                hub.handle(&token, id, m);
                if leaving {
                    break;
                }
            }
            _ => {}
        }
    }

    if let Some(id) = pid {
        hub.disconnect(&token, id, conn);
    }
    drop(tx);
    // kurz Zeit lassen, damit letzte Nachrichten (z. B. Fehler) rausgehen
    let _ = tokio::time::timeout(Duration::from_millis(300), writer).await;
}
