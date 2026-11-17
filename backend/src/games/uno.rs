use rand::seq::SliceRandom;
use serde::Serialize;
use serde_json::{json, Value};

use super::{Game, GameInfo, GameKind, PlayerId};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
enum Color {
    Red,
    Yellow,
    Green,
    Blue,
    Wild,
}

impl Color {
    fn parse(s: &str) -> Option<Color> {
        match s {
            "red" => Some(Color::Red),
            "yellow" => Some(Color::Yellow),
            "green" => Some(Color::Green),
            "blue" => Some(Color::Blue),
            _ => None,
        }
    }
}

// kind: 0-9 = Zahl, danach Aktionskarten
const SKIP: u8 = 10;
const REVERSE: u8 = 11;
const DRAW2: u8 = 12;
const WILD: u8 = 13;
const WILD4: u8 = 14;

#[derive(Clone, Copy, Serialize, Debug)]
struct Card {
    id: u8,
    color: Color,
    kind: u8,
}

fn new_deck() -> Vec<Card> {
    let mut deck = Vec::with_capacity(108);
    let mut id = 0u8;
    for color in [Color::Red, Color::Yellow, Color::Green, Color::Blue] {
        for kind in 0..=DRAW2 {
            let copies = if kind == 0 { 1 } else { 2 };
            for _ in 0..copies {
                deck.push(Card { id, color, kind });
                id += 1;
            }
        }
    }
    for kind in [WILD, WILD4] {
        for _ in 0..4 {
            deck.push(Card { id, color: Color::Wild, kind });
            id += 1;
        }
    }
    deck
}

pub struct UnoKind;

impl GameKind for UnoKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "uno",
            name: "UNO",
            description: "Das Kartenspiel-Klassiker: Karten passend ablegen, als Erstes alle loswerden.",
            min_players: 2,
            max_players: 10,
            js: "/shared/games/uno.js",
            css: "/shared/games/uno.css",
        }
    }

    fn start(&self, players: &[PlayerId], _options: &Value) -> Result<Box<dyn Game>, String> {
        let info = self.info();
        if players.len() < info.min_players {
            return Err(format!("Mindestens {} Spieler nötig", info.min_players));
        }
        if players.len() > info.max_players {
            return Err(format!("Höchstens {} Spieler erlaubt", info.max_players));
        }
        let mut rng = rand::thread_rng();
        let mut draw = new_deck();
        draw.shuffle(&mut rng);

        let mut hands: Vec<Vec<Card>> = Vec::new();
        for _ in players {
            let split = draw.len() - 7;
            hands.push(draw.split_off(split));
        }

        // Startkarte: eine Zahlenkarte
        let mut top = draw.pop().ok_or("Stapel leer")?;
        let mut guard = 0;
        while top.kind > 9 && guard < 200 {
            draw.insert(0, top);
            top = draw.pop().ok_or("Stapel leer")?;
            guard += 1;
        }

        Ok(Box::new(Uno {
            order: players.to_vec(),
            hands,
            draw,
            color: top.color,
            discard: vec![top],
            cur: 0,
            dir: 1,
            drew: None,
            winner: None,
            log: Vec::new(),
        }))
    }
}

pub struct Uno {
    order: Vec<PlayerId>,
    hands: Vec<Vec<Card>>,
    draw: Vec<Card>,
    discard: Vec<Card>,
    cur: usize,
    dir: i32,
    color: Color,
    /// Karte, die der aktuelle Spieler gerade gezogen hat (darf sie noch spielen)
    drew: Option<u8>,
    winner: Option<PlayerId>,
    log: Vec<Value>,
}

impl Uno {
    fn idx_of(&self, p: PlayerId) -> Option<usize> {
        self.order.iter().position(|x| *x == p)
    }

    fn step(&self, from: usize, k: i32) -> usize {
        let n = self.order.len() as i32;
        (from as i32 + self.dir * k).rem_euclid(n) as usize
    }

    fn top(&self) -> Card {
        *self.discard.last().expect("Ablagestapel nie leer")
    }

    fn playable(&self, c: &Card) -> bool {
        c.color == Color::Wild || c.color == self.color || c.kind == self.top().kind
    }

    fn refill(&mut self) {
        if self.discard.len() > 1 {
            let top = self.discard.pop().unwrap();
            let mut rest = std::mem::take(&mut self.discard);
            rest.shuffle(&mut rand::thread_rng());
            self.draw = rest;
            self.discard.push(top);
        }
    }

    fn draw_cards(&mut self, idx: usize, n: usize) {
        for _ in 0..n {
            if self.draw.is_empty() {
                self.refill();
            }
            match self.draw.pop() {
                Some(c) => self.hands[idx].push(c),
                None => break,
            }
        }
    }

