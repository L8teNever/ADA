//! Monopoly: Grundstücke kaufen/versteigern, Miete, Häuser & Hotels, Hypotheken, Handel,
//! Ereignis- und Gemeinschaftskarten, Gefängnis, Steuern, Bankrott. Letzter Spieler gewinnt.

use std::collections::VecDeque;

use rand::{seq::SliceRandom, Rng};
use serde_json::{json, Value};

use super::{Game, GameInfo, GameKind, PlayerId};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Go,
    Prop,
    Rail,
    Util,
    Tax,
    Chance,
    Chest,
    Jail,
    Free,
    GoJail,
}

struct Sq {
    name: &'static str,
    kind: Kind,
    group: u8,
    price: i32,
    house: i32,
    rent: [i32; 6],
}

const Z: [i32; 6] = [0; 6];
const NG: u8 = 255;

const fn sq(name: &'static str, kind: Kind, group: u8, price: i32, house: i32, rent: [i32; 6]) -> Sq {
    Sq { name, kind, group, price, house, rent }
}
const fn prop(name: &'static str, group: u8, price: i32, house: i32, rent: [i32; 6]) -> Sq {
    sq(name, Kind::Prop, group, price, house, rent)
}
const fn other(name: &'static str, kind: Kind) -> Sq {
    sq(name, kind, NG, 0, 0, Z)
}
const fn rail(name: &'static str) -> Sq {
    sq(name, Kind::Rail, 8, 200, 0, Z)
}

static BOARD: [Sq; 40] = [
    other("Los", Kind::Go),
    prop("Mühlenweg", 0, 60, 50, [2, 10, 30, 90, 160, 250]),
    other("Gemeinschaft", Kind::Chest),
    prop("Gerbergasse", 0, 60, 50, [4, 20, 60, 180, 320, 450]),
    sq("Einkommensteuer", Kind::Tax, NG, 200, 0, Z),
    rail("Nordbahnhof"),
    prop("Hafenstraße", 1, 100, 50, [6, 30, 90, 270, 400, 550]),
    other("Ereignis", Kind::Chance),
    prop("Werftallee", 1, 100, 50, [6, 30, 90, 270, 400, 550]),
    prop("Kaistraße", 1, 120, 50, [8, 40, 100, 300, 450, 600]),
    other("Gefängnis", Kind::Jail),
    prop("Rosengasse", 2, 140, 100, [10, 50, 150, 450, 625, 750]),
    sq("Stadtwerke", Kind::Util, 9, 150, 0, Z),
    prop("Lindenallee", 2, 140, 100, [10, 50, 150, 450, 625, 750]),
    prop("Parkstraße", 2, 160, 100, [12, 60, 180, 500, 700, 900]),
    rail("Ostbahnhof"),
    prop("Marktplatz", 3, 180, 100, [14, 70, 200, 550, 750, 950]),
    other("Gemeinschaft", Kind::Chest),
    prop("Rathausstraße", 3, 180, 100, [14, 70, 200, 550, 750, 950]),
    prop("Theaterplatz", 3, 200, 100, [16, 80, 220, 600, 800, 1000]),
    other("Frei Parken", Kind::Free),
    prop("Museumsstraße", 4, 220, 150, [18, 90, 250, 700, 875, 1050]),
    other("Ereignis", Kind::Chance),
    prop("Universitätsring", 4, 220, 150, [18, 90, 250, 700, 875, 1050]),
    prop("Gartenring", 4, 240, 150, [20, 100, 300, 750, 925, 1100]),
    rail("Südbahnhof"),
    prop("Goldgasse", 5, 260, 150, [22, 110, 330, 800, 975, 1150]),
    prop("Silberstraße", 5, 260, 150, [22, 110, 330, 800, 975, 1150]),
    sq("Wasserwerk", Kind::Util, 9, 150, 0, Z),
    prop("Juwelierplatz", 5, 280, 150, [24, 120, 360, 850, 1025, 1200]),
    other("Gehe ins Gefängnis", Kind::GoJail),
    prop("Seeblick", 6, 300, 200, [26, 130, 390, 900, 1100, 1275]),
    prop("Waldweg", 6, 300, 200, [26, 130, 390, 900, 1100, 1275]),
    other("Gemeinschaft", Kind::Chest),
    prop("Villenviertel", 6, 320, 200, [28, 150, 450, 1000, 1200, 1400]),
    rail("Westbahnhof"),
    other("Ereignis", Kind::Chance),
    prop("Börsenplatz", 7, 350, 200, [35, 175, 500, 1100, 1300, 1500]),
    sq("Luxussteuer", Kind::Tax, NG, 100, 0, Z),
    prop("Kronenallee", 7, 400, 200, [50, 200, 600, 1400, 1700, 2000]),
];

#[derive(Clone, Copy)]
enum Eff {
    Gain(i32),
    Pay(i32),
    Move(usize),
    Back3,
    Jail,
    Free,
    NearRail,
    NearUtil,
    Repairs(i32, i32),
    FromEach(i32),
    ToEach(i32),
}

static CHANCE: [(&str, Eff); 16] = [
    ("Rücke vor bis auf Los. Ziehe 200 € ein.", Eff::Move(0)),
    ("Rücke vor bis zum Marktplatz.", Eff::Move(16)),
    ("Rücke vor bis zur Kronenallee.", Eff::Move(39)),
    ("Rücke zum nächsten Bahnhof vor. Gehört er jemandem, zahle das Doppelte der Miete.", Eff::NearRail),
    ("Rücke zum nächsten Bahnhof vor. Gehört er jemandem, zahle das Doppelte der Miete.", Eff::NearRail),
    ("Rücke zum nächsten Versorgungswerk vor. Gehört es jemandem, zahle das Zehnfache der Augenzahl.", Eff::NearUtil),
    ("Bankdividende: Du erhältst 50 €.", Eff::Gain(50)),
    ("Du kommst aus dem Gefängnis frei. Behalte diese Karte.", Eff::Free),
    ("Gehe 3 Felder zurück.", Eff::Back3),
    ("Gehe in das Gefängnis. Gehe nicht über Los.", Eff::Jail),
    ("Allgemeine Reparaturen: 25 € je Haus, 100 € je Hotel.", Eff::Repairs(25, 100)),
    ("Strafe für zu schnelles Fahren: zahle 15 €.", Eff::Pay(15)),
    ("Fahre zum Ostbahnhof. Kommst du über Los, ziehe 200 € ein.", Eff::Move(15)),
    ("Du wirst zum Vorsitzenden gewählt: zahle jedem Spieler 50 €.", Eff::ToEach(50)),
    ("Deine Baudarlehen werfen Zinsen ab: du erhältst 150 €.", Eff::Gain(150)),
    ("Rücke vor bis zum Gartenring.", Eff::Move(24)),
];

