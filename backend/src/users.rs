use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u32,
    pub email: String,
    pub name: String,
    pub created: u64,
}

/// Einfacher Nutzerspeicher: pro Mail ein Account, abgelegt in users.json.
pub struct Users {
    path: PathBuf,
    inner: Mutex<Vec<User>>,
}

pub fn clean_name(raw: &str) -> Result<String, String> {
    let name: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name: String = name.chars().take(20).collect();
    if name.trim().is_empty() {
        Err("Bitte gib einen Namen ein".into())
    } else {
        Ok(name)
    }
}

fn default_name(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("Spieler");
    let words: Vec<String> = local
        .split(|c: char| c == '.' || c == '_' || c == '-' || c == '+')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect();
    clean_name(&words.join(" ")).unwrap_or_else(|_| "Spieler".into())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Users {
    pub fn load(dir: &Path) -> Self {
        let _ = std::fs::create_dir_all(dir);
        let path = dir.join("users.json");
        let list: Vec<User> = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Users {
            path,
            inner: Mutex::new(list),
        }
    }

    fn save(&self, list: &[User]) {
        let tmp = self.path.with_extension("json.tmp");
        match serde_json::to_vec_pretty(list) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &self.path)) {
                    tracing::error!("users.json konnte nicht gespeichert werden: {e}");
                }
            }
            Err(e) => tracing::error!("users serialisieren: {e}"),
        }
    }

    /// Account zur Mail holen oder automatisch anlegen.
    pub fn get_or_create(&self, email: &str) -> User {
        let mut list = self.inner.lock().unwrap();
        if let Some(u) = list.iter().find(|u| u.email == email) {
            return u.clone();
        }
        let id = list.iter().map(|u| u.id).max().unwrap_or(0) + 1;
        let user = User {
            id,
            email: email.to_string(),
            name: default_name(email),
            created: now(),
        };
        list.push(user.clone());
        self.save(&list);
        tracing::info!("neuer Account: {email}");
        user
    }

    pub fn rename(&self, email: &str, name: &str) -> Result<User, String> {
        let name = clean_name(name)?;
        let mut list = self.inner.lock().unwrap();
        let user = list.iter_mut().find(|u| u.email == email).ok_or("Account nicht gefunden")?;
        user.name = name;
        let out = user.clone();
        self.save(&list);
        Ok(out)
    }
}
