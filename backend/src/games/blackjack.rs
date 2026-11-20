//! Blackjack: alle Spieler gegen den Dealer (das Haus).
//!
//! Regeln: 6 Decks, Dealer bleibt bei 17 (auch Soft 17), Blackjack zahlt 3:2, Verdoppeln auf die
//! ersten zwei Karten, einmal Teilen (gesplittete Asse bekommen je nur eine Karte), keine Versicherung.
//! Alle setzen gleichzeitig, danach geht es reihum. Timer verhindern, dass ein Tisch hängen bleibt.

use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use serde::Serialize;
use serde_json::{json, Value};

use super::{Game, GameInfo, GameKind, PlayerId};

const START_CHIPS: u64 = 1000;
const MIN_BET: u64 = 10;
const DECKS: usize = 6;
const BET_SECS: u64 = 25;
const TURN_SECS: u64 = 30;
const RESULT_SECS: u64 = 8;

/// r: 1 = Ass, 2–10, 11 = Bube, 12 = Dame, 13 = König; s: Farbe 0–3
#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
struct Card {
    r: u8,
    s: u8,
}

fn card_value(c: &Card) -> u32 {
    match c.r {
        1 => 11,
        11..=13 => 10,
        n => n as u32,
    }
}

/// (Summe, ist „soft“ – ein Ass zählt noch als 11)
fn hand_value(cards: &[Card]) -> (u32, bool) {
    let mut total = 0;
    let mut aces = 0;
    for c in cards {
        total += card_value(c);
        if c.r == 1 {
            aces += 1;
        }
    }
    while total > 21 && aces > 0 {
        total -= 10;
        aces -= 1;
    }
    (total, aces > 0)
}

fn new_shoe() -> Vec<Card> {
    let mut v = Vec::with_capacity(52 * DECKS);
    for _ in 0..DECKS {
        for s in 0..4u8 {
            for r in 1..=13u8 {
                v.push(Card { r, s });
            }
        }
    }
    v.shuffle(&mut rand::thread_rng());
    v
}

#[derive(Clone, Debug)]
struct Hand {
    cards: Vec<Card>,
    bet: u64,
    done: bool,
    from_split: bool,
    result: Option<&'static str>,
}

impl Hand {
    fn new(bet: u64) -> Hand {
        Hand { cards: Vec::new(), bet, done: false, from_split: false, result: None }
    }
    fn value(&self) -> (u32, bool) {
        hand_value(&self.cards)
    }
    fn natural(&self) -> bool {
        !self.from_split && self.cards.len() == 2 && self.value().0 == 21
    }
    fn bust(&self) -> bool {
        self.value().0 > 21
    }
}

struct Seat {
    id: PlayerId,
    chips: u64,
    has_bet: bool,
    active: bool,
    ready: bool,
    hands: Vec<Hand>,
    cur_hand: usize,
    /// Gewinn/Verlust der letzten Runde
    delta: i64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Betting,
    Playing,
    Dealer,
    Results,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Phase::Betting => "betting",
            Phase::Playing => "playing",
            Phase::Dealer => "dealer",
            Phase::Results => "results",
        }
    }
}

pub struct BlackjackKind;

impl GameKind for BlackjackKind {
    fn info(&self) -> GameInfo {
        GameInfo {
            id: "blackjack",
            name: "Blackjack",
            description: "Schlag den Dealer: Komm mit deinen Karten näher an 21 heran, ohne sie zu überschreiten.",
            min_players: 1,
            max_players: 7,
            js: "/shared/games/blackjack.js",
            css: "/shared/games/blackjack.css",
        }
    }

    fn start(&self, players: &[PlayerId], options: &Value) -> Result<Box<dyn Game>, String> {
        let info = self.info();
        if players.len() < info.min_players {
            return Err(format!("Mindestens {} Spieler nötig", info.min_players));
        }
        if players.len() > info.max_players {
            return Err(format!("Höchstens {} Spieler erlaubt", info.max_players));
        }
        let rounds = options.get("rounds").and_then(Value::as_u64).unwrap_or(10).clamp(1, 50) as u32;
        let seats = players
            .iter()
            .map(|&id| Seat {
                id,
                chips: START_CHIPS,
                has_bet: false,
                active: false,
                ready: false,
                hands: Vec::new(),
                cur_hand: 0,
                delta: 0,
            })
            .collect();
        Ok(Box::new(Blackjack {
            seats,
            dealer: Vec::new(),
            shoe: new_shoe(),
            phase: Phase::Betting,
            cur: 0,
            round: 1,
            rounds,
            deadline: Instant::now() + Duration::from_secs(BET_SECS),
            dealer_next: Instant::now(),
            over: false,
        }))
    }
}

