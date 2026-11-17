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
        l.rel = 'stylesheet'; l.href = info.css + '?v=20261117c'; document.head.append(l);
      }
      const s = document.createElement('script');
      s.src = info.js + '?v=20261117c'; s.onload = () => resolve(Ada.games[info.id]); s.onerror = () => reject(new Error('Spiel konnte nicht geladen werden'));
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

    const OPTION_UI = {
      skribbl: [
        { key: 'rounds', label: 'Runden', values: [1, 2, 3, 5, 8], def: 3, suffix: '' },
        { key: 'time', label: 'Zeit pro Zug', values: [40, 60, 80, 120, 180], def: 80, suffix: ' s' },
      ],
    };

    function renderLobby(banner) {
      const { room, you } = state;
      const isHost = room.host === you;
      const online = room.players.filter((p) => p.connected).length;
      const g = room.game; // gewähltes Spiel oder null
      const rows = room.players.map((p, i) => `
        <div class="list-item ${p.connected ? '' : 'offline'}">
          <div class="avatar c${i % 3}">${esc(initial(p.name))}</div>
          <div class="grow"><div class="body">${esc(p.name)}${p.id === you ? ' <span class="muted">(du)</span>' : ''}</div>
            <div class="label muted">${p.account ? 'Account' : 'Gast'}${p.connected ? '' : ' · offline'}</div></div>
          ${p.id === room.host ? `<span class="chip accent">Host</span>` : ''}
          ${isHost && p.id !== you ? `<button class="icon-btn" data-kick="${p.id}" aria-label="Entfernen">${icon('close')}</button>` : ''}
        </div>`).join('');
      const share = o.shareLink ? `
        <div class="card filled invite">
          ${Ada.qrSvg(o.shareLink)}
          <div class="invite-info">
            <div class="label muted">EINLADEN</div>
            <div class="link-box"><code>${esc(o.shareLink)}</code>
              <button class="icon-btn" data-act="copy" aria-label="Link kopieren">${icon('copy')}</button></div>
            <span class="muted hint">QR scannen oder Link teilen – er gilt für alle Spiele.</span>
          </div>
        </div>` : '';

      const tiles = (room.games || []).map((x) => `
        <button class="gt ${g && g.id === x.id ? 'on' : ''}" data-pick="${esc(x.id)}" ${isHost ? '' : 'disabled'}>
          <span class="gi">${icon('cards')}</span><b>${esc(x.name)}</b><span class="gs">${x.min_players}–${x.max_players} Spieler</span></button>`).join('');
      const optDefs = g ? OPTION_UI[g.id] : null;
      const opts = optDefs ? optDefs.map((d) => {
        const cur = (room.options && room.options[d.key]) ?? d.def;
        return `<div class="opt"><span class="label muted">${esc(d.label)}</span><span class="seg">${d.values.map((val) =>
          `<button data-opt="${d.key}:${val}" class="${Number(cur) === val ? 'on' : ''}" ${isHost ? '' : 'disabled'}>${val}${d.suffix}</button>`).join('')}</span></div>`;
      }).join('') : '';

      let startHint = '';
      let canStart = false;
      if (!g) startHint = isHost ? 'Wähle oben ein Spiel.' : 'Der Host wählt gleich ein Spiel.';
      else if (online < g.min_players) startHint = `${g.name} braucht mindestens ${g.min_players} Spieler.`;
      else if (online > g.max_players) startHint = `${g.name} erlaubt höchstens ${g.max_players} Spieler – entferne jemanden oder wähle ein anderes Spiel.`;
      else canStart = isHost;

      Ada.patch(root, `${banner}
        <div class="room-wrap lobby fade-in">
          <div class="card hero lb-hero">
            <div class="row between"><div class="label">${g ? 'AUSGEWÄHLT' : 'SPIELE-LOBBY'}</div>
              <div class="row"><span class="chip">${online} online</span>${g ? `<span class="chip">${g.min_players}–${g.max_players} Spieler</span>` : ''}</div></div>
            <div class="display">${g ? esc(g.name) : 'Was spielen wir?'}</div>
            <p class="hero-desc">${g ? esc(g.description) : isHost ? 'Du bist der Host: Wähle ein Spiel – nach der Runde kannst du jederzeit ein anderes wählen.' : 'Der Host wählt gleich das Spiel.'}</p>
          </div>
          <div class="lb-invite">${share}</div>
          <div class="card lb-games">
            <div class="label muted">${isHost ? 'SPIEL WÄHLEN' : 'SPIELE'}</div>
            <div class="game-grid">${tiles}</div>
            ${opts ? `<div class="opts">${opts}</div>` : ''}
          </div>
          <div class="stack lb-players">
            <div class="title">Spieler <span class="muted">(${room.players.length})</span></div>
            <div class="list">${rows}</div>
          </div>
          <div class="lb-action">
            ${startHint ? `<p class="muted hint">${esc(startHint)}</p>` : ''}
            <div class="row nowrap">
              ${isHost ? `<button class="btn big grow" data-act="start" ${canStart ? '' : 'disabled'}>${icon('play')} ${g ? esc(g.name) + ' starten' : 'Spiel wählen'}</button>` : ''}
              <button class="btn ${isHost ? 'text' : 'outlined grow'}" data-act="leave">Verlassen</button>
            </div>
          </div>
        </div>`);
      const q = (sel) => root.querySelector(sel);
      const copyBtn = q('[data-act=copy]'); if (copyBtn) copyBtn.onclick = () => copy(o.shareLink);
      const startBtn = q('[data-act=start]'); if (startBtn) startBtn.onclick = () => send({ type: 'start' });
      q('[data-act=leave]').onclick = () => { send({ type: 'leave' }); setTimeout(exit, 250); };
      root.querySelectorAll('[data-kick]').forEach((b) => (b.onclick = () => send({ type: 'kick', player: Number(b.dataset.kick) })));
      root.querySelectorAll('[data-pick]').forEach((b) => (b.onclick = () => { if (isHost) send({ type: 'select_game', game: b.dataset.pick, options: {} }); }));
      root.querySelectorAll('[data-opt]').forEach((b) => (b.onclick = () => {
        const [k, val] = b.dataset.opt.split(':');
        send({ type: 'set_options', options: { ...(room.options || {}), [k]: Number(val) } });
      }));
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
        ${o.shareLink ? `<button class="btn text" data-act="invite">Einladen</button>` : ''}
        ${isHost ? `<button class="btn text" data-act="end">${room.over ? 'Zurück zur Lobby' : 'Runde beenden'}</button>` : ''}`);
      head.style.justifyContent = 'space-between';
      const inv = head.querySelector('[data-act=invite]');
      if (inv) inv.onclick = () => {
        const d = document.createElement('div'); d.className = 'qr-dialog';
        d.innerHTML = `<div class="card qr-card"><div class="title">Einladen</div>${Ada.qrSvg(o.shareLink)}
          <div class="link-box"><code>${esc(o.shareLink)}</code><button class="icon-btn" data-c aria-label="Link kopieren">${icon('copy')}</button></div>
          <button class="btn" data-x>Schließen</button></div>`;
        d.onclick = (e) => { if (e.target === d) d.remove(); };
        d.querySelector('[data-x]').onclick = () => d.remove();
        d.querySelector('[data-c]').onclick = () => copy(o.shareLink);
        document.body.append(d);
      };
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
