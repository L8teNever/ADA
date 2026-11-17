//! Skribbl: Einer zeichnet ein Wort, alle anderen raten im Chat. Timer, Buchstaben-Hinweise,
//! Punkte nach Geschwindigkeit, mehrere Runden. Zeichenstriche laufen als leichte Events.

use std::time::{Duration, Instant};

use rand::{seq::SliceRandom, Rng};
use serde_json::{json, Value};

use super::{Event, Game, GameInfo, GameKind, PlayerId};

const WORDS: &[&str] = &[
    "Hund", "Katze", "Elefant", "Giraffe", "Pinguin", "Krokodil", "Schmetterling", "Spinne", "Hase", "Igel", "Eichhörnchen", "Delfin",
    "Hai", "Krake", "Schildkröte", "Frosch", "Biene", "Marienkäfer", "Papagei", "Eule", "Adler", "Fuchs", "Wolf", "Bär", "Löwe",
    "Känguru", "Nashorn", "Zebra", "Kamel", "Schaf", "Kuh", "Pferd", "Schwein", "Huhn", "Ente", "Schwan", "Fledermaus", "Gorilla",
    "Apfel", "Banane", "Erdbeere", "Ananas", "Wassermelone", "Traube", "Zitrone", "Karotte", "Tomate", "Pilz", "Brezel", "Pizza",
    "Hamburger", "Eis", "Kuchen", "Schokolade", "Spaghetti", "Pommes", "Popcorn", "Kaffee", "Käse", "Brot", "Ei", "Honig", "Lutscher",
    "Haus", "Schloss", "Brücke", "Leuchtturm", "Windmühle", "Zelt", "Iglu", "Kirche", "Turm", "Tunnel", "Tür", "Fenster", "Treppe",
    "Auto", "Fahrrad", "Flugzeug", "Hubschrauber", "Schiff", "Zug", "Rakete", "Traktor", "Motorrad", "Bus", "U-Boot", "Roller",
    "Sonne", "Mond", "Stern", "Wolke", "Regenbogen", "Blitz", "Schneemann", "Vulkan", "Berg", "Insel", "Wasserfall", "Palme", "Kaktus",
    "Blume", "Baum", "Tulpe", "Sonnenblume", "Laub", "Gras", "Wald", "Strand", "Wüste", "Höhle", "Feuer", "Lagerfeuer",
    "Stuhl", "Tisch", "Bett", "Sofa", "Lampe", "Spiegel", "Uhr", "Schlüssel", "Schere", "Hammer", "Säge", "Leiter", "Schaukel",
    "Regenschirm", "Brille", "Hut", "Krone", "Schuh", "Socke", "Handschuh", "Rucksack", "Koffer", "Ring", "Halskette", "Krawatte",
    "Gitarre", "Klavier", "Geige", "Trommel", "Trompete", "Mikrofon", "Kopfhörer", "Fernseher", "Handy", "Computer", "Maus", "Tastatur",
    "Kamera", "Fernglas", "Taschenlampe", "Batterie", "Steckdose", "Glühbirne", "Roboter", "Satellit", "Magnet", "Kompass",
    "Ball", "Fußball", "Tennisschläger", "Skateboard", "Schlitten", "Schachbrett", "Würfel", "Kartenspiel", "Puzzle", "Drachen",
    "Luftballon", "Geschenk", "Kerze", "Piñata", "Feuerwerk", "Weihnachtsbaum", "Kürbis", "Osterei", "Zauberer", "Drache",
    "Pirat", "Ritter", "Prinzessin", "Gespenst", "Vampir", "Einhorn", "Meerjungfrau", "Außerirdischer", "Superheld", "Clown",
    "Arzt", "Feuerwehrmann", "Polizist", "Koch", "Bauer", "Astronaut", "Taucher", "Maler", "Zahnarzt", "Pilot",
    "Herz", "Auge", "Nase", "Ohr", "Mund", "Zahn", "Hand", "Fuß", "Daumen", "Skelett", "Gehirn", "Bart",
    "Buch", "Stift", "Papier", "Briefkasten", "Postkarte", "Landkarte", "Globus", "Flagge", "Schatztruhe", "Anker", "Fahne",
    "Eimer", "Gießkanne", "Besen", "Staubsauger", "Waschmaschine", "Kühlschrank", "Toaster", "Topf", "Pfanne", "Löffel", "Gabel", "Tasse",
    "Telefon", "Radio", "Wecker", "Kalender", "Waage", "Thermometer", "Spritze", "Pflaster", "Rollstuhl", "Krücke",
    "Brunnen", "Zaun", "Laterne", "Ampel", "Bank", "Briefträger", "Zirkus", "Karussell", "Riesenrad", "Achterbahn",
];

