use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    /// Host des Portals (hinter Cloudflare Zero Trust), z. B. ada.kulbarts.com
    pub portal_host: String,
    /// Host der öffentlichen Spieleseite, z. B. gameada.kulbarts.com
    pub play_host: String,
    /// Basis-URL für Einladungslinks, z. B. https://gameada.kulbarts.com
    pub play_base_url: String,
    /// Nur für lokale Entwicklung: tut so, als wäre diese Mail angemeldet.
    pub dev_email: Option<String>,
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let play_host = env("PLAY_HOST", "gameada.kulbarts.com").to_ascii_lowercase();
        let play_base_url = env("PLAY_BASE_URL", &format!("https://{play_host}"))
            .trim_end_matches('/')
            .to_string();
        Config {
            port: env("PORT", "8080").parse().unwrap_or(8080),
            data_dir: PathBuf::from(env("DATA_DIR", "/data")),
            web_dir: PathBuf::from(env("WEB_DIR", "/app/web")),
            portal_host: env("PORTAL_HOST", "ada.kulbarts.com").to_ascii_lowercase(),
            play_host,
            play_base_url,
            dev_email: std::env::var("DEV_EMAIL")
                .ok()
                .map(|v| v.trim().to_ascii_lowercase())
                .filter(|v| !v.is_empty()),
        }
    }
}
