# ADA

Spieleplattform mit Rust-Backend (axum) und zwei Webseiten in **einem** Docker-Container,
Design im Material-3-Stil (adaptiv: Handy, Tablet, Desktop).

| Seite | Adresse | Zugang |
|---|---|---|
| **Portal** | `ada.kulbarts.com` | Cloudflare Zero Trust. Account entsteht automatisch aus der Mail im Header. Hier Räume erstellen und spielen. |
| **Spiele** | `gameada.kulbarts.com/<token>` | Öffentlich. Wer den Link kennt, gibt einen Namen ein und spielt mit. |

Ablauf: Im Portal ein Spiel wählen → Raum erstellen → Link `https://gameada.kulbarts.com/<token>`
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
