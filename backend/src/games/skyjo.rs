use rand::seq::SliceRandom;
use serde_json::{json, Value};

use super::{Game, GameInfo, GameKind, PlayerId};

const COLS: usize = 4;
const SLOTS: usize = 12;
const END_SCORE: i32 = 100;

#[derive(Clone, Copy)]
struct Slot {
    v: i8,
    up: bool,
    gone: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// alle decken zwei Karten auf
    Setup,
    Turn,
    RoundOver,
    Over,
}

pub struct SkyjoKind;

impl GameKind for SkyjoKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "skyjo",
            name: "Skyjo",
            description: "Sammle möglichst wenige Punkte: Karten tauschen, aufdecken, Dreierreihen wegwerfen.",
            min_players: 2,
            max_players: 8,
            js: "/shared/games/skyjo.js",
            css: "/shared/games/skyjo.css",
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
        let mut g = Skyjo {
            order: players.to_vec(),
            grids: Vec::new(),
            draw: Vec::new(),
            discard: Vec::new(),
            phase: Phase::Setup,
            cur: 0,
            held: None,
            from_discard: false,
            ender: None,
            totals: vec![0; players.len()],
            last: Vec::new(),
            round: 1,
            log: Vec::new(),
        };
        g.deal();
        Ok(Box::new(g))
    }
}

fn deck() -> Vec<i8> {
    let mut d = Vec::with_capacity(150);
    for (v, n) in [(-2i8, 5), (-1, 10), (0, 15)] {
        d.extend(std::iter::repeat(v).take(n));
    }
    for v in 1..=12i8 {
        d.extend(std::iter::repeat(v).take(10));
    }
    d
}

pub struct Skyjo {
    order: Vec<PlayerId>,
    grids: Vec<Vec<Slot>>,
    draw: Vec<i8>,
    discard: Vec<i8>,
    phase: Phase,
    cur: usize,
    /// Karte in der Hand des aktuellen Spielers (gezogen oder von der Ablage)
    held: Option<i8>,
    from_discard: bool,
    ender: Option<usize>,
    totals: Vec<i32>,
    last: Vec<Value>,
    round: u32,
    log: Vec<Value>,
}

impl Skyjo {
    fn deal(&mut self) {
        let mut d = deck();
        d.shuffle(&mut rand::thread_rng());
        self.grids = self
            .order
            .iter()
            .map(|_| {
                (0..SLOTS)
                    .map(|_| Slot { v: d.pop().unwrap_or(0), up: false, gone: false })
                    .collect()
            })
            .collect();
        self.discard = vec![d.pop().unwrap_or(0)];
        self.draw = d;
        self.phase = Phase::Setup;
        self.held = None;
        self.from_discard = false;
        self.ender = None;
        self.cur = 0;
        self.log.clear();
    }

    fn log(&mut self, e: Value) {
        self.log.push(e);
        if self.log.len() > 6 {
            self.log.remove(0);
        }
    }

    fn up_sum(&self, i: usize) -> i32 {
        self.grids[i].iter().filter(|s| s.up && !s.gone).map(|s| s.v as i32).sum()
    }

    fn full_sum(&self, i: usize) -> i32 {
        self.grids[i].iter().filter(|s| !s.gone).map(|s| s.v as i32).sum()
    }

    fn refill(&mut self) {
        if self.draw.is_empty() && self.discard.len() > 1 {
            let top = self.discard.pop().unwrap();
            let mut rest = std::mem::take(&mut self.discard);
            rest.shuffle(&mut rand::thread_rng());
            self.draw = rest;
            self.discard.push(top);
        }
    }

    /// Dreierreihen entfernen, Rundenende prüfen, nächster Spieler.
    fn after_turn(&mut self, idx: usize) {
        for c in 0..COLS {
            let cols = [c, c + COLS, c + 2 * COLS];
            let g = &self.grids[idx];
            let same = cols.iter().all(|&i| g[i].up && !g[i].gone) && g[cols[0]].v == g[cols[1]].v && g[cols[1]].v == g[cols[2]].v;
            if same {
                let v = g[cols[0]].v;
                for &i in &cols {
                    self.grids[idx][i].gone = true;
                    self.discard.push(v);
                }
                let p = self.order[idx];
                self.log(json!({"p": p, "a": "column", "v": v}));
            }
        }
        if self.ender.is_none() && self.grids[idx].iter().all(|s| s.up || s.gone) {
            self.ender = Some(idx);
            let p = self.order[idx];
            self.log(json!({"p": p, "a": "ender"}));
        }
        let next = (self.cur + 1) % self.order.len();
        if self.ender == Some(next) {
            self.end_round();
        } else {
            self.cur = next;
        }
    }