pub struct Blackjack {
    seats: Vec<Seat>,
    dealer: Vec<Card>,
    shoe: Vec<Card>,
    phase: Phase,
    /// Index des Spielers, der gerade zieht (nur in Phase Playing)
    cur: usize,
    round: u32,
    rounds: u32,
    deadline: Instant,
    dealer_next: Instant,
    over: bool,
}

impl Blackjack {
    fn draw(&mut self) -> Card {
        if self.shoe.is_empty() {
            self.shoe = new_shoe();
        }
        self.shoe.pop().unwrap_or(Card { r: 1, s: 0 })
    }

    /// Haben alle, die setzen können, schon gesetzt?
    fn all_bet(&self) -> bool {
        self.seats.iter().filter(|s| s.has_bet || s.chips >= MIN_BET).all(|s| s.has_bet)
    }

    fn deal(&mut self) {
        self.dealer.clear();
        let idxs: Vec<usize> = (0..self.seats.len()).filter(|&i| self.seats[i].has_bet).collect();
        for &i in &idxs {
            let s = &mut self.seats[i];
            let bet = s.hands.first().map_or(0, |h| h.bet);
            s.hands = vec![Hand::new(bet)];
            s.cur_hand = 0;
        }
        for _ in 0..2 {
            for &i in &idxs {
                let c = self.draw();
                self.seats[i].hands[0].cards.push(c);
            }
            let c = self.draw();
            self.dealer.push(c);
        }
        // Dealer schaut bei Ass/Zehn sofort nach Blackjack
        let up = card_value(&self.dealer[0]);
        if (up == 10 || up == 11) && hand_value(&self.dealer).0 == 21 {
            self.resolve();
            return;
        }
        for s in self.seats.iter_mut() {
            for h in s.hands.iter_mut() {
                if h.natural() {
                    h.done = true;
                }
            }
        }
        self.phase = Phase::Playing;
        self.cur = 0;
        self.advance();
    }

    /// Nächste offene Hand suchen; sind alle fertig, ist der Dealer dran.
    fn advance(&mut self) {
        while self.cur < self.seats.len() {
            let s = &mut self.seats[self.cur];
            if s.active {
                if let Some(h) = s.hands.iter().position(|h| !h.done) {
                    s.cur_hand = h;
                    self.deadline = Instant::now() + Duration::from_secs(TURN_SECS);
                    return;
                }
            }
            self.cur += 1;
        }
        self.begin_dealer();
    }

    fn begin_dealer(&mut self) {
        let live = self
            .seats
            .iter()
            .any(|s| s.active && s.hands.iter().any(|h| !h.bust() && !h.natural()));
        if live {
            self.phase = Phase::Dealer;
            self.dealer_next = Instant::now() + Duration::from_millis(900);
        } else {
            self.resolve();
        }
    }

    fn dealer_step(&mut self) {
        if hand_value(&self.dealer).0 < 17 {
            let c = self.draw();
            self.dealer.push(c);
            self.dealer_next = Instant::now() + Duration::from_millis(900);
        } else {
            self.resolve();
        }
    }

    fn resolve(&mut self) {
        let dv = hand_value(&self.dealer).0;
        let dealer_bj = self.dealer.len() == 2 && dv == 21;
        for s in self.seats.iter_mut() {
            for h in s.hands.iter_mut() {
                let pv = h.value().0;
                let (res, payout): (&'static str, u64) = if pv > 21 {
                    ("bust", 0)
                } else if h.natural() {
                    if dealer_bj {
                        ("push", h.bet)
                    } else {
                        ("blackjack", h.bet + h.bet * 3 / 2)
                    }
                } else if dealer_bj {
                    ("lose", 0)
                } else if dv > 21 || pv > dv {
                    ("win", h.bet * 2)
                } else if pv == dv {
                    ("push", h.bet)
                } else {
                    ("lose", 0)
                };
                s.chips += payout;
                s.delta += payout as i64 - h.bet as i64;
                h.result = Some(res);
                h.done = true;
            }
        }
        self.phase = Phase::Results;
        self.deadline = Instant::now() + Duration::from_secs(RESULT_SECS);
    }

