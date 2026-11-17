use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::games::{self, Event, Game, GameInfo, GameKind, PlayerId};

pub type Tx = UnboundedSender<String>;

const MAX_ROOMS: usize = 300;
const MAX_ROOMS_PER_OWNER: usize = 10;
const ROOM_IDLE_SECS: u64 = 2 * 60 * 60;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    Join {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        key: Option<String>,
    },
    Start,
    Action {
        action: Value,
    },
    /// Host: Partie beenden, zurück in die Lobby
    Lobby,
    Leave,
    Kick {
        player: PlayerId,
    },
    Ping,
}

struct Player {
    id: PlayerId,
    /// Geheimer Schlüssel zum Wiederverbinden (nie an andere Spieler senden)
    key: String,
    name: String,
    account: bool,
    conn: u64,
    tx: Option<Tx>,
}

struct Room {
    token: String,
    info: GameInfo,
    owner: String,
    owner_key: String,
    options: Value,
    players: Vec<Player>,
    host: Option<PlayerId>,
    game: Option<Box<dyn Game>>,
    next_id: PlayerId,
    touched: u64,
}

pub struct Hub {
    rooms: Mutex<HashMap<String, Room>>,
    kinds: Vec<Box<dyn GameKind>>,
    conns: AtomicU64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_token() -> String {
    // ohne verwechselbare Zeichen (0/o, 1/l/i)
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..10)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect()
}

