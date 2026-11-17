# ADA

Spieleplattform mit Rust-Backend (axum) und zwei Webseiten in **einem** Docker-Container,
Design im Material-3-Stil (adaptiv: Handy, Tablet, Desktop).

| Seite | Adresse | Zugang |
|---|---|---|
| **Portal** | `ada.kulbarts.com` | Cloudflare Zero Trust. Account entsteht automatisch aus der Mail im Header. Hier Räume erstellen und spielen. |
| **Spiele** | `game.kulbarts.com/<token>` (Alias möglich: `PLAY_HOST=a.example,b.example`) | Öffentlich. Wer den Link kennt, gibt einen Namen ein und spielt mit. |

Ablauf: Im Portal ein Spiel wählen → Raum erstellen → Link `https://game.kulbarts.com/<token>`
verteilen. Zero-Trust-Nutzer spielen direkt im Portal mit ihrem Account, Gäste über den Link mit Namen.

## Starten

```bash
docker compose up -d --build     # läuft auf 127.0.0.1:8080
```

Cloudflare-Tunnel und Access: siehe [docs/CLOUDFLARE.md](docs/CLOUDFLARE.md).

Lokal testen ohne Zero Trust (Rust nötig):

```bash
cd backend && cargo build
DEV_EMAIL=ich@example.com PLAY_HOST=play.localhost PORT=8080 DATA_DIR=/tmp/ada WEB_DIR=../web target/debug/ada
# Portal: http://localhost:8080   Spiele: http://play.localhost:8080/<token>
```

## Aufbau

```
backend/src
  main.rs        Router, Host-Weiche (Portal/Spiele), Healthcheck
  auth.rs        Account aus Zero-Trust-Header
  users.rs       Accounts (users.json)
  rooms.rs       Räume, Lobby, Spieler, WebSocket-Verteilung
  ws.rs          WebSocket (Portal = Account, Spiele = Gast)
  api.rs         REST-API
  games/         Spiele-Plugins (uno.rs)
web/shared       m3.css, common.js, room.js + games/<id>.js|css (von beiden Seiten genutzt)
web/portal       Portal-App
web/play         Spieleseite
```

Daten (`/data/users.json`) liegen im Volume. Räume und laufende Spiele sind nur im Speicher;
leere Räume werden nach 2 Stunden entfernt.

## Neues Spiel hinzufügen

1. `backend/src/games/meinspiel.rs`: `GameKind` (Infos, `start`) und `Game` (`apply`, `view`, `finished`) implementieren.
2. In `games/mod.rs` `pub mod meinspiel;` und in `registry()` eintragen.
3. `web/shared/games/meinspiel.js` (`Ada.games.meinspiel = { render(el, view, ctx) }`) und optional `.css`.

Lobby, Einladungslink, Gäste, Reconnect und Host-Steuerung kommen automatisch.
`view` darf nur enthalten, was der jeweilige Spieler sehen darf.

## UNO-Regeln (vereinfacht)

2–10 Spieler, 7 Karten. Farbe oder Zahl/Symbol passend ablegen, Joker immer. Aussetzen, Richtungswechsel
(zu zweit = Aussetzen), +2, +4 (Gegner zieht und setzt aus). Gezogene Karte darf sofort gespielt werden,
sonst passen. Wer zuerst keine Karten hat, gewinnt. Kein Stapeln von +2/+4.

## Skyjo-Regeln

2–8 Spieler, je 12 verdeckte Karten im 4×3-Raster, zu Beginn decken alle 2 auf (wer die höchste Summe hat, beginnt).
Im Zug: Karte vom Stapel ziehen und eine Rasterkarte ersetzen **oder** abwerfen und eine verdeckte aufdecken –
oder die oberste Ablagekarte nehmen und eine Rasterkarte ersetzen. Drei gleiche Karten in einer Spalte fliegen raus.
Wer alles aufgedeckt hat, löst die letzte Runde aus; hat er dann nicht die niedrigste Punktzahl, zählt seine doppelt.
Das Spiel endet, sobald jemand 100 Punkte erreicht – wenige Punkte gewinnen.

## Darstellung

Alle Spielflächen skalieren mit Breite **und** Höhe (`dvh`), Karten brechen um statt aus dem Bild zu laufen,
im Querformat auf dem Handy gibt es ein eigenes zweispaltiges Layout.

## Schach

2 Spieler, Farben werden zufällig verteilt. Vollständige Regeln (Rochade, en passant, Umwandlung, Schach/Matt/Patt,
50-Züge-Regel, dreifache Wiederholung, zu wenig Material), Aufgeben und Remis anbieten/annehmen.
Die Zuggenerierung ist mit den Standard-Perft-Tests abgesichert (`cargo test`).

## Monopoly

2–6 Spieler, 1500 € Startgeld, eigenes Brett (40 Felder) mit allen Regeln: Grundstücke, Bahnhöfe und Werke kaufen oder
versteigern, Miete (doppelt bei ganzer Farbgruppe), Häuser und Hotels (gleichmäßig bauen, begrenzter Vorrat),
Hypotheken (10 % Aufschlag beim Auslösen), Handel (Geld, Grundstücke, Freikarten), Ereignis- und Gemeinschaftskarten,
Gefängnis (Pasch, 50 € oder Freikarte, nach 3 Versuchen Zahlung), Steuern, Schulden, Bankrott und Aufgeben.
Wer nicht mehr überbieten kann, scheidet automatisch aus der Auktion aus. Der letzte Spieler gewinnt.
Hausregeln: „Frei Parken" zahlt nichts aus, Miete in der Auktion/Schuldenphase wird sofort verrechnet.

## Skribbl (Zeichnen & Raten)

2–12 Spieler. Pro Zug wählt der Zeichner eines von 3 Wörtern (15 s), zeichnet 80 s lang (Pinsel, Radierer, Füllen, Rückgängig,
16 Farben, 4 Dicken) und alle anderen tippen im Chat. Je schneller richtig geraten, desto mehr Punkte; der Zeichner bekommt den
Durchschnitt. Buchstaben-Hinweise erscheinen nach 35 %, 60 % und 80 % der Zeit. „Fast richtig" wird nur dem Rater gezeigt, wer das
Wort kennt, chattet nur mit anderen Wissenden. 3 Runden (jeder zeichnet pro Runde einmal). Optionen `rounds` und `time` beim
Erstellen des Raums (API: `POST /api/rooms` mit `options`).

Technik: Das Spiel-Framework hat dafür `tick()` (Zeit), `drain_events()` (leichte Nachrichten ohne vollen Zustand) und
`needs_state()`; Striche laufen als Events, der volle Zustand enthält alle Striche für späte Mitspieler.