    fn next_round(&mut self) {
        if self.round >= self.rounds {
            self.over = true;
            return;
        }
        // Schuh unter 25 % → neu mischen
        if self.shoe.len() < DECKS * 52 / 4 {
            self.shoe = new_shoe();
        }
        self.round += 1;
        for s in self.seats.iter_mut() {
            s.hands.clear();
            s.has_bet = false;
            s.active = false;
            s.ready = false;
            s.delta = 0;
            s.cur_hand = 0;
        }
        self.dealer.clear();
        if self.seats.iter().all(|s| s.chips < MIN_BET) {
            self.over = true;
            return;
        }
        self.phase = Phase::Betting;
        self.deadline = Instant::now() + Duration::from_secs(BET_SECS);
    }

    fn stand_current(&mut self) {
        if self.cur < self.seats.len() {
            let hi = self.seats[self.cur].cur_hand;
            if let Some(h) = self.seats[self.cur].hands.get_mut(hi) {
                h.done = true;
            }
        }
        self.advance();
    }
}

impl Game for Blackjack {
    fn apply(&mut self, p: PlayerId, action: &Value) -> Result<(), String> {
        if self.over {
            return Err("Die Partie ist vorbei".into());
        }
        let i = self.seats.iter().position(|s| s.id == p).ok_or("Du bist nicht in diesem Spiel")?;
        let kind = action.get("type").and_then(Value::as_str).unwrap_or("");
        match kind {
            "bet" => {
                if self.phase != Phase::Betting {
                    return Err("Es wird gerade nicht gesetzt".into());
                }
                let amount = action.get("amount").and_then(Value::as_u64).ok_or("Einsatz fehlt")?;
                let s = &mut self.seats[i];
                if s.has_bet {
                    return Err("Du hast schon gesetzt".into());
                }
                if s.chips < MIN_BET {
                    return Err("Du hast nicht genug Chips".into());
                }
                if amount < MIN_BET || amount > s.chips {
                    return Err(format!("Der Einsatz muss zwischen {} und {} liegen", MIN_BET, s.chips));
                }
                s.chips -= amount;
                s.has_bet = true;
                s.active = true;
                s.hands = vec![Hand::new(amount)];
                if self.all_bet() {
                    self.deal();
                }
                Ok(())
            }
            "unbet" => {
                if self.phase != Phase::Betting {
                    return Err("Es wird gerade nicht gesetzt".into());
                }
                let s = &mut self.seats[i];
                if !s.has_bet {
                    return Err("Du hast noch nicht gesetzt".into());
                }
                s.chips += s.hands.first().map_or(0, |h| h.bet);
                s.hands.clear();
                s.has_bet = false;
                s.active = false;
                Ok(())
            }
            "next" => {
                if self.phase != Phase::Results {
                    return Err("Die Runde läuft noch".into());
                }
                self.seats[i].ready = true;
                if self.seats.iter().filter(|s| s.active).all(|s| s.ready) {
                    self.next_round();
                }
                Ok(())
            }
            "hit" | "stand" | "double" | "split" => {
                if self.phase != Phase::Playing || i != self.cur {
                    return Err("Du bist nicht dran".into());
                }
                let hi = self.seats[i].cur_hand;
                match kind {
                    "hit" => {
                        let c = self.draw();
                        let h = &mut self.seats[i].hands[hi];
                        h.cards.push(c);
                        if h.value().0 >= 21 {
                            h.done = true;
                        }
                    }
                    "stand" => {
                        self.seats[i].hands[hi].done = true;
                    }
                    "double" => {
                        let (n, bet) = {
                            let h = &self.seats[i].hands[hi];
                            (h.cards.len(), h.bet)
                        };
                        if n != 2 {
                            return Err("Verdoppeln geht nur mit den ersten zwei Karten".into());
                        }
                        if self.seats[i].chips < bet {
                            return Err("Zu wenig Chips zum Verdoppeln".into());
                        }
                        let c = self.draw();
                        let s = &mut self.seats[i];
                        s.chips -= bet;
                        let h = &mut s.hands[hi];
                        h.bet *= 2;
                        h.cards.push(c);
                        h.done = true;
                    }
                    _ => {
                        let (bet, first, second, ok) = {
                            let s = &self.seats[i];
                            let h = &s.hands[hi];
                            let ok = s.hands.len() == 1
                                && h.cards.len() == 2
                                && card_value(&h.cards[0]) == card_value(&h.cards[1]);
                            (h.bet, h.cards[0], h.cards[1], ok)
                        };
                        if !ok {
                            return Err("Teilen geht hier nicht".into());
                        }
                        if self.seats[i].chips < bet {
                            return Err("Zu wenig Chips zum Teilen".into());
                        }
                        let c1 = self.draw();
                        let c2 = self.draw();
                        let s = &mut self.seats[i];
                        s.chips -= bet;
                        s.hands[0].cards = vec![first, c1];
                        s.hands[0].from_split = true;
                        let mut h2 = Hand::new(bet);
                        h2.from_split = true;
                        h2.cards = vec![second, c2];
                        s.hands.push(h2);
                        let aces = first.r == 1;
                        for h in s.hands.iter_mut() {
                            if aces || h.value().0 >= 21 {
                                h.done = true;
                            }
                        }
                    }
                }
                self.advance();
                Ok(())
            }
            _ => Err("Unbekannte Aktion".into()),
        }
    }