static CHEST: [(&str, Eff); 16] = [
    ("Rücke vor bis auf Los. Ziehe 200 € ein.", Eff::Move(0)),
    ("Bankirrtum zu deinen Gunsten: du erhältst 200 €.", Eff::Gain(200)),
    ("Arztkosten: zahle 50 €.", Eff::Pay(50)),
    ("Du verkaufst Aktien: du erhältst 50 €.", Eff::Gain(50)),
    ("Du kommst aus dem Gefängnis frei. Behalte diese Karte.", Eff::Free),
    ("Gehe in das Gefängnis. Gehe nicht über Los.", Eff::Jail),
    ("Du hast Geburtstag: jeder Spieler zahlt dir 50 €.", Eff::FromEach(50)),
    ("Deine Lebensversicherung wird fällig: du erhältst 100 €.", Eff::Gain(100)),
    ("Krankenhausgebühren: zahle 100 €.", Eff::Pay(100)),
    ("Schulgeld: zahle 50 €.", Eff::Pay(50)),
    ("Beratungshonorar: du erhältst 25 €.", Eff::Gain(25)),
    ("Straßenreparaturen: 40 € je Haus, 115 € je Hotel.", Eff::Repairs(40, 115)),
    ("Du gewinnst den zweiten Preis im Schönheitswettbewerb: 10 €.", Eff::Gain(10)),
    ("Du erbst 100 €.", Eff::Gain(100)),
    ("Steuerrückzahlung: du erhältst 20 €.", Eff::Gain(20)),
    ("Dein Urlaubsgeld wird fällig: du erhältst 100 €.", Eff::Gain(100)),
];

const START_MONEY: i32 = 1500;
const GO_BONUS: i32 = 200;
const JAIL_FINE: i32 = 50;
const JAIL: usize = 10;

fn is_ownable(k: Kind) -> bool {
    matches!(k, Kind::Prop | Kind::Rail | Kind::Util)
}

fn kind_str(k: Kind) -> &'static str {
    match k {
        Kind::Go => "go",
        Kind::Prop => "prop",
        Kind::Rail => "rail",
        Kind::Util => "util",
        Kind::Tax => "tax",
        Kind::Chance => "chance",
        Kind::Chest => "chest",
        Kind::Jail => "jail",
        Kind::Free => "free",
        Kind::GoJail => "gojail",
    }
}

fn group_members(g: u8) -> Vec<usize> {
    (0..40).filter(|i| BOARD[*i].group == g && is_ownable(BOARD[*i].kind)).collect()
}

fn unmortgage_cost(pos: usize) -> i32 {
    (BOARD[pos].price / 2 * 11 + 9) / 10
}

pub struct MonopolyKind;

impl GameKind for MonopolyKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "monopoly",
            name: "Monopoly",
            description: "Kaufen, bauen, handeln, versteigern – mit allen Regeln. Der letzte Spieler mit Geld gewinnt.",
            min_players: 2,
            max_players: 6,
            js: "/shared/games/monopoly.js",
            css: "/shared/games/monopoly.css",
        }
    }

    fn start(&self, players: &[PlayerId], _options: &Value) -> Result<Box<dyn Game>, String> {
        if players.len() < 2 || players.len() > 6 {
            return Err("Monopoly braucht 2 bis 6 Spieler".into());
        }
        Ok(Box::new(Monopoly::new(players)))
    }
}

impl Monopoly {
    fn new(players: &[PlayerId]) -> Monopoly {
        let mut rng = rand::thread_rng();
        let mut order = players.to_vec();
        order.shuffle(&mut rng);
        let mut chance: Vec<usize> = (0..16).collect();
        let mut chest: Vec<usize> = (0..16).collect();
        chance.shuffle(&mut rng);
        chest.shuffle(&mut rng);
        Monopoly {
            pls: order
                .iter()
                .map(|id| Pl { id: *id, money: START_MONEY, pos: 0, in_jail: false, jail_fails: 0, cards: 0, out: false })
                .collect(),
            own: [Own::default(); 40],
            phase: Phase::Roll,
            cur: 0,
            dice: (0, 0),
            doubles: 0,
            rolls: 0,
            extra: false,
            buy: 0,
            auction: None,
            debt: None,
            trades: Vec::new(),
            next_trade: 1,
            chance: chance.into(),
            chest: chest.into(),
            houses_left: 32,
            hotels_left: 12,
            log: Vec::new(),
            last_card: None,
            winner: None,
        }
    }
}

struct Pl {
    id: PlayerId,
    money: i32,
    pos: usize,
    in_jail: bool,
    jail_fails: u8,
    cards: u8,
    out: bool,
}

#[derive(Clone, Copy, Default)]
struct Own {
    owner: Option<usize>,
    houses: u8,
    mort: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Roll,
    Buy,
    Auction,
    Debt,
    End,
    Over,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Normal,
    DoubleRail,
    TenUtil,
}

struct Auction {
    pos: usize,
    bid: i32,
    bidder: Option<usize>,
    passed: Vec<bool>,
}

#[derive(Clone)]
struct Debt {
    creditor: Option<usize>,
    amount: i32,
    /// Betrag pro Mitspieler (Karte „zahle jedem Spieler“)
    each: Option<i32>,
    then_move: Option<usize>,
}

#[derive(Clone, Default)]
struct Offer {
    money: i32,
    props: Vec<usize>,
    cards: u8,
}

#[derive(Clone)]
struct Trade {
    id: u32,
    /// Wer das aktuelle Angebot gemacht hat
    from: usize,
    /// Wer antworten muss
    to: usize,
    /// Das gibt `from` ab
    give: Offer,
    /// Das möchte `from` bekommen
    take: Offer,
    rounds: u32,
}

pub struct Monopoly {
    pls: Vec<Pl>,
    own: [Own; 40],
    phase: Phase,
    cur: usize,
    dice: (u8, u8),
    doubles: u8,
    rolls: u32,
    extra: bool,
    buy: usize,
    auction: Option<Auction>,
    debt: Option<Debt>,
    trades: Vec<Trade>,
    next_trade: u32,
    chance: VecDeque<usize>,
    chest: VecDeque<usize>,
    houses_left: i32,
    hotels_left: i32,
    log: Vec<Value>,
    last_card: Option<Value>,
    winner: Option<usize>,
}

fn parse_offer(v: &Value) -> Offer {
    Offer {
        money: v.get("money").and_then(Value::as_i64).unwrap_or(0).clamp(0, 1_000_000) as i32,
        props: v
            .get("props")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).filter(|p| *p < 40).map(|p| p as usize).collect())
            .unwrap_or_default(),
        cards: v.get("cards").and_then(Value::as_u64).unwrap_or(0).min(9) as u8,
    }
}

impl Monopoly {
    fn log(&mut self, p: usize, t: String, q: Option<usize>) {
        let mut e = json!({"p": self.pls[p].id, "t": t});
        if let Some(q) = q {
            e["q"] = json!(self.pls[q].id);
        }
        self.log.push(e);
        if self.log.len() > 30 {
            self.log.remove(0);
        }
    }

    fn alive(&self) -> Vec<usize> {
        (0..self.pls.len()).filter(|i| !self.pls[*i].out).collect()
    }

    fn dice_sum(&self) -> i32 {
        (self.dice.0 + self.dice.1) as i32
    }

    fn owns_group(&self, o: usize, g: u8) -> bool {
        group_members(g).iter().all(|p| self.own[*p].owner == Some(o))
    }

