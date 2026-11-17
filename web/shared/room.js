/* Raum-Oberfläche (Lobby + Spiel) – wird von Portal UND Spieleseite benutzt.
 *
 * Ada.mountRoom(root, {
 *   token, wsPath,          // WebSocket-Pfad, z. B. "/ws/abc123"
 *   name, key,              // nur Gäste: Anzeigename + geheimer Schlüssel
 *   shareLink,              // Einladungslink (optional)
 *   onExit()                // "Verlassen" gedrückt oder Raum geschlossen
 * })
 */
(function () {
  const { esc, icon, initial, toast, copy } = Ada;
  const loaded = {};

  Ada.loadGame = (info) => {
    if (loaded[info.id]) return loaded[info.id];
    loaded[info.id] = new Promise((resolve, reject) => {
      if (info.css) {
        const l = document.createElement('link');
        l.rel = 'stylesheet'; l.href = info.css; document.head.append(l);
      }
      const s = document.createElement('script');
      s.src = info.js; s.onload = () => resolve(Ada.games[info.id]); s.onerror = () => reject(new Error('Spiel konnte nicht geladen werden'));
      document.head.append(s);
    });
    return loaded[info.id];
  };

  Ada.mountRoom = (root, o) => {
    let ws, state = null, closed = false, retry = 0, ping, reconnectTimer, gameMod = null, gameKey = null;
    let status = 'connecting';
    let fatal = null;

    const send = (m) => { if (ws && ws.readyState === 1) ws.send(JSON.stringify(m)); };

    function connect() {
      status = 'connecting';
      const proto = location.protocol === 'https:' ? 'wss' : 'ws';
      ws = new WebSocket(`${proto}://${location.host}${o.wsPath}`);
      ws.onopen = () => {
        retry = 0; status = 'online';
        send({ type: 'join', name: o.name, key: o.key });
        clearInterval(ping);
        ping = setInterval(() => send({ type: 'ping' }), 25000);
      };
      ws.onmessage = (ev) => {
        let m; try { m = JSON.parse(ev.data); } catch (_) { return; }
        if (m.type === 'state') { state = m; render(); }
        else if (m.type === 'event') { if (gameMod && gameMod.event) gameMod.event(m.data); }
        else if (m.type === 'error') {
          if (m.fatal) { fatal = m.message; closed = true; render(); }
          else toast(m.message);
        }
        else if (m.type === 'closed') { closed = true; toast('Der Raum wurde geschlossen'); exit(); }
        else if (m.type === 'kicked') { closed = true; toast('Du wurdest aus dem Raum entfernt'); exit(); }
        else if (m.type === 'left') { closed = true; exit(); }
        else if (m.type === 'replaced') { closed = true; fatal = 'Du bist in einem anderen Tab/Gerät verbunden.'; render(); }
      };
      ws.onclose = () => {
        clearInterval(ping);
        if (closed) return;
        status = 'connecting'; render();
        reconnectTimer = setTimeout(connect, Math.min(800 * 2 ** retry++, 8000));
      };
    }

    function exit() { destroy(); o.onExit && o.onExit(); }
    function destroy() {
      closed = true; clearInterval(ping); clearTimeout(reconnectTimer);
      try { ws && ws.close(); } catch (_) { /* egal */ }
      root.innerHTML = '';
    }

    function playersMap() {
      const map = {};
      (state.room.players || []).forEach((p) => (map[p.id] = p));
      return map;
    }

    function render() {
      if (fatal) {
        root.innerHTML = `<div class="center-page"><div class="card fade-in">
          <div class="title">Verbindung beendet</div><p class="muted">${esc(fatal)}</p>
          <button class="btn" data-act="exit">${icon('back')} Zurück</button></div></div>`;
        root.querySelector('[data-act=exit]').onclick = exit;
        return;
      }
      if (!state) {
        root.innerHTML = `<div class="center-page"><div class="muted title">Verbinde…</div></div>`;
        return;
      }
      const banner = status !== 'online' ? `<div class="banner" style="margin-bottom:12px">Verbindung unterbrochen – verbinde neu…</div>` : '';
      if (state.room.state === 'lobby') {
        gameKey = null;
        renderLobby(banner);
      } else {
        renderGame(banner);
      }
    }

    function renderLobby(banner) {
      const { room, you } = state;
      const isHost = room.host === you;
      const online = room.players.filter((p) => p.connected).length;
      const min = room.game.min_players;
      const rows = room.players.map((p, i) => `
        <div class="list-item ${p.connected ? '' : 'offline'}">
          <div class="avatar c${i % 3}">${esc(initial(p.name))}</div>
          <div class="grow"><div class="body">${esc(p.name)}${p.id === you ? ' <span class="muted">(du)</span>' : ''}</div>
            <div class="label muted">${p.account ? 'Account' : 'Gast'}${p.connected ? '' : ' · offline'}</div></div>
          ${p.id === room.host ? `<span class="chip accent">Host</span>` : ''}
          ${isHost && p.id !== you ? `<button class="icon-btn" data-kick="${p.id}" aria-label="Entfernen">${icon('close')}</button>` : ''}
        </div>`).join('');
      const share = o.shareLink ? `
        <div class="card filled"><div class="label muted">EINLADUNGSLINK</div>
          <div class="link-box"><code>${esc(o.shareLink)}</code>
            <button class="icon-btn" data-act="copy" aria-label="Link kopieren">${icon('copy')}</button></div>
          <p class="muted">Jeder mit diesem Link kann mit einem Namen mitspielen.</p></div>` : '';
      const canStart = isHost && online >= min;
      Ada.patch(root, `${banner}
        <div class="room-wrap lobby fade-in">
          <div class="stack">
            <div class="card hero">
              <div class="label">LOBBY</div>
              <div class="display">${esc(room.game.name)}</div>
              <p>${esc(room.game.description)}</p>
              <div class="row"><span class="chip">${online} online</span><span class="chip">${min}–${room.game.max_players} Spieler</span></div>
            </div>
            ${share}
          </div>
          <div class="stack">
            <div class="title">Spieler</div>
            <div class="list">${rows}</div>
            ${isHost
              ? `<button class="btn big" data-act="start" ${canStart ? '' : 'disabled'}>${icon('play')} Spiel starten</button>
                 ${canStart ? '' : `<p class="muted">Ab ${min} Spielern kann es losgehen.</p>`}`
              : `<p class="muted">Warte, bis der Host das Spiel startet…</p>`}
            <button class="btn text" data-act="leave">Raum verlassen</button>
          </div>
        </div>`);
      const q = (s) => root.querySelector(s);
      const copyBtn = q('[data-act=copy]'); if (copyBtn) copyBtn.onclick = () => copy(o.shareLink);
      const startBtn = q('[data-act=start]'); if (startBtn) startBtn.onclick = () => send({ type: 'start' });
      q('[data-act=leave]').onclick = () => { send({ type: 'leave' }); setTimeout(exit, 250); };
      root.querySelectorAll('[data-kick]').forEach((b) => (b.onclick = () => send({ type: 'kick', player: Number(b.dataset.kick) })));
    }

    function renderGame(banner) {
      const { room, you } = state;
      const isHost = room.host === you;
      if (gameKey !== room.game.id) {
        gameKey = room.game.id; gameMod = null;
        root.innerHTML = `<div class="center-page"><div class="muted title">Lade ${esc(room.game.name)}…</div></div>`;
        Ada.loadGame(room.game).then((m) => { gameMod = m; render(); }).catch((e) => toast(e.message));
        return;
      }
      if (!gameMod) return;
      // Der Spielbereich bleibt zwischen Zustandsupdates bestehen (wichtig z. B. für Zeichenflächen).
      let shell = root.querySelector('.game-shell');
      if (!shell || shell.dataset.game !== room.game.id) {
        root.innerHTML = `<div class="bn"></div><div class="game-shell fade-in" data-game="${esc(room.game.id)}"><div class="row gs-head"></div><div class="game-root"></div></div>`;
        shell = root.querySelector('.game-shell');
      }
      Ada.patch(root.querySelector('.bn'), banner);
      const head = shell.querySelector('.gs-head');
      Ada.patch(head, `<div class="title">${esc(room.game.name)}</div>
        ${isHost ? `<button class="btn text" data-act="end">${room.over ? 'Zurück zur Lobby' : 'Runde beenden'}</button>` : ''}`);
      head.style.justifyContent = 'space-between';
      const end = head.querySelector('[data-act=end]'); if (end) end.onclick = () => send({ type: 'lobby' });
      gameMod.render(shell.querySelector('.game-root'), state.game, {
        you, room, isHost, players: playersMap(),
        send: (action) => send({ type: 'action', action }),
        toLobby: () => send({ type: 'lobby' }),
      });
    }

    connect();
    render();
    return { destroy };
  };
})();