const CHOOSE_SECS: u64 = 15;
const TURN_END_SECS: u64 = 7;

pub struct SkribblKind;

impl GameKind for SkribblKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "skribbl",
            name: "Skribbl",
            description: "Zeichnen und raten: Einer malt, alle anderen tippen das Wort – wer schneller ist, bekommt mehr Punkte.",
            min_players: 2,
            max_players: 12,
            js: "/shared/games/skribbl.js",
            css: "/shared/games/skribbl.css",
        }
    }

    fn start(&self, players: &[PlayerId], options: &Value) -> Result<Box<dyn Game>, String> {
        let info = self.info();
        if players.len() < info.min_players || players.len() > info.max_players {
            return Err(format!("Skribbl braucht {} bis {} Spieler", info.min_players, info.max_players));
        }
        let rounds = options.get("rounds").and_then(Value::as_u64).unwrap_or(3).clamp(1, 10) as u32;
        let draw_secs = options.get("time").and_then(Value::as_u64).unwrap_or(80).clamp(20, 240);
        let mut order = players.to_vec();
        order.shuffle(&mut rand::thread_rng());
        let n = order.len();
        let mut g = Skribbl {
            order,
            scores: vec![0; n],
            turn_pts: vec![0; n],
            guessed: vec![false; n],
            turn: 0,
            rounds,
            draw_secs,
            phase: Phase::Choosing,
            deadline: Instant::now(),
            phase_total: Duration::from_secs(CHOOSE_SECS),
            started: Instant::now(),
            choices: Vec::new(),
            word: "",
            shown: Vec::new(),
            hints: 0,
            ops: Vec::new(),
            stroke_pts: 0,
            chat: Vec::new(),
            events: Vec::new(),
            dirty: false,
            used: Vec::new(),
        };
        g.start_turn();
        Ok(Box::new(g))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Choosing,
    Drawing,
    TurnEnd,
    Over,
}

struct Skribbl {
    order: Vec<PlayerId>,
    scores: Vec<i32>,
    turn_pts: Vec<i32>,
    guessed: Vec<bool>,
    turn: u32,
    rounds: u32,
    draw_secs: u64,
    phase: Phase,
    deadline: Instant,
    phase_total: Duration,
    started: Instant,
    choices: Vec<&'static str>,
    word: &'static str,
    shown: Vec<bool>,
    hints: u8,
    ops: Vec<Value>,
    stroke_pts: usize,
    chat: Vec<Value>,
    events: Vec<Event>,
    dirty: bool,
    used: Vec<&'static str>,
}

/// Für den Vergleich: klein, Umlaute als ae/oe/ue, ß als ss, nur Buchstaben und Zahlen.
fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .flat_map(|c| match c {
            'ä' => vec!['a', 'e'],
            'ö' => vec!['o', 'e'],
            'ü' => vec!['u', 'e'],
            'ß' => vec!['s', 's'],
            c if c.is_alphanumeric() => vec![c],
            _ => vec![],
        })
        .collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

impl Skribbl {
    fn n(&self) -> usize {
        self.order.len()
    }

    fn drawer(&self) -> usize {
        (self.turn as usize) % self.n()
    }

    fn round(&self) -> u32 {
        self.turn / self.n() as u32 + 1
    }

    fn idx(&self, p: PlayerId) -> Option<usize> {
        self.order.iter().position(|x| *x == p)
    }

    fn all_but_drawer(&self) -> Vec<PlayerId> {
        let d = self.drawer();
        self.order.iter().enumerate().filter(|(i, _)| *i != d).map(|(_, p)| *p).collect()
    }

    fn knowers(&self) -> Vec<PlayerId> {
        let d = self.drawer();
        self.order.iter().enumerate().filter(|(i, _)| *i == d || self.guessed[*i]).map(|(_, p)| *p).collect()
    }

    fn add_chat(&mut self, p: Option<PlayerId>, text: String, kind: &str, vis: Vec<PlayerId>) {
        let line = json!({"p": p, "text": text, "kind": kind, "vis": vis});
        self.chat.push(line.clone());
        if self.chat.len() > 60 {
            self.chat.remove(0);
        }
        self.events.push(Event { to: vis, data: json!({"t": "chat", "line": line}) });
    }