    fn count_owned(&self, o: usize, g: u8) -> i32 {
        group_members(g).iter().filter(|p| self.own[**p].owner == Some(o)).count() as i32
    }

    fn rent(&self, pos: usize, ctx: Ctx) -> i32 {
        let Some(o) = self.own[pos].owner else { return 0 };
        let s = &BOARD[pos];
        match s.kind {
            Kind::Prop => {
                let h = self.own[pos].houses as usize;
                if h > 0 {
                    s.rent[h]
                } else if self.owns_group(o, s.group) {
                    s.rent[0] * 2
                } else {
                    s.rent[0]
                }
            }
            Kind::Rail => {
                let n = self.count_owned(o, 8);
                25 * (1 << (n - 1).max(0)) * if ctx == Ctx::DoubleRail { 2 } else { 1 }
            }
            Kind::Util => {
                let n = self.count_owned(o, 9);
                let mult = if ctx == Ctx::TenUtil || n >= 2 { 10 } else { 4 };
                mult * self.dice_sum()
            }
            _ => 0,
        }
    }

    /// Zahlt, falls möglich – sonst beginnt eine Schuldenphase.
    fn charge(&mut self, i: usize, creditor: Option<usize>, amount: i32, each: Option<i32>, then_move: Option<usize>) -> bool {
        if amount <= 0 {
            return true;
        }
        let debt = Debt { creditor, amount, each, then_move };
        if self.pls[i].money >= amount {
            self.settle(i, &debt);
            true
        } else {
            self.debt = Some(debt);
            self.phase = Phase::Debt;
            false
        }
    }

    fn settle(&mut self, i: usize, d: &Debt) {
        self.pls[i].money -= d.amount;
        if let Some(n) = d.each {
            for j in self.alive() {
                if j != i {
                    self.pls[j].money += n;
                }
            }
        } else if let Some(c) = d.creditor {
            self.pls[c].money += d.amount;
        }
    }

    fn move_steps(&mut self, i: usize, steps: usize) {
        let old = self.pls[i].pos;
        let new = (old + steps) % 40;
        if new < old {
            self.pls[i].money += GO_BONUS;
            self.log(i, format!("zieht über Los und erhält {GO_BONUS} €"), None);
        }
        self.pls[i].pos = new;
    }

    fn move_to(&mut self, i: usize, target: usize) {
        let old = self.pls[i].pos;
        if target < old {
            self.pls[i].money += GO_BONUS;
            self.log(i, format!("zieht über Los und erhält {GO_BONUS} €"), None);
        }
        self.pls[i].pos = target;
    }

    fn send_to_jail(&mut self, i: usize) {
        self.pls[i].pos = JAIL;
        self.pls[i].in_jail = true;
        self.pls[i].jail_fails = 0;
        self.extra = false;
        self.doubles = 0;
        self.log(i, "kommt ins Gefängnis".into(), None);
    }

    fn land(&mut self, i: usize, ctx: Ctx) {
        self.phase = Phase::End;
        let pos = self.pls[i].pos;
        let s = &BOARD[pos];
        match s.kind {
            Kind::Prop | Kind::Rail | Kind::Util => match self.own[pos].owner {
                None => {
                    self.buy = pos;
                    self.phase = Phase::Buy;
                }
                Some(o) if o != i => {
                    if !self.own[pos].mort {
                        let rent = self.rent(pos, ctx);
                        self.log(i, format!("zahlt {rent} € Miete für {} an", s.name), Some(o));
                        self.charge(i, Some(o), rent, None, None);
                    }
                }
                _ => {}
            },
            Kind::Tax => {
                self.log(i, format!("zahlt {} € ({})", s.price, s.name), None);
                self.charge(i, None, s.price, None, None);
            }
            Kind::Chance => self.draw(i, true),
            Kind::Chest => self.draw(i, false),
            Kind::GoJail => self.send_to_jail(i),
            _ => {}
        }
    }

    fn draw(&mut self, i: usize, chance: bool) {
        let idx = if chance { self.chance.pop_front() } else { self.chest.pop_front() }.unwrap_or(0);
        let (text, eff) = if chance { CHANCE[idx] } else { CHEST[idx] };
        self.last_card = Some(json!({"deck": if chance { "chance" } else { "chest" }, "text": text, "p": self.pls[i].id}));
        self.log(i, format!("zieht eine Karte: {text}"), None);
        if !matches!(eff, Eff::Free) {
            if chance { self.chance.push_back(idx) } else { self.chest.push_back(idx) }
        }
        match eff {
            Eff::Gain(n) => self.pls[i].money += n,
            Eff::Pay(n) => {
                self.charge(i, None, n, None, None);
            }
            Eff::Move(t) => {
                self.move_to(i, t);
                self.land(i, Ctx::Normal);
            }
            Eff::Back3 => {
                self.pls[i].pos = (self.pls[i].pos + 37) % 40;
                self.land(i, Ctx::Normal);
            }
            Eff::Jail => self.send_to_jail(i),
            Eff::Free => self.pls[i].cards += 1,
            Eff::NearRail | Eff::NearUtil => {
                let list: [usize; 4] = [5, 15, 25, 35];
                let utils: [usize; 2] = [12, 28];
                let pos = self.pls[i].pos;
                let target = if matches!(eff, Eff::NearRail) {
                    list.iter().copied().find(|p| *p > pos).unwrap_or(5)
                } else {
                    utils.iter().copied().find(|p| *p > pos).unwrap_or(12)
                };
                self.move_to(i, target);
                self.land(i, if matches!(eff, Eff::NearRail) { Ctx::DoubleRail } else { Ctx::TenUtil });
            }
            Eff::Repairs(h, hot) => {
                let (mut houses, mut hotels) = (0, 0);
                for p in 0..40 {
                    if self.own[p].owner == Some(i) {
                        if self.own[p].houses == 5 {
                            hotels += 1;
                        } else {
                            houses += self.own[p].houses as i32;
                        }
                    }
                }
                let cost = houses * h + hotels * hot;
                self.charge(i, None, cost, None, None);
            }
            Eff::FromEach(n) => {
                for j in self.alive() {
                    if j != i {
                        let x = n.min(self.pls[j].money);
                        self.pls[j].money -= x;
                        self.pls[i].money += x;
                    }
                }
            }
            Eff::ToEach(n) => {
                let total = n * (self.alive().len() as i32 - 1);
                self.charge(i, None, total, Some(n), None);
            }
        }
    }

    fn next_player(&mut self) {
        let n = self.pls.len();
        loop {
            self.cur = (self.cur + 1) % n;
            if !self.pls[self.cur].out {
                break;
            }
        }
        self.doubles = 0;
        self.extra = false;
        self.phase = Phase::Roll;
    }

    fn check_over(&mut self) -> bool {
        let alive = self.alive();
        if alive.len() <= 1 {
            self.phase = Phase::Over;
            self.winner = alive.first().copied();
            true
        } else {
            false
        }
    }

