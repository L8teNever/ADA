mod api;
mod auth;
mod config;
mod games;
mod rooms;
mod users;
mod ws;

use std::{sync::Arc, time::Duration};

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tower::ServiceExt;
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};

use config::Config;
use rooms::Hub;
use users::Users;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub users: Arc<Users>,
    pub hub: Arc<Hub>,
}

#[derive(Clone)]
struct Dispatch {
    play_hosts: Vec<String>,
    portal: Router,
    play: Router,
}

/// Wählt anhand des Hosts die passende Webseite.
async fn dispatch(State(d): State<Dispatch>, req: Request) -> Response {
    if req.uri().path() == "/healthz" {
        return "ok".into_response();
    }
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.uri().host().map(str::to_string))
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let router = if d.play_hosts.contains(&host) { d.play.clone() } else { d.portal.clone() };
    match router.oneshot(req).await {
        Ok(r) => r,
        Err(e) => match e {},
    }
}

async fn play_index(State(st): State<AppState>) -> Response {
    match tokio::fs::read_to_string(st.cfg.web_dir.join("play/index.html")).await {
        Ok(s) => Html(s).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn portal_router(st: AppState) -> Router {
    let web = st.cfg.web_dir.clone();
    Router::new()
        .route("/api/me", get(api::me).put(api::rename))
        .route("/api/games", get(api::games))
        .route("/api/rooms", get(api::my_rooms).post(api::create_room))
        .route("/api/rooms/{token}", get(api::room_info).delete(api::delete_room))
        .route("/ws/{token}", get(ws::portal_ws))
        .nest_service("/shared", ServeDir::new(web.join("shared")))
        .fallback_service(ServeDir::new(web.join("portal")))
        .with_state(st)
}

fn play_router(st: AppState) -> Router {
    let web = st.cfg.web_dir.clone();
    Router::new()
        .route("/", get(play_index))
        .route("/{token}", get(play_index))
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/api/rooms/{token}", get(api::public_room_info))
        .route("/ws/{token}", get(ws::play_ws))
        .nest_service("/shared", ServeDir::new(web.join("shared")))
        .nest_service("/assets", ServeDir::new(web.join("play")))
        .with_state(st)
}

/// `ada healthcheck` – für den Docker-HEALTHCHECK (ohne curl im Image).
fn healthcheck() -> i32 {
    use std::io::{Read, Write};
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let Ok(mut s) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return 1;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = s.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n");
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    if buf.starts_with("HTTP/1.1 200") || buf.starts_with("HTTP/1.0 200") {
        0
    } else {
        1
    }
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(healthcheck());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = Arc::new(Config::from_env());
    let st = AppState {
        users: Arc::new(Users::load(&cfg.data_dir)),
        hub: Arc::new(Hub::new()),
        cfg: cfg.clone(),
    };

    let sweeper = st.hub.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            sweeper.sweep();
        }
    });

    let dispatch_state = Dispatch {
        play_hosts: cfg.play_hosts.clone(),
        portal: portal_router(st.clone()),
        play: play_router(st.clone()),
    };

    let header_layer = |name: header::HeaderName, value: &'static str| {
        SetResponseHeaderLayer::if_not_present(name, HeaderValue::from_static(value))
    };
    let app = Router::new()
        .fallback(dispatch)
        .with_state(dispatch_state)
        .layer(header_layer(header::CACHE_CONTROL, "no-cache"))
        .layer(header_layer(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .layer(header_layer(header::REFERRER_POLICY, "same-origin"))
        .layer(header_layer(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self' ws: wss:; frame-ancestors 'none'",
        ));

    let addr = format!("0.0.0.0:{}", cfg.port);
    tracing::info!(
        "ADA läuft auf {addr} (Portal: {}, Spiele: {})",
        cfg.portal_host,
        cfg.play_hosts.join(", ")
    );
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Port belegt");
    axum::serve(listener, app).await.expect("Server beendet");
}