fn unique_name(players: &[Player], name: String) -> String {
    let taken = |n: &str| players.iter().any(|p| p.name.eq_ignore_ascii_case(n));
    if !taken(&name) {
        return name;
    }
    let base: String = name.chars().take(17).collect();
    for i in 2..100 {
        let candidate = format!("{base} {i}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    name
}

fn state_for(room: &Room, pid: PlayerId) -> Value {
    let players: Vec<Value> = room
        .players
        .iter()
        .map(|p| json!({"id": p.id, "name": p.name, "connected": p.tx.is_some(), "account": p.account}))
        .collect();
    let game = room.game.as_ref().map(|g| g.view(pid));
    json!({
        "type": "state",
        "you": pid,
        "room": {
            "token": room.token,
            "game": room.info,
            "state": if room.game.is_some() { "playing" } else { "lobby" },
            "over": room.game.as_ref().map_or(false, |g| g.finished()),
            "host": room.host,
            "players": players,
        },
        "game": game,
    })
}

fn broadcast(room: &Room) {
    for p in &room.players {
        if let Some(tx) = &p.tx {
            let _ = tx.send(state_for(room, p.id).to_string());
        }
    }
}

fn send_events(room: &Room, events: Vec<Event>) {
    for e in events {
        let text = json!({"type": "event", "data": e.data}).to_string();
        for p in &room.players {
            if let Some(tx) = &p.tx {
                if e.to.is_empty() || e.to.contains(&p.id) {
                    let _ = tx.send(text.clone());
                }
            }
        }
    }
}

fn send_to(room: &Room, pid: PlayerId, msg: Value) {
    if let Some(tx) = room.players.iter().find(|p| p.id == pid).and_then(|p| p.tx.as_ref()) {
        let _ = tx.send(msg.to_string());
    }
}

fn pick_host(room: &mut Room) {
    room.host = room
        .players
        .iter()
        .find(|p| p.tx.is_some())
        .or_else(|| room.players.first())
        .map(|p| p.id);
}

fn public_info(room: &Room) -> Value {
    json!({
        "token": room.token,
        "game": room.info,
        "state": if room.game.is_some() { "playing" } else { "lobby" },
        "players": room.players.len(),
        "online": room.players.iter().filter(|p| p.tx.is_some()).count(),
        "max": room.info.max_players,
    })
}

impl Hub {
    pub fn new() -> Self {
        Hub {
            rooms: Mutex::new(HashMap::new()),
            kinds: games::registry(),
            conns: AtomicU64::new(1),
        }
    }

    pub fn games(&self) -> Vec<GameInfo> {
        self.kinds.iter().map(|k| k.info()).collect()
    }

    pub fn next_conn(&self) -> u64 {
        self.conns.fetch_add(1, Ordering::Relaxed)
    }

    pub fn create_room(&self, game_id: &str, owner_email: &str, options: Value) -> Result<String, String> {
        let kind = self
            .kinds
            .iter()
            .find(|k| k.info().id == game_id)
            .ok_or("Unbekanntes Spiel")?;
        let mut rooms = self.rooms.lock().unwrap();
        if rooms.len() >= MAX_ROOMS {
            return Err("Gerade sind zu viele Räume offen".into());
        }
        if rooms.values().filter(|r| r.owner == owner_email).count() >= MAX_ROOMS_PER_OWNER {
            return Err("Du hast schon zu viele offene Räume – lösche erst einen".into());
        }
        let mut token = new_token();
        while rooms.contains_key(&token) {
            token = new_token();
        }
        rooms.insert(
            token.clone(),
            Room {
                token: token.clone(),
                info: kind.info(),
                owner: owner_email.to_string(),
                owner_key: format!("acct:{owner_email}"),
                options,
                players: Vec::new(),
                host: None,
                game: None,
                next_id: 1,
                touched: now(),
            },
        );
        Ok(token)
    }

    pub fn info(&self, token: &str) -> Option<Value> {
        self.rooms.lock().unwrap().get(token).map(public_info)
    }

    pub fn rooms_of(&self, email: &str) -> Vec<Value> {
        let rooms = self.rooms.lock().unwrap();
        let mut list: Vec<&Room> = rooms.values().filter(|r| r.owner == email).collect();
        list.sort_by_key(|r| std::cmp::Reverse(r.touched));
        list.into_iter().map(public_info).collect()
    }

    pub fn delete_room(&self, token: &str, email: &str) -> bool {
        let mut rooms = self.rooms.lock().unwrap();
        match rooms.get(token) {
            Some(r) if r.owner == email => {}
            _ => return false,
        }
        if let Some(room) = rooms.remove(token) {
            for p in &room.players {
                if let Some(tx) = &p.tx {
                    let _ = tx.send(json!({"type": "closed"}).to_string());
                }
            }
        }
        true
    }

    /// Räume ohne Verbindung, die länger ruhen, aufräumen.
    pub fn sweep(&self) {
        let t = now();
        let mut rooms = self.rooms.lock().unwrap();
        rooms.retain(|_, r| {
            let online = r.players.iter().any(|p| p.tx.is_some());
            online || t.saturating_sub(r.touched) < ROOM_IDLE_SECS
        });
    }

    pub fn join(
        &self,
        token: &str,
        key: String,
        name: String,
        account: bool,
        conn: u64,
        tx: Tx,
    ) -> Result<PlayerId, String> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(token).ok_or("Dieser Raum existiert nicht (mehr)")?;
        room.touched = now();

        let existing = room.players.iter().position(|p| p.key == key);
        let pid = match existing {
            Some(i) => {
                let p = &mut room.players[i];
                if let Some(old) = p.tx.take() {
                    let _ = old.send(json!({"type": "replaced"}).to_string());
                }
                p.conn = conn;
                p.tx = Some(tx);
                p.account = account;
                p.id
            }
            None => {
                if room.game.is_some() {
                    return Err("Das Spiel läuft schon – warte auf die nächste Runde".into());
                }
                if room.players.len() >= room.info.max_players {
                    return Err("Der Raum ist voll".into());
                }
                let name = unique_name(&room.players, name);
                let id = room.next_id;
                room.next_id += 1;
                room.players.push(Player {
                    id,
                    key: key.clone(),
                    name,
                    account,
                    conn,
                    tx: Some(tx),
                });
                id
            }
        };
        if room.host.is_none() || key == room.owner_key {
            room.host = Some(pid);
        }
        broadcast(room);
        Ok(pid)
    }

    pub fn disconnect(&self, token: &str, pid: PlayerId, conn: u64) {
        let mut rooms = self.rooms.lock().unwrap();
        if let Some(room) = rooms.get_mut(token) {
            if let Some(p) = room.players.iter_mut().find(|p| p.id == pid && p.conn == conn) {
                p.tx = None;
                room.touched = now();
                if room.host == Some(pid) && room.players.iter().any(|p| p.tx.is_some()) {
                    // Host weg: ein anderer Online-Spieler übernimmt
                    if let Some(next) = room.players.iter().find(|p| p.tx.is_some()).map(|p| p.id) {
                        room.host = Some(next);
                    }
                }
                broadcast(room);
            }
        }
    }

    pub fn handle(&self, token: &str, pid: PlayerId, msg: ClientMsg) {
        let mut rooms = self.rooms.lock().unwrap();
        let Some(room) = rooms.get_mut(token) else { return };
        room.touched = now();
        let is_host = room.host == Some(pid);
        let mut quiet = false;
        let mut events: Vec<Event> = Vec::new();

        let result: Result<(), String> = match msg {
            ClientMsg::Start => {
                if !is_host {
                    Err("Nur der Host kann starten".into())
                } else if room.game.is_some() {
                    Err("Das Spiel läuft schon".into())
                } else {
                    // Nur wer online ist, spielt mit
                    room.players.retain(|p| p.tx.is_some());
                    let ids: Vec<PlayerId> = room.players.iter().map(|p| p.id).collect();
                    let kind = self.kinds.iter().find(|k| k.info().id == room.info.id);
                    match kind {
                        Some(k) => k.start(&ids, &room.options).map(|g| {
                            room.game = Some(g);
                        }),
                        None => Err("Spiel nicht gefunden".into()),
                    }
                }
            }
            ClientMsg::Action { action } => match room.game.as_mut() {
                Some(g) => g.apply(pid, &action).map(|_| {
                    quiet = !g.needs_state();
                    events = g.drain_events();
                }),
                None => Err("Es läuft kein Spiel".into()),
            },
            ClientMsg::Lobby => {
                if !is_host {
                    Err("Nur der Host kann die Runde beenden".into())
                } else {
                    room.game = None;
                    Ok(())
                }
            }
            ClientMsg::Kick { player } => {
                if !is_host {
                    Err("Nur der Host kann Spieler entfernen".into())
                } else if room.game.is_some() {
                    Err("Während des Spiels geht das nicht".into())
                } else if player == pid {
                    Err("Du kannst dich nicht selbst entfernen".into())
                } else {
                    send_to(room, player, json!({"type": "kicked"}));
                    room.players.retain(|p| p.id != player);
                    Ok(())
                }
            }
            ClientMsg::Leave => {
                if room.game.is_none() {
                    send_to(room, pid, json!({"type": "left"}));
                    room.players.retain(|p| p.id != pid);
                    if room.host == Some(pid) {
                        pick_host(room);
                    }
                }
                Ok(())
            }
            ClientMsg::Join { .. } | ClientMsg::Ping => return,
        };

        match result {
            Ok(()) => {
                if !quiet {
                    broadcast(room);
                }
                send_events(room, events);
            }
            Err(message) => send_to(room, pid, json!({"type": "error", "message": message})),
        }
    }

    /// Zeitsteuerung laufender Spiele (z. B. Zeichnen & Raten).
    pub fn tick(&self) {
        let mut rooms = self.rooms.lock().unwrap();
        for room in rooms.values_mut() {
            if room.players.iter().all(|p| p.tx.is_none()) {
                continue;
            }
            let Some(g) = room.game.as_mut() else { continue };
            let changed = g.tick();
            let events = g.drain_events();
            if changed {
                broadcast(room);
            }
            send_events(room, events);
        }
    }
}