    fn bankrupt(&mut self, i: usize, creditor: Option<usize>) {
        let money = self.pls[i].money.max(0);
        let cards = self.pls[i].cards;
        for p in 0..40 {
            if self.own[p].owner == Some(i) {
                let h = self.own[p].houses as i32;
                if h == 5 {
                    self.hotels_left += 1;
                } else {
                    self.houses_left += h;
                }
                self.own[p].houses = 0;
                match creditor {
                    Some(c) => self.own[p].owner = Some(c),
                    None => {
                        self.own[p].owner = None;
                        self.own[p].mort = false;
                    }
                }
            }
        }
        if let Some(c) = creditor {
            self.pls[c].money += money;
            self.pls[c].cards += cards;
        }
        self.pls[i].money = 0;
        self.pls[i].cards = 0;
        self.pls[i].out = true;
        self.debt = None;
        self.trades.retain(|t| t.from != i && t.to != i);
        let t = if creditor.is_some() { "ist bankrott, Besitz geht an" } else { "ist bankrott" };
        self.log(i, t.into(), creditor);
        if !self.check_over() {
            self.next_player();
        }
    }

    /// Bargeld plus alles, was sich durch Hypotheken und Hausverkäufe flüssig machen ließe.
    fn liquid(&self, j: usize) -> i32 {
        let mut total = self.pls[j].money;
        for p in 0..40 {
            if self.own[p].owner == Some(j) {
                if !self.own[p].mort {
                    total += BOARD[p].price / 2;
                }
                total += self.own[p].houses as i32 * BOARD[p].house / 2;
            }
        }
        total
    }

    fn auction_check(&mut self) {
        let Some(a) = &self.auction else { return };
        // Wer das Gebot nicht mehr überbieten kann, scheidet automatisch aus
        let pending = (0..self.pls.len())
            .filter(|j| !self.pls[*j].out && !a.passed[*j] && Some(*j) != a.bidder && self.liquid(*j) > a.bid)
            .count();
        if pending > 0 {
            return;
        }
        let (pos, bid, bidder) = (a.pos, a.bid, a.bidder);
        self.auction = None;
        self.phase = Phase::End;
        if let Some(b) = bidder {
            self.pls[b].money -= bid;
            self.own[pos].owner = Some(b);
            self.log(b, format!("ersteigert {} für {bid} €", BOARD[pos].name), None);
        } else {
            self.log(self.cur, format!("– niemand wollte {}", BOARD[pos].name), None);
        }
    }

    fn do_roll(&mut self, i: usize) {
        let mut rng = rand::thread_rng();
        let (d1, d2) = (rng.gen_range(1..=6u8), rng.gen_range(1..=6u8));
        self.dice = (d1, d2);
        self.rolls += 1;
        let sum = (d1 + d2) as usize;
        let dbl = d1 == d2;
        self.log(i, format!("würfelt {d1} + {d2}"), None);
        if self.pls[i].in_jail {
            if dbl {
                self.pls[i].in_jail = false;
                self.pls[i].jail_fails = 0;
                self.extra = false;
                self.move_steps(i, sum);
                self.land(i, Ctx::Normal);
            } else {
                self.pls[i].jail_fails += 1;
                if self.pls[i].jail_fails >= 3 {
                    self.pls[i].in_jail = false;
                    self.pls[i].jail_fails = 0;
                    self.log(i, format!("zahlt {JAIL_FINE} € und verlässt das Gefängnis"), None);
                    if !self.charge(i, None, JAIL_FINE, None, Some(sum)) {
                        return;
                    }
                    self.move_steps(i, sum);
                    self.land(i, Ctx::Normal);
                } else {
                    self.phase = Phase::End;
                }
            }
            return;
        }
        if dbl {
            self.doubles += 1;
            if self.doubles >= 3 {
                self.send_to_jail(i);
                self.phase = Phase::End;
                return;
            }
            self.extra = true;
        } else {
            self.doubles = 0;
            self.extra = false;
        }
        self.move_steps(i, sum);
        self.land(i, Ctx::Normal);
    }

    fn valid_offer(&self, owner: usize, o: &Offer) -> Result<(), String> {
        if o.money > self.pls[owner].money {
            return Err("Nicht genug Geld für das Angebot".into());
        }
        if o.cards > self.pls[owner].cards {
            return Err("So viele Freikarten hast du nicht".into());
        }
        for p in &o.props {
            let s = &BOARD[*p];
            if !is_ownable(s.kind) || self.own[*p].owner != Some(owner) {
                return Err(format!("{} gehört nicht dem Anbieter", s.name));
            }
            if s.kind == Kind::Prop && group_members(s.group).iter().any(|m| self.own[*m].houses > 0) {
                return Err(format!("{}: In der Farbgruppe stehen Häuser – erst verkaufen", s.name));
            }
        }
        Ok(())
    }

    fn group_ok_for_build(&self, i: usize, pos: usize) -> Result<(), String> {
        let s = &BOARD[pos];
        if s.kind != Kind::Prop || self.own[pos].owner != Some(i) {
            return Err("Das Grundstück gehört dir nicht".into());
        }
        if !self.owns_group(i, s.group) {
            return Err("Dir gehört nicht die ganze Farbgruppe".into());
        }
        Ok(())
    }

    fn trade_action(&mut self, i: usize, kind: &str, action: &Value) -> Result<(), String> {
        let offers = |a: &Value| (parse_offer(a.get("give").unwrap_or(&Value::Null)), parse_offer(a.get("take").unwrap_or(&Value::Null)));
        if kind == "trade_propose" {
            let to_id = action.get("to").and_then(Value::as_u64).ok_or("Partner fehlt")? as u32;
            let to = self.pls.iter().position(|x| x.id == to_id).ok_or("Unbekannter Mitspieler")?;
            if to == i || self.pls[to].out {
                return Err("Mit diesem Spieler kannst du nicht handeln".into());
            }
            let (give, take) = offers(action);
            if give.money == 0 && give.props.is_empty() && give.cards == 0 && take.money == 0 && take.props.is_empty() && take.cards == 0 {
                return Err("Das Angebot ist leer".into());
            }
            if self.trades.iter().any(|t| (t.from == i && t.to == to) || (t.from == to && t.to == i)) {
                return Err("Mit diesem Spieler ist schon ein Angebot offen – beantworte oder ändere es".into());
            }
            if self.trades.iter().filter(|t| t.from == i).count() >= 4 {
                return Err("Du hast schon zu viele offene Angebote".into());
            }
            self.valid_offer(i, &give)?;
            self.valid_offer(to, &take)?;
            let id = self.next_trade;
            self.next_trade += 1;
            self.trades.push(Trade { id, from: i, to, give, take, rounds: 0 });
            self.log(i, "macht ein Handelsangebot an".into(), Some(to));
            return Ok(());
        }
        let id = action.get("id").and_then(Value::as_u64).ok_or("Angebot fehlt")? as u32;
        let pos = self.trades.iter().position(|t| t.id == id).ok_or("Dieses Angebot gibt es nicht mehr")?;
        let (from, to) = (self.trades[pos].from, self.trades[pos].to);
        match kind {
            "trade_cancel" => {
                if from != i {
                    return Err("Nur wer das Angebot gemacht hat, kann es zurückziehen".into());
                }
                self.trades.remove(pos);
                self.log(i, "zieht das Handelsangebot an".into(), Some(to));
            }
            "trade_decline" => {
                if to != i {
                    return Err("Das Angebot ist nicht an dich gerichtet".into());
                }
                self.trades.remove(pos);
                self.log(i, "lehnt das Handelsangebot von".into(), Some(from));
            }
            "trade_counter" => {
                if to != i {
                    return Err("Das Angebot ist nicht an dich gerichtet".into());
                }
                // Aus Sicht des Antwortenden: give = was ich abgebe, take = was ich bekomme
                let (give, take) = offers(action);
                if give.money == 0 && give.props.is_empty() && give.cards == 0 && take.money == 0 && take.props.is_empty() && take.cards == 0 {
                    return Err("Das Angebot ist leer".into());
                }
                self.valid_offer(i, &give)?;
                self.valid_offer(from, &take)?;
                let t = &mut self.trades[pos];
                t.from = i;
                t.to = from;
                t.give = give;
                t.take = take;
                t.rounds += 1;
                self.log(i, "macht ein Gegenangebot an".into(), Some(from));
            }
            _ => {
                if to != i {
                    return Err("Das Angebot ist nicht an dich gerichtet".into());
                }
                self.execute_trade(pos)?;
            }
        }
        Ok(())
    }