    fn view(&self, p: PlayerId) -> Value {
        let reveal = matches!(self.phase, Phase::Dealer | Phase::Results);
        let dealer_cards: Vec<Value> = if self.dealer.is_empty() {
            Vec::new()
        } else if reveal {
            self.dealer.iter().map(|c| json!(c)).collect()
        } else {
            vec![json!(self.dealer[0]), Value::Null]
        };
        let dealer_value = if reveal && !self.dealer.is_empty() { Some(hand_value(&self.dealer).0) } else { None };
        let current = if self.phase == Phase::Playing { self.seats.get(self.cur).map(|s| s.id) } else { None };

        let seats: Vec<Value> = self
            .seats
            .iter()
            .map(|s| {
                let hands: Vec<Value> = s
                    .hands
                    .iter()
                    .map(|h| {
                        let (v, soft) = h.value();
                        let state = if h.result.is_some() {
                            "result"
                        } else if v > 21 {
                            "bust"
                        } else if h.natural() {
                            "blackjack"
                        } else if h.done {
                            "stand"
                        } else {
                            "playing"
                        };
                        json!({
                            "cards": h.cards,
                            "bet": h.bet,
                            "value": v,
                            "soft": soft && v < 21,
                            "state": state,
                            "result": h.result,
                        })
                    })
                    .collect();
                json!({
                    "id": s.id,
                    "chips": s.chips,
                    "has_bet": s.has_bet,
                    "active": s.active,
                    "ready": s.ready,
                    "delta": s.delta,
                    "cur_hand": s.cur_hand,
                    "can_bet": s.has_bet || s.chips >= MIN_BET,
                    "hands": hands,
                })
            })
            .collect();

        let mut can = json!({"hit": false, "stand": false, "double": false, "split": false});
        if self.phase == Phase::Playing && current == Some(p) {
            if let Some(s) = self.seats.get(self.cur) {
                if let Some(h) = s.hands.get(s.cur_hand) {
                    can = json!({
                        "hit": true,
                        "stand": true,
                        "double": h.cards.len() == 2 && s.chips >= h.bet,
                        "split": s.hands.len() == 1
                            && h.cards.len() == 2
                            && card_value(&h.cards[0]) == card_value(&h.cards[1])
                            && s.chips >= h.bet,
                    });
                }
            }
        }
        let left_ms = match self.phase {
            Phase::Dealer => 0,
            _ => self.deadline.saturating_duration_since(Instant::now()).as_millis() as u64,
        };
        json!({
            "phase": self.phase.name(),
            "round": self.round,
            "rounds": self.rounds,
            "over": self.over,
            "left_ms": left_ms,
            "current": current,
            "dealer": {"cards": dealer_cards, "value": dealer_value},
            "seats": seats,
            "shoe": self.shoe.len(),
            "min_bet": MIN_BET,
            "can": can,
        })
    }

    fn finished(&self) -> bool {
        self.over
    }

