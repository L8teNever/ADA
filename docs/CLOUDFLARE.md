# Cloudflare einrichten

Beide Adressen zeigen auf **denselben Container** (Port 8080). Das Backend erkennt am
`Host`-Header, welche Seite gemeint ist.

## 1. Tunnel – Public Hostnames

| Hostname | Service |
|---|---|
| `ada.kulbarts.com` | `http://localhost:8080` (bzw. `http://ada:8080` im selben Docker-Netz) |
| `gameada.kulbarts.com` | derselbe Service |

## 2. Zero Trust Access

- **Nur** für `ada.kulbarts.com` eine Self-hosted Application mit Policy (erlaubte Mails/Gruppen) anlegen.
- Für `gameada.kulbarts.com` **keine** Access-App – diese Seite ist öffentlich.
  Zugang zu einem Spiel hat nur, wer den geheimen Link `https://gameada.kulbarts.com/<token>` kennt.

## 3. Wie der Login funktioniert

Nach dem Zero-Trust-Login setzt Cloudflare den Header `Cf-Access-Authenticated-User-Email`.
Das Backend legt beim ersten Besuch automatisch einen Account zur Mail an
(`/data/users.json`, Name aus dem Mail-Anfang, im Profil änderbar).

Sicherheit:
- Der Header wird **nur** auf dem Portal-Host ausgewertet, nie auf `gameada…`.
- Der Port 8080 ist in `docker-compose.yml` nur an `127.0.0.1` gebunden. Der Container darf nicht
  an anderer Stelle direkt erreichbar sein, sonst ließe sich der Header fälschen.
- WebSockets prüfen den `Origin` (Schutz vor fremden Webseiten).