    fn execute_trade(&mut self, pos: usize) -> Result<(), String> {
        let t = self.trades[pos].clone();
        let (from, to) = (t.from, t.to);
        self.valid_offer(from, &t.give)?;
        self.valid_offer(to, &t.take)?;
        // Wer ein Grundstück mit Hypothek übernimmt, zahlt 10 % des Hypothekenwerts an die Bank
        let fee = |props: &Vec<usize>| -> i32 { props.iter().filter(|p| self.own[**p].mort).map(|p| (BOARD[*p].price / 2 + 9) / 10).sum() };
        let (to_fee, from_fee) = (fee(&t.give.props), fee(&t.take.props));
        let new_from = self.pls[from].money - t.give.money + t.take.money - from_fee;
        let new_to = self.pls[to].money - t.take.money + t.give.money - to_fee;
        if new_from < 0 || new_to < 0 {
            return Err("Nicht genug Geld (inklusive 10 % Gebühr für übernommene Hypotheken)".into());
        }
        self.pls[from].money = new_from;
        self.pls[to].money = new_to;
        for (src, dst, o) in [(from, to, &t.give), (to, from, &t.take)] {
            self.pls[src].cards -= o.cards;
            self.pls[dst].cards += o.cards;
            for pr in &o.props {
                self.own[*pr].owner = Some(dst);
            }
        }
        self.trades.remove(pos);
        self.log(to, "tauscht mit".into(), Some(from));
        // Andere Angebote, die nun nicht mehr gültig sind, verfallen
        let stale: Vec<u32> = self
            .trades
            .iter()
            .filter(|x| self.valid_offer(x.from, &x.give).is_err() || self.valid_offer(x.to, &x.take).is_err())
            .map(|x| x.id)
            .collect();
        self.trades.retain(|x| !stale.contains(&x.id));
        Ok(())
    }

    /// Bauen, Häuser verkaufen, Hypothek aufnehmen/auslösen (Berechtigung prüft der Aufrufer).
    fn manage(&mut self, i: usize, kind: &str, pos: usize) -> Result<(), String> {
        let s = &BOARD[pos];
                match kind {
                    "build" => {
                        self.group_ok_for_build(i, pos)?;
                        let grp = group_members(s.group);
                        if grp.iter().any(|m| self.own[*m].mort) {
                            return Err("In der Gruppe liegt eine Hypothek".into());
                        }
                        let h = self.own[pos].houses;
                        if h >= 5 {
                            return Err("Hier steht schon ein Hotel".into());
                        }
                        let min = grp.iter().map(|m| self.own[*m].houses).min().unwrap_or(0);
                        if h > min {
                            return Err("Baue gleichmäßig – erst auf den anderen Grundstücken".into());
                        }
                        if self.pls[i].money < s.house {
                            return Err("Nicht genug Geld".into());
                        }
                        if h == 4 {
                            if self.hotels_left <= 0 {
                                return Err("Keine Hotels mehr in der Bank".into());
                            }
                            self.hotels_left -= 1;
                            self.houses_left += 4;
                        } else {
                            if self.houses_left <= 0 {
                                return Err("Keine Häuser mehr in der Bank".into());
                            }
                            self.houses_left -= 1;
                        }
                        self.pls[i].money -= s.house;
                        self.own[pos].houses += 1;
                        let name = s.name;
                        self.log(i, format!("baut auf {name}"), None);
                        Ok(())
                    }
                    "sell_house" => {
                        if s.kind != Kind::Prop || self.own[pos].owner != Some(i) {
                            return Err("Das Grundstück gehört dir nicht".into());
                        }
                        let h = self.own[pos].houses;
                        if h == 0 {
                            return Err("Hier steht kein Haus".into());
                        }
                        let max = group_members(s.group).iter().map(|m| self.own[*m].houses).max().unwrap_or(0);
                        if h < max {
                            return Err("Verkaufe gleichmäßig – erst auf den anderen Grundstücken".into());
                        }
                        if h == 5 {
                            if self.houses_left < 4 {
                                return Err("Nicht genug Häuser in der Bank, um das Hotel zu ersetzen".into());
                            }
                            self.houses_left -= 4;
                            self.hotels_left += 1;
                        } else {
                            self.houses_left += 1;
                        }
                        self.own[pos].houses -= 1;
                        self.pls[i].money += s.house / 2;
                        let name = s.name;
                        self.log(i, format!("verkauft ein Haus auf {name}"), None);
                        Ok(())
                    }
                    "mortgage" => {
                        if !is_ownable(s.kind) || self.own[pos].owner != Some(i) {
                            return Err("Das Grundstück gehört dir nicht".into());
                        }
                        if self.own[pos].mort {
                            return Err("Schon mit Hypothek belegt".into());
                        }
                        if s.kind == Kind::Prop && group_members(s.group).iter().any(|m| self.own[*m].houses > 0) {
                            return Err("Verkaufe erst alle Häuser der Farbgruppe".into());
                        }
                        self.own[pos].mort = true;
                        self.pls[i].money += s.price / 2;
                        let name = s.name;
                        self.log(i, format!("nimmt eine Hypothek auf {name}"), None);
                        Ok(())
                    }
                    _ => {
                        if self.own[pos].owner != Some(i) || !self.own[pos].mort {
                            return Err("Keine Hypothek auf diesem Grundstück".into());
                        }
                        let cost = unmortgage_cost(pos);
                        if self.pls[i].money < cost {
                            return Err("Nicht genug Geld".into());
                        }
                        self.pls[i].money -= cost;
                        self.own[pos].mort = false;
                        let name = s.name;
                        self.log(i, format!("löst die Hypothek auf {name} aus"), None);
                        Ok(())
                    }
                }
    }
}