    fn start_turn(&mut self) {
        let mut rng = rand::thread_rng();
        let mut pool: Vec<&'static str> = WORDS.iter().copied().filter(|w| !self.used.contains(w)).collect();
        if pool.len() < 3 {
            self.used.clear();
            pool = WORDS.to_vec();
        }
        pool.shuffle(&mut rng);
        self.choices = pool.into_iter().take(3).collect();
        self.word = "";
        self.shown.clear();
        self.hints = 0;
        self.ops.clear();
        self.stroke_pts = 0;
        self.guessed = vec![false; self.n()];
        self.turn_pts = vec![0; self.n()];
        self.phase = Phase::Choosing;
        self.phase_total = Duration::from_secs(CHOOSE_SECS);
        self.deadline = Instant::now() + self.phase_total;
        self.dirty = true;
    }

    fn pick(&mut self, w: &'static str) {
        self.word = w;
        self.used.push(w);
        self.shown = vec![false; w.chars().count()];
        self.hints = 0;
        self.phase = Phase::Drawing;
        self.started = Instant::now();
        self.phase_total = Duration::from_secs(self.draw_secs);
        self.deadline = self.started + self.phase_total;
        self.dirty = true;
    }

    fn end_turn(&mut self) {
        let d = self.drawer();
        let guessers: Vec<i32> = (0..self.n()).filter(|i| *i != d && self.guessed[*i]).map(|i| self.turn_pts[i]).collect();
        if !guessers.is_empty() {
            let avg = guessers.iter().sum::<i32>() / (self.n() as i32 - 1).max(1);
            self.turn_pts[d] = avg;
            self.scores[d] += avg;
        }
        self.phase = Phase::TurnEnd;
        self.phase_total = Duration::from_secs(TURN_END_SECS);
        self.deadline = Instant::now() + self.phase_total;
        let text = format!("Das Wort war „{}“", self.word);
        self.add_chat(None, text, "sys", Vec::new());
        self.dirty = true;
    }

    fn next_turn(&mut self) {
        self.turn += 1;
        if self.turn >= self.rounds * self.n() as u32 {
            self.phase = Phase::Over;
            self.dirty = true;
        } else {
            self.start_turn();
        }
    }

    fn reveal_hint(&mut self) {
        let chars = self.word.chars().count();
        let hidden: Vec<usize> = (0..chars).filter(|i| !self.shown[*i]).collect();
        // Höchstens die Hälfte der Buchstaben verraten
        if hidden.len() * 2 <= chars + 1 || hidden.is_empty() {
            return;
        }
        let i = hidden[rand::thread_rng().gen_range(0..hidden.len())];
        self.shown[i] = true;
    }

    fn hint(&self) -> Vec<String> {
        self.word.chars().enumerate().map(|(i, c)| if self.shown.get(i).copied().unwrap_or(false) { c.to_string() } else { "_".into() }).collect()
    }

    fn draw_event(&mut self, data: Value) {
        let to = self.all_but_drawer();
        self.events.push(Event { to, data });
    }
}