    fn tick(&mut self) -> bool {
        if self.over {
            return false;
        }
        let now = Instant::now();
        match self.phase {
            Phase::Betting => {
                if now < self.deadline {
                    return false;
                }
                if self.seats.iter().any(|s| s.has_bet) {
                    self.deal();
                } else {
                    // niemand hat gesetzt: Timer neu starten, der Tisch wartet
                    self.deadline = now + Duration::from_secs(BET_SECS);
                }
                true
            }
            Phase::Playing => {
                if now < self.deadline {
                    return false;
                }
                self.stand_current();
                true
            }
            Phase::Dealer => {
                if now < self.dealer_next {
                    return false;
                }
                self.dealer_step();
                true
            }
            Phase::Results => {
                if now < self.deadline {
                    return false;
                }
                self.next_round();
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(r: u8) -> Card {
        Card { r, s: 0 }
    }

    fn start(n: usize, rounds: u64) -> Blackjack {
        let ids: Vec<PlayerId> = (1..=n as u32).collect();
        let seats = ids
            .iter()
            .map(|&id| Seat { id, chips: START_CHIPS, has_bet: false, active: false, ready: false, hands: Vec::new(), cur_hand: 0, delta: 0 })
            .collect();
        Blackjack {
            seats,
            dealer: Vec::new(),
            shoe: new_shoe(),
            phase: Phase::Betting,
            cur: 0,
            round: 1,
            rounds: rounds as u32,
            deadline: Instant::now() + Duration::from_secs(BET_SECS),
            dealer_next: Instant::now(),
            over: false,
        }
    }

    #[test]
    fn kind_rejects_too_many_players() {
        let ids: Vec<PlayerId> = (1..=8).collect();
        assert!(BlackjackKind.start(&ids, &json!({})).is_err());
        assert!(BlackjackKind.start(&[1], &json!({})).is_ok());
    }

    #[test]
    fn hand_values() {
        assert_eq!(hand_value(&[c(1), c(13)]), (21, true));
        assert_eq!(hand_value(&[c(1), c(1), c(9)]), (21, true));
        assert_eq!(hand_value(&[c(1), c(6), c(10)]), (17, false));
        assert_eq!(hand_value(&[c(10), c(12), c(5)]), (25, false));
    }

    #[test]
    fn blackjack_pays_three_to_two() {
        let mut g = start(1, 3);
        // Ziehreihenfolge: Spieler, Dealer, Spieler, Dealer – pop() nimmt vom Ende
        g.shoe = vec![c(7), c(13), c(9), c(1)];
        g.apply(1, &json!({"type": "bet", "amount": 100})).unwrap();
        assert_eq!(g.phase, Phase::Results);
        assert_eq!(g.seats[0].chips, 1000 - 100 + 250);
    }

    #[test]
    fn dealer_stands_on_soft_17_and_player_wins_with_20() {
        let mut g = start(1, 3);
        // Spieler 10+K = 20, Dealer A+6 = soft 17 → bleibt stehen
        g.shoe = vec![c(6), c(13), c(1), c(10)];
        g.apply(1, &json!({"type": "bet", "amount": 50})).unwrap();
        assert_eq!(g.phase, Phase::Playing);
        g.apply(1, &json!({"type": "stand"})).unwrap();
        assert_eq!(g.phase, Phase::Dealer);
        g.dealer_next = Instant::now();
        assert!(g.tick());
        assert_eq!(g.phase, Phase::Results);
        assert_eq!(g.dealer.len(), 2);
        assert_eq!(g.seats[0].chips, 1000 + 50);
    }

    #[test]
    fn double_and_bust() {
        let mut g = start(1, 3);
        // Spieler 10+6, Dealer 10+7; Spieler verdoppelt und zieht 10 → 26 = Bust
        // Reihenfolge der pop(): 10 (Sp), 10 (De), 6 (Sp), 7 (De), dann 10 (Double)
        g.shoe = vec![c(10), c(7), c(6), c(10), c(10)];
        g.apply(1, &json!({"type": "bet", "amount": 100})).unwrap();
        g.apply(1, &json!({"type": "double"})).unwrap();
        assert_eq!(g.phase, Phase::Results);
        assert_eq!(g.seats[0].chips, 1000 - 200);
    }

    #[test]
    fn timeouts_and_bot_game_finish() {
        let mut g = start(3, 4);
        let mut guard = 0;
        while !g.over && guard < 2000 {
            guard += 1;
            match g.phase {
                Phase::Betting => {
                    // nur Spieler 1 setzt, die anderen sitzen aus
                    let _ = g.apply(1, &json!({"type": "bet", "amount": 10}));
                    g.deadline = Instant::now();
                }
                Phase::Playing => {
                    if let Some(cur) = g.seats.get(g.cur).map(|s| s.id) {
                        let _ = g.apply(cur, &json!({"type": if guard % 3 == 0 { "hit" } else { "stand" }}));
                    }
                    g.deadline = Instant::now();
                }
                Phase::Dealer => g.dealer_next = Instant::now(),
                Phase::Results => g.deadline = Instant::now(),
            }
            g.tick();
            let _ = g.view(1);
            let _ = g.view(99); // Zuschauer
        }
        assert!(g.over, "Partie endet nach den Runden");
        assert!(g.seats[0].chips > 0);
    }
}