impl Game for Monopoly {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        let i = self.pls.iter().position(|x| x.id == p).ok_or("Du bist nicht in diesem Spiel")?;
        if self.pls[i].out {
            return Err("Du bist ausgeschieden".into());
        }
        if self.phase == Phase::Over {
            return Err("Das Spiel ist vorbei".into());
        }
        let kind = action.get("type").and_then(Value::as_str).unwrap_or("");
        let pos_arg = || -> Result<usize, String> {
            let p = action.get("pos").and_then(Value::as_u64).ok_or("Feld fehlt")? as usize;
            if p < 40 { Ok(p) } else { Err("Ungültiges Feld".into()) }
        };

        // ---- Aktionen, die nicht nur der aktive Spieler ausführen darf ----
        match kind {
            "bid" | "pass" => {
                if self.phase != Phase::Auction {
                    return Err("Gerade läuft keine Auktion".into());
                }
                let a = self.auction.as_mut().ok_or("Keine Auktion")?;
                if a.passed[i] {
                    return Err("Du hast schon gepasst".into());
                }
                if kind == "pass" {
                    a.passed[i] = true;
                    self.log(i, "passt".into(), None);
                } else {
                    let amount = action.get("amount").and_then(Value::as_i64).ok_or("Gebot fehlt")?;
                    if amount <= a.bid as i64 {
                        return Err("Dein Gebot muss höher sein".into());
                    }
                    if amount > self.pls[i].money as i64 {
                        return Err("So viel Geld hast du nicht".into());
                    }
                    a.bid = amount as i32;
                    a.bidder = Some(i);
                    self.log(i, format!("bietet {amount} €"), None);
                }
                self.auction_check();
                return Ok(());
            }
            "trade_propose" | "trade_counter" | "trade_accept" | "trade_decline" | "trade_cancel" => {
                if self.phase == Phase::Auction {
                    return Err("Während einer Auktion kann nicht gehandelt werden".into());
                }
                return self.trade_action(i, kind, action);
            }
            "build" | "sell_house" | "mortgage" | "unmortgage" => {
                let pos = pos_arg()?;
                let light = kind == "sell_house" || kind == "mortgage";
                let ok = match self.phase {
                    Phase::Roll | Phase::End => i == self.cur,
                    Phase::Buy | Phase::Debt => i == self.cur && light,
                    // In der Auktion darf jeder Geld flüssig machen, um mitzubieten
                    Phase::Auction => light,
                    Phase::Over => false,
                };
                if !ok {
                    return Err("Gerade nicht möglich".into());
                }
                return self.manage(i, kind, pos);
            }
            _ => {}
        }

        if i != self.cur {
            return Err("Du bist nicht dran".into());
        }