    fn end_round(&mut self) {
        for g in self.grids.iter_mut() {
            for s in g.iter_mut() {
                s.up = true;
            }
        }
        let scores: Vec<i32> = (0..self.order.len()).map(|i| self.full_sum(i)).collect();
        let mut doubled_for = None;
        let mut results = scores.clone();
        if let Some(e) = self.ender {
            let others_min = scores.iter().enumerate().filter(|(i, _)| *i != e).map(|(_, s)| *s).min();
            if let Some(m) = others_min {
                if scores[e] > 0 && scores[e] >= m {
                    results[e] = scores[e] * 2;
                    doubled_for = Some(e);
                }
            }
        }
        self.last = (0..self.order.len())
            .map(|i| {
                self.totals[i] += results[i];
                json!({"id": self.order[i], "round": results[i], "doubled": doubled_for == Some(i)})
            })
            .collect();
        self.phase = if self.totals.iter().any(|t| *t >= END_SCORE) { Phase::Over } else { Phase::RoundOver };
    }

    fn slot_arg(action: &Value) -> Result<usize, String> {
        let s = action.get("slot").and_then(Value::as_u64).ok_or("Karte fehlt")? as usize;
        if s < SLOTS { Ok(s) } else { Err("Ungültige Karte".into()) }
    }
}

impl Game for Skyjo {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        let idx = self.order.iter().position(|x| *x == p).ok_or("Du bist nicht in diesem Spiel")?;
        let kind = action.get("type").and_then(Value::as_str).unwrap_or("");