impl Game for Skribbl {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        let i = self.idx(p).ok_or("Du bist nicht in diesem Spiel")?;
        let kind = action.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "pick" => {
                if self.phase != Phase::Choosing || i != self.drawer() {
                    return Err("Du kannst gerade kein Wort wählen".into());
                }
                let k = action.get("index").and_then(Value::as_u64).ok_or("Wort fehlt")? as usize;
                let w = *self.choices.get(k).ok_or("Ungültige Wahl")?;
                self.pick(w);
                Ok(())
            }
            "guess" => {
                let text: String = action.get("text").and_then(Value::as_str).unwrap_or("").trim().chars().take(60).collect();
                if text.is_empty() {
                    return Ok(());
                }
                if self.phase != Phase::Drawing {
                    self.add_chat(Some(p), text, "chat", Vec::new());
                    return Ok(());
                }
                // Wer das Wort kennt, chattet nur mit anderen, die es kennen
                if i == self.drawer() || self.guessed[i] {
                    let vis = self.knowers();
                    self.add_chat(Some(p), text, "chat", vis);
                    return Ok(());
                }
                let (g, w) = (norm(&text), norm(self.word));
                if g == w {
                    let left = self.deadline.saturating_duration_since(Instant::now()).as_secs_f64();
                    let frac = (left / self.draw_secs as f64).clamp(0.0, 1.0);
                    let pts = (50.0 + 450.0 * frac).round() as i32 / 10 * 10;
                    self.guessed[i] = true;
                    self.turn_pts[i] = pts;
                    self.scores[i] += pts;
                    self.add_chat(Some(p), "hat das Wort erraten!".into(), "correct", Vec::new());
                    self.dirty = true;
                    let d = self.drawer();
                    if (0..self.n()).filter(|k| *k != d).all(|k| self.guessed[k]) {
                        self.end_turn();
                    }
                } else {
                    self.add_chat(Some(p), text, "guess", Vec::new());
                    if w.chars().count() >= 4 && edit_distance(&g, &w) <= 1 {
                        self.add_chat(None, "Fast richtig!".into(), "close", vec![p]);
                    }
                }
                Ok(())
            }
            "seg" | "fill" | "undo" | "clear" => {
                if self.phase != Phase::Drawing || i != self.drawer() {
                    return Err("Du darfst gerade nicht zeichnen".into());
                }
                let colour = action.get("c").and_then(Value::as_u64).unwrap_or(1).min(15);
                let width = action.get("w").and_then(Value::as_u64).unwrap_or(1).min(7);
                match kind {
                    "seg" => {
                        let start = action.get("start").and_then(Value::as_bool).unwrap_or(false);
                        let pts: Vec<i64> = action
                            .get("p")
                            .and_then(Value::as_array)
                            .map(|a| a.iter().filter_map(Value::as_i64).map(|v| v.clamp(0, 800)).collect())
                            .unwrap_or_default();
                        if pts.len() < 2 || pts.len() > 400 || pts.len() % 2 != 0 {
                            return Err("Ungültiger Strich".into());
                        }
                        if start {
                            if self.ops.len() >= 3000 {
                                return Err("Zu viele Striche".into());
                            }
                            self.stroke_pts = 0;
                            self.ops.push(json!({"k": "s", "c": colour, "w": width, "p": pts}));
                        } else {
                            let Some(last) = self.ops.last_mut() else { return Err("Kein offener Strich".into()) };
                            if last["k"] != "s" || self.stroke_pts > 20000 {
                                return Err("Kein offener Strich".into());
                            }
                            if let Some(arr) = last["p"].as_array_mut() {
                                arr.extend(pts.iter().map(|v| json!(v)));
                            }
                        }
                        self.stroke_pts += pts.len();
                        self.draw_event(json!({"t": "seg", "start": start, "c": colour, "w": width, "p": pts}));
                    }
                    "fill" => {
                        let x = action.get("x").and_then(Value::as_i64).unwrap_or(0).clamp(0, 799);
                        let y = action.get("y").and_then(Value::as_i64).unwrap_or(0).clamp(0, 599);
                        if self.ops.len() >= 3000 {
                            return Err("Zu viele Striche".into());
                        }
                        self.ops.push(json!({"k": "f", "c": colour, "x": x, "y": y}));
                        self.draw_event(json!({"t": "fill", "c": colour, "x": x, "y": y}));
                    }
                    "undo" => {
                        self.ops.pop();
                        self.draw_event(json!({"t": "undo"}));
                    }
                    _ => {
                        self.ops.clear();
                        self.draw_event(json!({"t": "clear"}));
                    }
                }
                Ok(())
            }
            _ => Err("Unbekannte Aktion".into()),
        }
    }

    fn view(&self, viewer: PlayerId) -> Value {
        let me = self.idx(viewer);
        let is_drawer = me == Some(self.drawer());
        let knows = is_drawer || me.map_or(false, |i| self.guessed[i]);
        let reveal = matches!(self.phase, Phase::TurnEnd | Phase::Over);
        let word = if (self.phase == Phase::Drawing && knows) || reveal { Some(self.word) } else { None };
        let scores: Vec<Value> = (0..self.n())
            .map(|i| json!({"id": self.order[i], "score": self.scores[i], "turn": self.turn_pts[i], "guessed": self.guessed[i]}))
            .collect();
        let chat: Vec<&Value> = self
            .chat
            .iter()
            .filter(|l| l["vis"].as_array().map_or(true, |v| v.is_empty() || v.iter().any(|x| x.as_u64() == Some(viewer as u64))))
            .collect();
        json!({
            "phase": match self.phase { Phase::Choosing => "choosing", Phase::Drawing => "drawing", Phase::TurnEnd => "turnend", Phase::Over => "over" },
            "round": self.round().min(self.rounds),
            "rounds": self.rounds,
            "drawer": self.order[self.drawer()],
            "remaining_ms": self.deadline.saturating_duration_since(Instant::now()).as_millis() as u64,
            "total_ms": self.phase_total.as_millis() as u64,
            "choices": if self.phase == Phase::Choosing && is_drawer { json!(self.choices) } else { Value::Null },
            "word": word,
            "hint": if self.phase == Phase::Drawing || self.phase == Phase::TurnEnd { json!(self.hint()) } else { Value::Null },
            "word_len": self.word.chars().count(),
            "scores": scores,
            "ops": self.ops,
            "chat": chat,
        })
    }

    fn finished(&self) -> bool {
        self.phase == Phase::Over
    }

    fn needs_state(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn tick(&mut self) -> bool {
        let now = Instant::now();
        match self.phase {
            Phase::Choosing => {
                if now >= self.deadline {
                    let w = self.choices[rand::thread_rng().gen_range(0..self.choices.len())];
                    self.pick(w);
                }
            }
            Phase::Drawing => {
                let elapsed = now.saturating_duration_since(self.started).as_secs_f64() / self.draw_secs as f64;
                let marks = [0.35, 0.6, 0.8];
                if (self.hints as usize) < marks.len() && elapsed >= marks[self.hints as usize] {
                    self.hints += 1;
                    if self.word.chars().count() >= 4 {
                        self.reveal_hint();
                    }
                    self.dirty = true;
                }
                if now >= self.deadline {
                    self.end_turn();
                }
            }
            Phase::TurnEnd => {
                if now >= self.deadline {
                    self.next_turn();
                }
            }
            Phase::Over => {}
        }
        std::mem::take(&mut self.dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_game_with_bots() {
        let mut g = SkribblKind.start(&[1, 2, 3], &json!({"rounds": 1, "time": 30})).unwrap();
        for _ in 0..3 {
            let v = g.view(1);
            let drawer = v["drawer"].as_u64().unwrap() as u32;
            assert_eq!(v["phase"], "choosing");
            let word = v_choice(&mut g, drawer);
            g.apply(drawer, &json!({"type": "seg", "start": true, "c": 3, "w": 2, "p": [10, 10, 20, 20]})).unwrap();
            g.apply(drawer, &json!({"type": "seg", "start": false, "c": 3, "w": 2, "p": [30, 30]})).unwrap();
            assert_eq!(g.view(drawer)["ops"][0]["p"].as_array().unwrap().len(), 6);
            assert!(g.drain_events().iter().all(|e| !e.to.contains(&drawer)));
            // Falsche Tipps, dann richtig
            let others: Vec<u32> = [1u32, 2, 3].iter().copied().filter(|p| *p != drawer).collect();
            g.apply(others[0], &json!({"type": "guess", "text": "xyz"})).unwrap();
            assert_eq!(g.view(others[0])["word"], Value::Null);
            g.apply(others[0], &json!({"type": "guess", "text": word.to_uppercase()})).unwrap();
            assert!(g.view(others[0])["word"].is_string());
            assert_eq!(g.view(others[1])["word"], Value::Null);
            g.apply(others[1], &json!({"type": "guess", "text": word})).unwrap();
            assert_eq!(g.view(1)["phase"], "turnend");
            g.apply(drawer, &json!({"type": "guess", "text": "egal"})).unwrap();
            // Turn-End überspringen
            if let Some(s) = (g.as_mut() as &mut dyn Game).tick().then_some(()) { let _ = s; }
            force_deadline(&mut g);
        }
        assert!(g.finished());
    }

    fn v_choice(g: &mut Box<dyn Game>, drawer: u32) -> String {
        let v = g.view(drawer);
        let w = v["choices"][0].as_str().unwrap().to_string();
        g.apply(drawer, &json!({"type": "pick", "index": 0})).unwrap();
        w
    }

    fn force_deadline(g: &mut Box<dyn Game>) {
        // Spielzeit der Auswertungsphase abkürzen: tick() wiederholt aufrufen, bis die Phase wechselt
        for _ in 0..40 {
            if g.view(1)["phase"] != "turnend" {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
            g.tick();
        }
    }

    #[test]
    fn normalizing() {
        assert_eq!(norm("Käse"), norm("kaese"));
        assert_eq!(norm("Fuß-ball"), "fussball");
        assert_eq!(edit_distance("katze", "katse"), 1);
    }
}