        match kind {
            "roll" => {
                if self.phase != Phase::Roll {
                    return Err("Du kannst gerade nicht würfeln".into());
                }
                self.do_roll(i);
                Ok(())
            }
            "pay_jail" | "use_card" => {
                if self.phase != Phase::Roll || !self.pls[i].in_jail {
                    return Err("Du bist nicht im Gefängnis".into());
                }
                if kind == "use_card" {
                    if self.pls[i].cards == 0 {
                        return Err("Du hast keine Freikarte".into());
                    }
                    self.pls[i].cards -= 1;
                    self.log(i, "nutzt die Freikarte".into(), None);
                } else {
                    if self.pls[i].money < JAIL_FINE {
                        return Err("Nicht genug Geld".into());
                    }
                    self.pls[i].money -= JAIL_FINE;
                    self.log(i, format!("zahlt {JAIL_FINE} € und verlässt das Gefängnis"), None);
                }
                self.pls[i].in_jail = false;
                self.pls[i].jail_fails = 0;
                Ok(())
            }
            "buy" => {
                if self.phase != Phase::Buy {
                    return Err("Es gibt nichts zu kaufen".into());
                }
                let pos = self.buy;
                let price = BOARD[pos].price;
                if self.pls[i].money < price {
                    return Err("Nicht genug Geld – starte eine Auktion".into());
                }
                self.pls[i].money -= price;
                self.own[pos].owner = Some(i);
                self.phase = Phase::End;
                self.log(i, format!("kauft {} für {price} €", BOARD[pos].name), None);
                Ok(())
            }
            "decline_buy" => {
                if self.phase != Phase::Buy {
                    return Err("Es gibt nichts zu kaufen".into());
                }
                let passed: Vec<bool> = self.pls.iter().map(|x| x.out).collect();
                self.auction = Some(Auction { pos: self.buy, bid: 0, bidder: None, passed });
                self.phase = Phase::Auction;
                self.log(i, format!("versteigert {}", BOARD[self.buy].name), None);
                Ok(())
            }
            "end_turn" => {
                if self.phase != Phase::End {
                    return Err("Dein Zug ist noch nicht fertig".into());
                }
                if self.extra && !self.pls[i].in_jail {
                    self.phase = Phase::Roll;
                } else {
                    self.next_player();
                }
                Ok(())
            }
            "pay_debt" => {
                if self.phase != Phase::Debt {
                    return Err("Du hast keine Schulden".into());
                }
                let d = self.debt.clone().ok_or("Keine Schulden")?;
                if self.pls[i].money < d.amount {
                    return Err("Du hast noch nicht genug Geld – verkaufe Häuser oder nimm Hypotheken auf".into());
                }
                self.settle(i, &d);
                self.debt = None;
                self.phase = Phase::End;
                if let Some(steps) = d.then_move {
                    self.move_steps(i, steps);
                    self.land(i, Ctx::Normal);
                }
                Ok(())
            }
            "surrender" => {
                if !matches!(self.phase, Phase::Roll | Phase::End | Phase::Debt) {
                    return Err("Gerade nicht möglich".into());
                }
                let creditor = if self.phase == Phase::Debt { self.debt.as_ref().and_then(|d| d.creditor) } else { None };
                self.bankrupt(i, creditor);
                Ok(())
            }
            _ => Err("Unbekannte Aktion".into()),
        }
    }

    fn view(&self, viewer: PlayerId) -> Value {
        let board: Vec<Value> = BOARD
            .iter()
            .map(|s| {
                json!({"n": s.name, "t": kind_str(s.kind), "g": if s.group == NG { Value::Null } else { json!(s.group) },
                       "p": s.price, "h": s.house, "r": s.rent})
            })
            .collect();
        let own: Vec<Value> = (0..40)
            .map(|p| {
                if is_ownable(BOARD[p].kind) {
                    let o = &self.own[p];
                    json!({"o": o.owner.map(|x| self.pls[x].id), "h": o.houses, "m": o.mort})
                } else {
                    Value::Null
                }
            })
            .collect();
        let players: Vec<Value> = self
            .pls
            .iter()
            .map(|p| json!({"id": p.id, "money": p.money, "pos": p.pos, "jail": p.in_jail, "cards": p.cards, "out": p.out}))
            .collect();
        let offer_json = |o: &Offer| json!({"money": o.money, "props": o.props, "cards": o.cards});
        json!({
            "phase": match self.phase { Phase::Roll => "roll", Phase::Buy => "buy", Phase::Auction => "auction", Phase::Debt => "debt", Phase::End => "end", Phase::Over => "over" },
            "current": self.pls[self.cur].id,
            "players": players,
            "board": board,
            "own": own,
            "dice": if self.dice.0 > 0 { json!([self.dice.0, self.dice.1]) } else { Value::Null },
            "extra": self.extra,
            "rolls": self.rolls,
            "buy": if self.phase == Phase::Buy { json!(self.buy) } else { Value::Null },
            "auction": self.auction.as_ref().map(|a| json!({
                "pos": a.pos, "bid": a.bid, "bidder": a.bidder.map(|b| self.pls[b].id),
                "passed": (0..self.pls.len()).filter(|j| a.passed[*j] && !self.pls[*j].out).map(|j| self.pls[j].id).collect::<Vec<_>>(),
            })),
            "debt": self.debt.as_ref().map(|d| json!({
                "creditor": d.creditor.map(|c| self.pls[c].id), "amount": d.amount, "each": d.each,
            })),
            "trades": self.trades.iter().filter(|t| self.pls[t.from].id == viewer || self.pls[t.to].id == viewer).map(|t| json!({
                "id": t.id, "from": self.pls[t.from].id, "to": self.pls[t.to].id,
                "give": offer_json(&t.give), "take": offer_json(&t.take), "rounds": t.rounds,
            })).collect::<Vec<_>>(),
            "houses_left": self.houses_left,
            "hotels_left": self.hotels_left,
            "card": self.last_card,
            "log": self.log,
            "winner": self.winner.map(|w| self.pls[w].id),
        })
    }

    fn finished(&self) -> bool {
        self.phase == Phase::Over
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me(v: &Value, id: u64) -> Value {
        v["players"].as_array().unwrap().iter().find(|p| p["id"] == id).unwrap().clone()
    }

    /// Zufallsspieler: darf nie abstürzen, Zustände bleiben gültig, Spiele enden.
    #[test]
    fn random_bots_play_full_games() {
        let mut rng = rand::thread_rng();
        let mut finished = 0;
        for _ in 0..6 {
            let mut g = MonopolyKind.start(&[1, 2, 3, 4], &Value::Null).unwrap();
            for _ in 0..20000 {
                let v = g.view(1);
                let phase = v["phase"].as_str().unwrap().to_string();
                if phase == "over" {
                    finished += 1;
                    break;
                }
                let cur = v["current"].as_u64().unwrap() as u32;
                let cm = me(&v, cur as u64);
                match phase.as_str() {
                    "roll" => {
                        if cm["jail"] == true && rng.gen_bool(0.3) {
                            let _ = g.apply(cur, &json!({"type": if cm["cards"].as_u64().unwrap() > 0 { "use_card" } else { "pay_jail" }}));
                        }
                        // zufällig bauen / Hypothek / Handel
                        let pos = rng.gen_range(0..40);
                        let t = ["build", "mortgage", "unmortgage", "sell_house"][rng.gen_range(0..4)];
                        let _ = g.apply(cur, &json!({"type": t, "pos": pos}));
                        g.apply(cur, &json!({"type": "roll"})).ok();
                    }
                    "buy" => {
                        let t = if rng.gen_bool(0.6) { "buy" } else { "decline_buy" };
                        if g.apply(cur, &json!({"type": t})).is_err() {
                            g.apply(cur, &json!({"type": "decline_buy"})).unwrap();
                        }
                    }
                    "auction" => {
                        for pid in [1u32, 2, 3, 4] {
                            if v["players"].as_array().unwrap().iter().any(|p| p["id"] == pid && p["out"] == false) {
                                let bid = v["auction"]["bid"].as_i64().unwrap() + rng.gen_range(1..60);
                                if rng.gen_bool(0.5) {
                                    if g.apply(pid, &json!({"type": "bid", "amount": bid})).is_err() {
                                        let _ = g.apply(pid, &json!({"type": "pass"}));
                                    }
                                } else {
                                    let _ = g.apply(pid, &json!({"type": "pass"}));
                                }
                            }
                        }
                    }
                    "debt" => {
                        for pos in 0..40 {
                            let _ = g.apply(cur, &json!({"type": "sell_house", "pos": pos}));
                        }
                        for pos in 0..40 {
                            if g.apply(cur, &json!({"type": "pay_debt"})).is_ok() {
                                break;
                            }
                            let _ = g.apply(cur, &json!({"type": "mortgage", "pos": pos}));
                        }
                        if g.view(1)["phase"] == "debt" {
                            g.apply(cur, &json!({"type": "surrender"})).unwrap();
                        }
                    }
                    "end" => {
                        let pos = rng.gen_range(0..40);
                        let t = ["build", "build", "mortgage", "unmortgage"][rng.gen_range(0..4)];
                        let _ = g.apply(cur, &json!({"type": t, "pos": pos}));
                        if rng.gen_bool(0.05) {
                            let other = [1u32, 2, 3, 4][rng.gen_range(0..4)];
                            let mine: Vec<u64> = v["own"].as_array().unwrap().iter().enumerate().filter(|(_, o)| o["o"] == cur).map(|(i, _)| i as u64).collect();
                            let theirs: Vec<u64> = v["own"].as_array().unwrap().iter().enumerate().filter(|(_, o)| o["o"] == other).map(|(i, _)| i as u64).collect();
                            let _ = g.apply(cur, &json!({"type": "trade_propose", "to": other,
                                "give": {"money": 50, "props": mine.iter().take(1).collect::<Vec<_>>()},
                                "take": {"props": theirs.iter().take(1).collect::<Vec<_>>()}}));
                            if let Some(t) = g.view(cur as u32)["trades"].as_array().and_then(|a| a.first().cloned()) {
                                let id = t["id"].clone();
                                if rng.gen_bool(0.5) {
                                    let _ = g.apply(t["to"].as_u64().unwrap() as u32, &json!({"type": "trade_counter", "id": id, "give": {"money": 10}, "take": {"money": 20}}));
                                }
                                let id = g.view(1)["trades"].as_array().and_then(|a| a.first().map(|t| t["id"].clone())).unwrap_or(id);
                                let to = g.view(1)["trades"].as_array().and_then(|a| a.first().map(|t| t["to"].as_u64().unwrap() as u32)).unwrap_or(other);
                                let _ = g.apply(to, &json!({"type": "trade_accept", "id": id}));
                                let _ = g.apply(cur, &json!({"type": "trade_cancel", "id": id}));
                            }
                        }
                        g.apply(cur, &json!({"type": "end_turn"})).unwrap();
                    }
                    _ => {}
                }
                // Invarianten
                let v = g.view(1);
                for p in v["players"].as_array().unwrap() {
                    assert!(p["out"] == true || v["phase"] == "debt" || p["money"].as_i64().unwrap() >= 0, "negatives Geld: {p}");
                }
                assert!(v["houses_left"].as_i64().unwrap() >= 0 && v["houses_left"].as_i64().unwrap() <= 32);
                assert!(v["hotels_left"].as_i64().unwrap() >= 0 && v["hotels_left"].as_i64().unwrap() <= 12);
            }
        }
        assert!(finished > 0, "kein Spiel wurde beendet");
    }

    #[test]
    fn board_is_consistent() {
        assert_eq!(BOARD.iter().filter(|s| s.kind == Kind::Prop).count(), 22);
        assert_eq!(BOARD.iter().filter(|s| s.kind == Kind::Rail).count(), 4);
        assert_eq!(group_members(9).len(), 2);
        assert_eq!(unmortgage_cost(1), 33);
    }

    fn game() -> Monopoly {
        let mut g = Monopoly::new(&[10, 20, 30]);
        g.pls.sort_by_key(|p| p.id);
        g.cur = 0;
        g
    }

    #[test]
    fn mortgage_while_buying_and_in_auction() {
        let mut g = game();
        // Spieler 0 besitzt Braun + Hellblau (ganze Gruppen), hat aber kaum Bargeld
        for p in [1usize, 3, 6, 8, 9] {
            g.own[p].owner = Some(0);
        }
        g.pls[0].money = 10;
        g.phase = Phase::Buy;
        g.buy = 11; // Rosengasse 140 €
        assert!(g.apply(10, &json!({"type": "buy"})).is_err());
        for p in [1, 3, 6, 8] {
            g.apply(10, &json!({"type": "mortgage", "pos": p})).unwrap();
        }
        // 10 + 30 + 30 + 50 + 50 = 170
        assert_eq!(g.pls[0].money, 170);
        // Andere dürfen in der Kauf-Phase nicht
        assert!(g.apply(20, &json!({"type": "mortgage", "pos": 9})).is_err());
        g.apply(10, &json!({"type": "buy"})).unwrap();
        assert_eq!(g.own[11].owner, Some(0));
        assert!(g.phase == Phase::End);

        // Auktion: auch Nicht-Aktive dürfen Hypotheken aufnehmen
        let mut g = game();
        g.own[39].owner = Some(1); // Kronenallee
        g.pls[1].money = 5;
        g.buy = 37;
        g.phase = Phase::Buy;
        g.apply(10, &json!({"type": "decline_buy"})).unwrap();
        assert!(g.phase == Phase::Auction);
        g.apply(20, &json!({"type": "mortgage", "pos": 39})).unwrap(); // +200
        assert_eq!(g.pls[1].money, 205);
        g.apply(20, &json!({"type": "bid", "amount": 205})).unwrap();
        // Spieler 0 und 2 können mitbieten, bis sie passen
        g.apply(10, &json!({"type": "pass"})).unwrap();
        g.apply(30, &json!({"type": "pass"})).unwrap();
        assert_eq!(g.own[37].owner, Some(1));
    }

    #[test]
    fn auction_waits_for_player_who_can_raise_cash() {
        let mut g = game();
        g.pls[1].money = 0;
        g.own[39].owner = Some(1); // könnte 200 € per Hypothek holen
        g.buy = 37;
        g.phase = Phase::Buy;
        g.apply(10, &json!({"type": "decline_buy"})).unwrap();
        g.apply(10, &json!({"type": "bid", "amount": 50})).unwrap();
        g.apply(30, &json!({"type": "pass"})).unwrap();
        // Spieler 1 hat 0 €, kann aber flüssig machen -> Auktion läuft weiter
        assert!(g.phase == Phase::Auction);
        g.apply(20, &json!({"type": "pass"})).unwrap();
        assert!(g.phase == Phase::End);
    }

    #[test]
    fn trade_negotiation_ping_pong() {
        let mut g = game();
        g.own[1].owner = Some(0);
        g.own[3].owner = Some(1);
        g.pls[0].money = 500;
        g.pls[1].money = 500;
        // Angebot: 0 gibt Mühlenweg + 100 €, will Gerbergasse
        g.apply(10, &json!({"type": "trade_propose", "to": 20, "give": {"money": 100, "props": [1]}, "take": {"props": [3]}})).unwrap();
        // Nicht der Anbieter darf annehmen, ein Dritter auch nicht
        let id = g.view(10)["trades"][0]["id"].clone();
        assert!(g.apply(10, &json!({"type": "trade_accept", "id": id})).is_err());
        assert!(g.apply(30, &json!({"type": "trade_accept", "id": id})).is_err());
        assert_eq!(g.view(30)["trades"].as_array().unwrap().len(), 0);
        // Spieler 1 verhandelt: will nur 50 € statt 100 € und tauscht trotzdem
        g.apply(20, &json!({"type": "trade_counter", "id": id, "give": {"props": [3]}, "take": {"money": 50, "props": [1]}})).unwrap();
        let t = g.view(10)["trades"][0].clone();
        assert_eq!(t["from"], 20);
        assert_eq!(t["to"], 10);
        assert_eq!(t["rounds"], 1);
        // Spieler 0 ändert erneut
        g.apply(10, &json!({"type": "trade_counter", "id": id, "give": {"money": 50, "props": [1]}, "take": {"props": [3]}})).unwrap();
        g.apply(20, &json!({"type": "trade_accept", "id": id})).unwrap();
        assert_eq!(g.own[1].owner, Some(1));
        assert_eq!(g.own[3].owner, Some(0));
        assert_eq!(g.pls[0].money, 450);
        assert_eq!(g.pls[1].money, 550);
        assert!(g.trades.is_empty());
    }

    #[test]
    fn trade_decline_cancel_and_mortgage_fee() {
        let mut g = game();
        g.own[39].owner = Some(1);
        g.own[39].mort = true;
        g.pls[0].money = 500;
        g.pls[1].money = 100;
        g.apply(10, &json!({"type": "trade_propose", "to": 20, "give": {"money": 100}, "take": {"props": [39]}})).unwrap();
        assert!(g.apply(10, &json!({"type": "trade_propose", "to": 20, "give": {"money": 5}})).is_err(), "nur ein Angebot pro Paar");
        let id = g.view(10)["trades"][0]["id"].clone();
        g.apply(20, &json!({"type": "trade_accept", "id": id})).unwrap();
        // 10 % Gebühr auf den Hypothekenwert (200) = 20 € für den Übernehmer (Spieler 0)
        assert_eq!(g.pls[0].money, 500 - 100 - 20);
        assert_eq!(g.pls[1].money, 200);
        assert_eq!(g.own[39].owner, Some(0));
        // Ablehnen und Zurückziehen
        g.apply(20, &json!({"type": "trade_propose", "to": 30, "give": {"money": 10}})).unwrap();
        let id = g.view(20)["trades"][0]["id"].clone();
        assert!(g.apply(20, &json!({"type": "trade_decline", "id": id})).is_err());
        g.apply(30, &json!({"type": "trade_decline", "id": id})).unwrap();
        g.apply(20, &json!({"type": "trade_propose", "to": 30, "give": {"money": 10}})).unwrap();
        let id = g.view(20)["trades"][0]["id"].clone();
        g.apply(20, &json!({"type": "trade_cancel", "id": id})).unwrap();
        assert!(g.trades.is_empty());
        // Handeln ist auch außerhalb des eigenen Zuges möglich
        assert!(g.cur == 0);
        g.apply(30, &json!({"type": "trade_propose", "to": 20, "give": {"money": 1}})).unwrap();
    }
}