    fn log(&mut self, entry: Value) {
        self.log.push(entry);
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }
}

impl Game for Uno {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        if self.winner.is_some() {
            return Err("Die Runde ist vorbei".into());
        }
        let idx = self.idx_of(p).ok_or("Du bist nicht in diesem Spiel")?;
        if idx != self.cur {
            return Err("Du bist nicht dran".into());
        }

        match action.get("type").and_then(Value::as_str) {
            Some("play") => {
                let id = action
                    .get("card")
                    .and_then(Value::as_u64)
                    .ok_or("Karte fehlt")? as u8;
                if let Some(d) = self.drew {
                    if d != id {
                        return Err("Du darfst nur die gezogene Karte spielen".into());
                    }
                }
                let pos = self.hands[idx]
                    .iter()
                    .position(|c| c.id == id)
                    .ok_or("Die Karte hast du nicht")?;
                let card = self.hands[idx][pos];
                if !self.playable(&card) {
                    return Err("Die Karte passt nicht".into());
                }
                let chosen = if card.color == Color::Wild {
                    let c = action
                        .get("color")
                        .and_then(Value::as_str)
                        .and_then(Color::parse)
                        .ok_or("Bitte wähle eine Farbe")?;
                    Some(c)
                } else {
                    None
                };

                self.hands[idx].remove(pos);
                self.discard.push(card);
                self.color = chosen.unwrap_or(card.color);
                self.drew = None;
                self.log(json!({"p": p, "a": "play", "card": card, "color": self.color}));

                if self.hands[idx].is_empty() {
                    self.winner = Some(p);
                    return Ok(());
                }
                if self.hands[idx].len() == 1 {
                    self.log(json!({"p": p, "a": "uno"}));
                }

                let n = self.order.len();
                match card.kind {
                    SKIP => {
                        self.cur = self.step(self.cur, 2);
                    }
                    REVERSE => {
                        self.dir = -self.dir;
                        // zu zweit wirkt Reverse wie Aussetzen
                        self.cur = if n == 2 { self.step(self.cur, 2) } else { self.step(self.cur, 1) };
                    }
                    DRAW2 | WILD4 => {
                        let target = self.step(self.cur, 1);
                        let count = if card.kind == DRAW2 { 2 } else { 4 };
                        self.draw_cards(target, count);
                        let tp = self.order[target];
                        self.log(json!({"p": tp, "a": "forced", "n": count}));
                        self.cur = self.step(self.cur, 2);
                    }
                    _ => {
                        self.cur = self.step(self.cur, 1);
                    }
                }
                Ok(())
            }
            Some("draw") => {
                if self.drew.is_some() {
                    return Err("Du hast schon gezogen".into());
                }
                self.draw_cards(idx, 1);
                let drawn = self.hands[idx].last().copied();
                self.log(json!({"p": p, "a": "draw"}));
                match drawn {
                    Some(c) if self.playable(&c) => {
                        self.drew = Some(c.id);
                    }
                    _ => {
                        self.cur = self.step(self.cur, 1);
                    }
                }
                Ok(())
            }
            Some("pass") => {
                if self.drew.is_none() {
                    return Err("Du musst erst eine Karte ziehen".into());
                }
                self.drew = None;
                self.cur = self.step(self.cur, 1);
                Ok(())
            }
            _ => Err("Unbekannte Aktion".into()),
        }
    }

    fn view(&self, p: PlayerId) -> Value {
        let me = self.idx_of(p);
        let players: Vec<Value> = self
            .order
            .iter()
            .zip(self.hands.iter())
            .map(|(id, h)| json!({"id": id, "cards": h.len()}))
            .collect();
        let hand: Vec<Card> = me.map(|i| self.hands[i].clone()).unwrap_or_default();
        let my_turn = me == Some(self.cur) && self.winner.is_none();
        let playable: Vec<u8> = if my_turn {
            hand.iter()
                .filter(|c| self.drew.map_or(true, |d| d == c.id) && self.playable(c))
                .map(|c| c.id)
                .collect()
        } else {
            Vec::new()
        };
        let current = self.order[self.cur];
        let top = self.top();
        json!({
            "players": players,
            "hand": hand,
            "top": top,
            "color": self.color,
            "dir": self.dir,
            "current": current,
            "deck": self.draw.len(),
            "drew": self.drew,
            "winner": self.winner,
            "playable": playable,
            "log": self.log,
        })
    }

    fn finished(&self) -> bool {
        self.winner.is_some()
    }
}
