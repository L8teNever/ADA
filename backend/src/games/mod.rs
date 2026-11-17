//! Spiele-Plugins.
//!
//! Ein neues Spiel besteht aus:
//!   1. einer Rust-Datei hier (implementiert `GameKind` + `Game`),
//!   2. einem Eintrag in `registry()`,
//!   3. einer Oberfläche in web/shared/games/<id>.js (+ optional .css).
//! Raum, Lobby, Einladungslinks und WebSocket kommen automatisch dazu.

use serde::Serialize;
use serde_json::Value;

pub mod chess;
pub mod skyjo;
pub mod uno;

pub type PlayerId = u32;

#[derive(Clone, Serialize)]
pub struct GameInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub min_players: usize,
    pub max_players: usize,
    /// Pfad zum Frontend-Modul (auf beiden Seiten unter /shared erreichbar)
    pub js: &'static str,
    pub css: &'static str,
}

/// Beschreibt ein Spiel und startet neue Partien.
pub trait GameKind: Send + Sync {
    fn info(&self) -> GameInfo;
    fn start(&self, players: &[PlayerId], options: &Value) -> Result<Box<dyn Game>, String>;
}

/// Eine laufende Partie.
pub trait Game: Send {
    /// Aktion eines Spielers (Format bestimmt das Spiel).
    fn apply(&mut self, player: PlayerId, action: &Value) -> Result<(), String>;
    /// Sicht eines Spielers (versteckte Infos wie fremde Handkarten weglassen!).
    fn view(&self, player: PlayerId) -> Value;
    fn finished(&self) -> bool;
}

/// Alle verfügbaren Spiele.
pub fn registry() -> Vec<Box<dyn GameKind>> {
    vec![Box::new(uno::UnoKind), Box::new(skyjo::SkyjoKind), Box::new(chess::ChessKind)]
}