        match self.phase {
            Phase::Over => Err("Das Spiel ist vorbei".into()),
            Phase::RoundOver => {
                if kind == "next" {
                    self.round += 1;
                    self.deal();
                    Ok(())
                } else {
                    Err("Die Runde ist vorbei".into())
                }
            }
            Phase::Setup => {
                if kind != "flip" {
                    return Err("Decke zuerst zwei Karten auf".into());
                }
                let s = Self::slot_arg(action)?;
                let g = &mut self.grids[idx];
                if g[s].up {
                    return Err("Die Karte ist schon aufgedeckt".into());
                }
                if g.iter().filter(|x| x.up).count() >= 2 {
                    return Err("Du hast schon zwei Karten aufgedeckt".into());
                }
                g[s].up = true;
                if self.grids.iter().all(|g| g.iter().filter(|x| x.up).count() >= 2) {
                    // Wer die höchste Summe aufgedeckt hat, beginnt
                    let mut best = 0;
                    for i in 1..self.order.len() {
                        if self.up_sum(i) > self.up_sum(best) {
                            best = i;
                        }
                    }
                    self.cur = best;
                    self.phase = Phase::Turn;
                }
                Ok(())
            }
            Phase::Turn => {
                if idx != self.cur {
                    return Err("Du bist nicht dran".into());
                }
                match kind {
                    "draw" => {
                        if self.held.is_some() {
                            return Err("Du hältst schon eine Karte".into());
                        }
                        self.refill();
                        let c = self.draw.pop().ok_or("Der Stapel ist leer")?;
                        self.held = Some(c);
                        self.from_discard = false;
                        Ok(())
                    }
                    "take_discard" => {
                        if self.held.is_some() {
                            return Err("Du hältst schon eine Karte".into());
                        }
                        let c = self.discard.pop().ok_or("Die Ablage ist leer")?;
                        self.held = Some(c);
                        self.from_discard = true;
                        Ok(())
                    }
                    "place" => {
                        let held = self.held.ok_or("Zieh erst eine Karte")?;
                        let s = Self::slot_arg(action)?;
                        if self.grids[idx][s].gone {
                            return Err("Die Karte ist schon weg".into());
                        }
                        let old = self.grids[idx][s].v;
                        self.grids[idx][s].v = held;
                        self.grids[idx][s].up = true;
                        self.discard.push(old);
                        self.held = None;
                        self.log(json!({"p": p, "a": "place", "v": held, "old": old}));
                        self.after_turn(idx);
                        Ok(())
                    }
                    "discard_flip" => {
                        let held = self.held.ok_or("Zieh erst eine Karte")?;
                        if self.from_discard {
                            return Err("Eine Karte von der Ablage musst du behalten".into());
                        }
                        let s = Self::slot_arg(action)?;
                        if self.grids[idx][s].up || self.grids[idx][s].gone {
                            return Err("Wähle eine verdeckte Karte".into());
                        }
                        self.grids[idx][s].up = true;
                        let flipped = self.grids[idx][s].v;
                        self.discard.push(held);
                        self.held = None;
                        self.log(json!({"p": p, "a": "dflip", "v": held, "flip": flipped}));
                        self.after_turn(idx);
                        Ok(())
                    }
                    _ => Err("Unbekannte Aktion".into()),
                }
            }
        }
    }

    fn view(&self, viewer: PlayerId) -> Value {
        let revealed = matches!(self.phase, Phase::RoundOver | Phase::Over);
        let players: Vec<Value> = self
            .order
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let slots: Vec<Value> = self.grids[i]
                    .iter()
                    .map(|s| {
                        if s.gone {
                            json!({"gone": true})
                        } else if s.up || revealed {
                            json!({"up": true, "v": s.v})
                        } else {
                            json!({"up": false})
                        }
                    })
                    .collect();
                json!({
                    "id": id,
                    "slots": slots,
                    "shown": self.up_sum(i),
                    "total": self.totals[i],
                    "flipped": self.grids[i].iter().filter(|s| s.up).count(),
                })
            })
            .collect();
        let cur_id = self.order[self.cur];
        let held = match self.held {
            Some(v) if viewer == cur_id || self.from_discard => Some(v),
            _ => None,
        };
        let winner = if self.phase == Phase::Over {
            let mut best = 0;
            for i in 1..self.order.len() {
                if self.totals[i] < self.totals[best] {
                    best = i;
                }
            }
            Some(self.order[best])
        } else {
            None
        };
        json!({
            "phase": match self.phase { Phase::Setup => "setup", Phase::Turn => "turn", Phase::RoundOver => "roundover", Phase::Over => "over" },
            "round": self.round,
            "current": cur_id,
            "players": players,
            "draw": self.draw.len(),
            "discard": self.discard.last(),
            "held": held,
            "holding": self.held.is_some(),
            "from_discard": self.from_discard,
            "ender": self.ender.map(|i| self.order[i]),
            "last": self.last,
            "winner": winner,
            "end_score": END_SCORE,
            "log": self.log,
        })
    }

    fn finished(&self) -> bool {
        self.phase == Phase::Over
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    /// Spielt mit zufälligen, aber legalen Zügen und prüft, dass Runden sauber weiterlaufen.
    #[test]
    fn rounds_progress_and_game_ends() {
        let mut rng = rand::thread_rng();
        let mut saw_next_round = false;
        for _ in 0..30 {
            let mut g = SkyjoKind.start(&[1, 2, 3], &Value::Null).unwrap();
            for _ in 0..4000 {
                let v = g.view(1);
                match v["phase"].as_str().unwrap() {
                    "over" => break,
                    "roundover" => {
                        g.apply(1, &json!({"type": "next"})).unwrap();
                        assert_eq!(g.view(1)["phase"], "setup");
                        saw_next_round = true;
                    }
                    "setup" => {
                        for p in [1u32, 2, 3] {
                            let mine = g.view(p)["players"].as_array().unwrap().iter().find(|x| x["id"] == p).unwrap().clone();
                            let mut need = 2 - mine["flipped"].as_u64().unwrap() as i32;
                            for i in 0..12 {
                                if need > 0 && mine["slots"][i]["up"] == false {
                                    g.apply(p, &json!({"type": "flip", "slot": i})).unwrap();
                                    need -= 1;
                                }
                            }
                        }
                    }
                    _ => {
                        let cur = v["current"].as_u64().unwrap() as u32;
                        let cv = g.view(cur);
                        let me = cv["players"].as_array().unwrap().iter().find(|x| x["id"] == cur).unwrap().clone();
                        let live: Vec<usize> = (0..12).filter(|i| me["slots"][*i]["gone"] != true).collect();
                        let down: Vec<usize> = live.iter().copied().filter(|i| me["slots"][*i]["up"] == false).collect();
                        if cv["holding"] == false {
                            g.apply(cur, &json!({"type": "draw"})).unwrap();
                        } else if cv["from_discard"] == false && !down.is_empty() && rng.gen_bool(0.4) {
                            g.apply(cur, &json!({"type": "discard_flip", "slot": down[rng.gen_range(0..down.len())]})).unwrap();
                        } else {
                            g.apply(cur, &json!({"type": "place", "slot": live[rng.gen_range(0..live.len())]})).unwrap();
                        }
                    }
                }
            }
        }
        assert!(saw_next_round, "es gab nie eine zweite Runde");
    }
}
