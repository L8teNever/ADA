//! Schach mit vollständigen Regeln: Rochade, en passant, Umwandlung, Schach/Matt/Patt,
//! 50-Züge-Regel, dreifache Stellungswiederholung, ungenügendes Material, Aufgabe, Remis-Angebot.

use rand::Rng;
use serde_json::{json, Value};

use super::{Game, GameInfo, GameKind, PlayerId};

const P: i8 = 1;
const N: i8 = 2;
const B: i8 = 3;
const R: i8 = 4;
const Q: i8 = 5;
const K: i8 = 6;

const KNIGHT: [(i32, i32); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const ROOK_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

pub struct ChessKind;

impl GameKind for ChessKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "chess",
            name: "Schach",
            description: "Der Klassiker für zwei: vollständige Regeln, Rochade, en passant, Matt und Remis.",
            min_players: 2,
            max_players: 2,
            js: "/shared/games/chess.js",
            css: "/shared/games/chess.css",
        }
    }

    fn start(&self, players: &[PlayerId], _options: &Value) -> Result<Box<dyn Game>, String> {
        if players.len() != 2 {
            return Err("Schach braucht genau 2 Spieler".into());
        }
        let (white, black) = if rand::thread_rng().gen_bool(0.5) {
            (players[0], players[1])
        } else {
            (players[1], players[0])
        };
        let pos = Pos::start();
        Ok(Box::new(Chess {
            white,
            black,
            keys: vec![pos.key()],
            pos,
            moves: Vec::new(),
            last: None,
            status: "playing",
            winner: None,
            draw_offer: None,
            by_white: Vec::new(),
            by_black: Vec::new(),
        }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Mv {
    from: usize,
    to: usize,
    promo: i8,
}

#[derive(Clone)]
struct Pos {
    b: [i8; 64],
    white: bool,
    /// Rochaderechte: Weiß kurz, Weiß lang, Schwarz kurz, Schwarz lang
    cas: [bool; 4],
    ep: Option<usize>,
    half: u32,
}

fn on(r: i32, f: i32) -> bool {
    (0..8).contains(&r) && (0..8).contains(&f)
}

impl Pos {
    fn start() -> Pos {
        let mut b = [0i8; 64];
        let back = [R, N, B, Q, K, B, N, R];
        for f in 0..8 {
            b[f] = back[f];
            b[8 + f] = P;
            b[48 + f] = -P;
            b[56 + f] = -back[f];
        }
        Pos { b, white: true, cas: [true; 4], ep: None, half: 0 }
    }

    fn key(&self) -> String {
        format!("{:?}|{}|{:?}|{:?}", self.b.as_slice(), self.white, self.cas, self.ep)
    }

    fn attacked(&self, sq: usize, by_white: bool) -> bool {
        let s: i8 = if by_white { 1 } else { -1 };
        let (r, f) = ((sq / 8) as i32, (sq % 8) as i32);
        let at = |rr: i32, ff: i32| -> i8 { if on(rr, ff) { self.b[(rr * 8 + ff) as usize] } else { 0 } };
        let pr = if by_white { r - 1 } else { r + 1 };
        for df in [-1, 1] {
            if at(pr, f + df) == s * P {
                return true;
            }
        }
        for (dr, df) in KNIGHT {
            if at(r + dr, f + df) == s * N {
                return true;
            }
        }
        for (dr, df) in KING {
            if at(r + dr, f + df) == s * K {
                return true;
            }
        }
        for (dirs, a, b2) in [(ROOK_DIRS, R, Q), (BISHOP_DIRS, B, Q)] {
            for (dr, df) in dirs {
                let (mut rr, mut ff) = (r + dr, f + df);
                while on(rr, ff) {
                    let p = self.b[(rr * 8 + ff) as usize];
                    if p != 0 {
                        if p == s * a || p == s * b2 {
                            return true;
                        }
                        break;
                    }
                    rr += dr;
                    ff += df;
                }
            }
        }
        false
    }

    fn king_sq(&self, white: bool) -> Option<usize> {
        let k = if white { K } else { -K };
        self.b.iter().position(|p| *p == k)
    }

    fn in_check(&self, white: bool) -> bool {
        match self.king_sq(white) {
            Some(k) => self.attacked(k, !white),
            None => false,
        }
    }

    fn pseudo(&self) -> Vec<Mv> {
        let mut out = Vec::with_capacity(48);
        let s: i8 = if self.white { 1 } else { -1 };
        for sq in 0..64usize {
            let p = self.b[sq];
            if p == 0 || p.signum() != s {
                continue;
            }
            let (r, f) = ((sq / 8) as i32, (sq % 8) as i32);
            match p.abs() {
                P => {
                    let dir = s as i32;
                    let start = if self.white { 1 } else { 6 };
                    let last = if self.white { 7 } else { 0 };
                    let push = |to: usize, out: &mut Vec<Mv>| {
                        if (to / 8) as i32 == last {
                            for pr in [Q, R, B, N] {
                                out.push(Mv { from: sq, to, promo: pr });
                            }
                        } else {
                            out.push(Mv { from: sq, to, promo: 0 });
                        }
                    };
                    let r1 = r + dir;
                    if on(r1, f) && self.b[(r1 * 8 + f) as usize] == 0 {
                        push((r1 * 8 + f) as usize, &mut out);
                        let r2 = r + 2 * dir;
                        if r == start && self.b[(r2 * 8 + f) as usize] == 0 {
                            out.push(Mv { from: sq, to: (r2 * 8 + f) as usize, promo: 0 });
                        }
                    }
                    for df in [-1, 1] {
                        let (rr, ff) = (r + dir, f + df);
                        if !on(rr, ff) {
                            continue;
                        }
                        let to = (rr * 8 + ff) as usize;
                        let t = self.b[to];
                        if (t != 0 && t.signum() == -s) || (t == 0 && self.ep == Some(to)) {
                            push(to, &mut out);
                        }
                    }
                }
                N | K => {
                    let offs: &[(i32, i32); 8] = if p.abs() == N { &KNIGHT } else { &KING };
                    for (dr, df) in offs {
                        let (rr, ff) = (r + dr, f + df);
                        if on(rr, ff) {
                            let to = (rr * 8 + ff) as usize;
                            if self.b[to].signum() != s {
                                out.push(Mv { from: sq, to, promo: 0 });
                            }
                        }
                    }
                    if p.abs() == K {
                        self.castles(sq, &mut out);
                    }
                }
                _ => {
                    let dirs: Vec<(i32, i32)> = match p.abs() {
                        B => BISHOP_DIRS.to_vec(),
                        R => ROOK_DIRS.to_vec(),
                        _ => ROOK_DIRS.iter().chain(BISHOP_DIRS.iter()).copied().collect(),
                    };
                    for (dr, df) in dirs {
                        let (mut rr, mut ff) = (r + dr, f + df);
                        while on(rr, ff) {
                            let to = (rr * 8 + ff) as usize;
                            let t = self.b[to];
                            if t == 0 {
                                out.push(Mv { from: sq, to, promo: 0 });
                            } else {
                                if t.signum() != s {
                                    out.push(Mv { from: sq, to, promo: 0 });
                                }
                                break;
                            }
                            rr += dr;
                            ff += df;
                        }
                    }
                }
            }
        }
        out
    }

    fn castles(&self, sq: usize, out: &mut Vec<Mv>) {
        let (base, rook, ks, qs) = if self.white { (0usize, R, self.cas[0], self.cas[1]) } else { (56usize, -R, self.cas[2], self.cas[3]) };
        if sq != base + 4 {
            return;
        }
        let enemy_white = !self.white;
        if self.attacked(sq, enemy_white) {
            return;
        }
        if ks
            && self.b[base + 7] == rook
            && self.b[base + 5] == 0
            && self.b[base + 6] == 0
            && !self.attacked(base + 5, enemy_white)
            && !self.attacked(base + 6, enemy_white)
        {
            out.push(Mv { from: sq, to: base + 6, promo: 0 });
        }
        if qs
            && self.b[base] == rook
            && self.b[base + 1] == 0
            && self.b[base + 2] == 0
            && self.b[base + 3] == 0
            && !self.attacked(base + 3, enemy_white)
            && !self.attacked(base + 2, enemy_white)
        {
            out.push(Mv { from: sq, to: base + 2, promo: 0 });
        }
    }

    fn make(&mut self, m: Mv) {
        let piece = self.b[m.from];
        let cap = self.b[m.to];
        let s: i8 = if self.white { 1 } else { -1 };
        let mut captured_any = cap != 0;
        if piece.abs() == P && cap == 0 && self.ep == Some(m.to) && m.from % 8 != m.to % 8 {
            self.b[(m.from / 8) * 8 + m.to % 8] = 0;
            captured_any = true;
        }
        if piece.abs() == K && (m.to as i32 - m.from as i32).abs() == 2 {
            let (rf, rt) = if m.to > m.from { (m.from + 3, m.from + 1) } else { (m.from - 4, m.from - 1) };
            self.b[rt] = self.b[rf];
            self.b[rf] = 0;
        }
        self.b[m.to] = if m.promo != 0 { s * m.promo } else { piece };
        self.b[m.from] = 0;
        self.ep = if piece.abs() == P && (m.to as i32 - m.from as i32).abs() == 16 { Some((m.from + m.to) / 2) } else { None };
        if piece.abs() == K {
            if self.white {
                self.cas[0] = false;
                self.cas[1] = false;
            } else {
                self.cas[2] = false;
                self.cas[3] = false;
            }
        }
        for sq in [m.from, m.to] {
            match sq {
                7 => self.cas[0] = false,
                0 => self.cas[1] = false,
                63 => self.cas[2] = false,
                56 => self.cas[3] = false,
                _ => {}
            }
        }
        self.half = if piece.abs() == P || captured_any { 0 } else { self.half + 1 };
        self.white = !self.white;
    }

    fn legal(&self) -> Vec<Mv> {
        self.pseudo()
            .into_iter()
            .filter(|m| {
                let mut p = self.clone();
                p.make(*m);
                !p.in_check(self.white)
            })
            .collect()
    }

    fn insufficient(&self) -> bool {
        let others: Vec<(usize, i8)> = self.b.iter().enumerate().filter(|(_, p)| **p != 0 && p.abs() != K).map(|(i, p)| (i, p.abs())).collect();
        match others.len() {
            0 => true,
            1 => others[0].1 == N || others[0].1 == B,
            _ => others.iter().all(|(i, p)| *p == B && ((i / 8) + (i % 8)) % 2 == ((others[0].0 / 8) + (others[0].0 % 8)) % 2),
        }
    }
}

fn sq_name(sq: usize) -> String {
    format!("{}{}", (b'a' + (sq % 8) as u8) as char, sq / 8 + 1)
}

fn parse_sq(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() != 2 || !(b'a'..=b'h').contains(&b[0]) || !(b'1'..=b'8').contains(&b[1]) {
        return None;
    }
    Some(((b[1] - b'1') as usize) * 8 + (b[0] - b'a') as usize)
}

fn letter(p: i8) -> char {
    let c = match p.abs() {
        P => 'p',
        N => 'n',
        B => 'b',
        R => 'r',
        Q => 'q',
        _ => 'k',
    };
    if p > 0 { c.to_ascii_uppercase() } else { c }
}

struct Chess {
    white: PlayerId,
    black: PlayerId,
    pos: Pos,
    keys: Vec<String>,
    moves: Vec<String>,
    last: Option<(usize, usize)>,
    status: &'static str,
    winner: Option<PlayerId>,
    /// true = Weiß hat Remis angeboten
    draw_offer: Option<bool>,
    by_white: Vec<char>,
    by_black: Vec<char>,
}

impl Chess {
    fn san(&self, m: Mv, legal: &[Mv]) -> String {
        let piece = self.pos.b[m.from].abs();
        if piece == K && (m.to as i32 - m.from as i32).abs() == 2 {
            return if m.to > m.from { "O-O".into() } else { "O-O-O".into() };
        }
        let capture = self.pos.b[m.to] != 0 || (piece == P && m.from % 8 != m.to % 8);
        let mut s = String::new();
        if piece == P {
            if capture {
                s.push((b'a' + (m.from % 8) as u8) as char);
            }
        } else {
            s.push(letter(piece.abs()).to_ascii_uppercase());
            let rivals: Vec<&Mv> = legal.iter().filter(|o| o.to == m.to && o.from != m.from && self.pos.b[o.from].abs() == piece).collect();
            if !rivals.is_empty() {
                let same_file = rivals.iter().any(|o| o.from % 8 == m.from % 8);
                let same_rank = rivals.iter().any(|o| o.from / 8 == m.from / 8);
                if !same_file {
                    s.push((b'a' + (m.from % 8) as u8) as char);
                } else if !same_rank {
                    s.push((b'1' + (m.from / 8) as u8) as char);
                } else {
                    s.push_str(&sq_name(m.from));
                }
            }
        }
        if capture {
            s.push('x');
        }
        s.push_str(&sq_name(m.to));
        if m.promo != 0 {
            s.push('=');
            s.push(letter(m.promo).to_ascii_uppercase());
        }
        s
    }

    fn color_of(&self, p: PlayerId) -> Option<bool> {
        if p == self.white {
            Some(true)
        } else if p == self.black {
            Some(false)
        } else {
            None
        }
    }

    fn other(&self, white: bool) -> PlayerId {
        if white { self.black } else { self.white }
    }
}

impl Game for Chess {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        if self.status != "playing" {
            return Err("Die Partie ist beendet".into());
        }
        let me = self.color_of(p).ok_or("Du spielst nicht mit")?;
        match action.get("type").and_then(Value::as_str) {
            Some("resign") => {
                self.status = "resigned";
                self.winner = Some(self.other(me));
                Ok(())
            }
            Some("offer_draw") => {
                if self.draw_offer == Some(me) {
                    return Err("Du hast schon Remis angeboten".into());
                }
                if self.draw_offer == Some(!me) {
                    self.status = "draw_agreed";
                    return Ok(());
                }
                self.draw_offer = Some(me);
                Ok(())
            }
            Some("accept_draw") => {
                if self.draw_offer == Some(!me) {
                    self.status = "draw_agreed";
                    Ok(())
                } else {
                    Err("Es gibt kein Remis-Angebot".into())
                }
            }
            Some("decline_draw") => {
                if self.draw_offer == Some(!me) {
                    self.draw_offer = None;
                    Ok(())
                } else {
                    Err("Es gibt kein Remis-Angebot".into())
                }
            }
            Some("move") => {
                if self.pos.white != me {
                    return Err("Du bist nicht dran".into());
                }
                let from = action.get("from").and_then(Value::as_str).and_then(parse_sq).ok_or("Ungültiges Startfeld")?;
                let to = action.get("to").and_then(Value::as_str).and_then(parse_sq).ok_or("Ungültiges Zielfeld")?;
                let want = match action.get("promo").and_then(Value::as_str) {
                    Some("r") => R,
                    Some("b") => B,
                    Some("n") => N,
                    _ => Q,
                };
                let legal = self.pos.legal();
                let m = legal
                    .iter()
                    .copied()
                    .find(|m| m.from == from && m.to == to && (m.promo == 0 || m.promo == want))
                    .ok_or("Dieser Zug ist nicht erlaubt")?;
                let mut san = self.san(m, &legal);

                let captured = if self.pos.b[m.to] != 0 {
                    Some(self.pos.b[m.to])
                } else if self.pos.b[m.from].abs() == P && m.from % 8 != m.to % 8 {
                    Some(-self.pos.b[m.from])
                } else {
                    None
                };
                if let Some(c) = captured {
                    if me { self.by_white.push(letter(c)) } else { self.by_black.push(letter(c)) }
                }

                self.pos.make(m);
                self.last = Some((m.from, m.to));
                self.draw_offer = None;
                let key = self.pos.key();
                self.keys.push(key.clone());

                let next_legal = self.pos.legal();
                let check = self.pos.in_check(self.pos.white);
                if next_legal.is_empty() {
                    if check {
                        san.push('#');
                        self.status = "checkmate";
                        self.winner = Some(p);
                    } else {
                        self.status = "stalemate";
                    }
                } else {
                    if check {
                        san.push('+');
                    }
                    if self.pos.half >= 100 {
                        self.status = "draw_50";
                    } else if self.pos.insufficient() {
                        self.status = "draw_material";
                    } else if self.keys.iter().filter(|k| **k == key).count() >= 3 {
                        self.status = "draw_repetition";
                    }
                }
                self.moves.push(san);
                Ok(())
            }
            _ => Err("Unbekannte Aktion".into()),
        }
    }

    fn view(&self, viewer: PlayerId) -> Value {
        let you = match self.color_of(viewer) {
            Some(true) => "w",
            Some(false) => "b",
            None => "",
        };
        let board: Vec<String> = self.pos.b.iter().map(|p| if *p == 0 { String::new() } else { letter(*p).to_string() }).collect();
        let my_turn = self.status == "playing" && self.color_of(viewer) == Some(self.pos.white);
        let legal: Vec<[usize; 2]> = if my_turn {
            let mut v: Vec<[usize; 2]> = self.pos.legal().iter().map(|m| [m.from, m.to]).collect();
            v.dedup();
            v
        } else {
            Vec::new()
        };
        let check = if self.status == "playing" || self.status == "checkmate" {
            if self.pos.in_check(self.pos.white) { self.pos.king_sq(self.pos.white) } else { None }
        } else {
            None
        };
        json!({
            "white": self.white,
            "black": self.black,
            "board": board,
            "turn": if self.pos.white { "w" } else { "b" },
            "you": you,
            "legal": legal,
            "last": self.last.map(|(f, t)| [f, t]),
            "check": check,
            "status": self.status,
            "winner": self.winner,
            "draw_offer": self.draw_offer.map(|w| if w { "w" } else { "b" }),
            "by_w": self.by_white,
            "by_b": self.by_black,
            "moves": self.moves,
        })
    }

    fn finished(&self) -> bool {
        self.status != "playing"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(pos: &Pos, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        pos.legal()
            .iter()
            .map(|m| {
                let mut p = pos.clone();
                p.make(*m);
                count(&p, depth - 1)
            })
            .sum()
    }

    /// Bekannte Perft-Werte der Grundstellung prüfen die komplette Zuggenerierung.
    #[test]
    fn perft_start_position() {
        let p = Pos::start();
        assert_eq!(count(&p, 1), 20);
        assert_eq!(count(&p, 2), 400);
        assert_eq!(count(&p, 3), 8902);
        assert_eq!(count(&p, 4), 197_281);
    }

    fn from_fen(fen: &str) -> Pos {
        let parts: Vec<&str> = fen.split(' ').collect();
        let mut b = [0i8; 64];
        for (ri, row) in parts[0].split('/').enumerate() {
            let r = 7 - ri;
            let mut f = 0;
            for ch in row.chars() {
                if let Some(d) = ch.to_digit(10) {
                    f += d as usize;
                } else {
                    let v = match ch.to_ascii_lowercase() { 'p' => P, 'n' => N, 'b' => B, 'r' => R, 'q' => Q, _ => K };
                    b[r * 8 + f] = if ch.is_ascii_uppercase() { v } else { -v };
                    f += 1;
                }
            }
        }
        Pos {
            b,
            white: parts[1] == "w",
            cas: [parts[2].contains('K'), parts[2].contains('Q'), parts[2].contains('k'), parts[2].contains('q')],
            ep: if parts[3] == "-" { None } else { parse_sq(parts[3]) },
            half: 0,
        }
    }

    #[test]
    fn perft_special_positions() {
        // Kiwipete: Rochade, en passant, Umwandlung, Fesselungen
        let k = from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -");
        assert_eq!(count(&k, 1), 48);
        assert_eq!(count(&k, 2), 2039);
        assert_eq!(count(&k, 3), 97_862);
        // Position 3: en passant mit Schachgebot
        let p3 = from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -");
        assert_eq!(count(&p3, 3), 2812);
        assert_eq!(count(&p3, 4), 43_238);
        // Position 4: Umwandlungen
        let p4 = from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq -");
        assert_eq!(count(&p4, 3), 9467);
    }

    #[test]
    fn fools_mate() {
        let mut g = ChessKind.start(&[1, 2], &Value::Null).unwrap();
        let v = g.view(1);
        let (w, b) = (v["white"].as_u64().unwrap() as u32, v["black"].as_u64().unwrap() as u32);
        for (p, f, t) in [(w, "f2", "f3"), (b, "e7", "e5"), (w, "g2", "g4"), (b, "d8", "h4")] {
            g.apply(p, &json!({"type": "move", "from": f, "to": t})).unwrap();
        }
        let v = g.view(w);
        assert_eq!(v["status"], "checkmate");
        assert_eq!(v["winner"].as_u64().unwrap() as u32, b);
        assert_eq!(v["moves"][3], "Qh4#");
    }
}
