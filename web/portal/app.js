(function () {
  const { esc, icon, api, toast, copy, initial, tokenFrom } = Ada;
  const view = document.getElementById('view');
  const titleEl = document.getElementById('title');
  let me, games = [], room = null;

  const NAV = { '/': 'cards', '/rooms': 'group', '/profile': 'person' };
  document.querySelectorAll('.nav-item').forEach((b) => {
    b.querySelector('.pill').innerHTML = icon(NAV[b.dataset.nav]);
    b.onclick = () => (location.hash = '#' + b.dataset.nav);
  });

  function setActive(path) {
    document.querySelectorAll('.nav-item').forEach((b) => b.classList.toggle('active', b.dataset.nav === path));
  }

  function leaveRoom() { if (room) { room.destroy(); room = null; } document.body.classList.remove('in-room'); }

  async function createRoom(gameId) {
    try {
      const r = await api('/api/rooms', { method: 'POST', body: { game: gameId } });
      location.hash = '#/room/' + r.token;
    } catch (e) { toast(e.message); }
  }

  function viewGames() {
    titleEl.textContent = 'ADA';
    view.innerHTML = `<div class="stack fade-in">
      <div class="card hero">
        <div class="label">WILLKOMMEN</div>
        <div class="display">Hi ${esc(me.name)} 👋</div>
        <p>Starte ein Spiel und schick den Link an deine Freunde – oder tritt einem Raum bei.</p>
        <form id="join" class="row" style="flex-wrap:nowrap;margin-top:8px">
          <label class="field" style="flex:1;min-width:0"><input name="code" placeholder="Link oder Code" autocomplete="off" autocapitalize="off"><span>Raum beitreten</span></label>
          <button class="btn" type="submit" aria-label="Beitreten">${icon('forward')}</button>
        </form>
      </div>
      <div class="headline section">Spiele</div>
      <div class="grid">${games.map((g) => `
        <div class="card">
          <div class="icon-badge">${icon('cards')}</div>
          <div class="title">${esc(g.name)}</div>
          <p class="muted">${esc(g.description)}</p>
          <div class="row"><span class="chip">${icon('group')} ${g.min_players}–${g.max_players}</span></div>
          <button class="btn" data-create="${esc(g.id)}">${icon('add')} Raum erstellen</button>
        </div>`).join('') || '<div class="empty">Noch keine Spiele installiert.</div>'}
      </div>
    </div>`;
    view.querySelectorAll('[data-create]').forEach((b) => (b.onclick = () => createRoom(b.dataset.create)));
    view.querySelector('#join').onsubmit = (e) => {
      e.preventDefault();
      const t = tokenFrom(new FormData(e.target).get('code'));
      if (t) location.hash = '#/room/' + t; else toast('Bitte Link oder Code eingeben');
    };
  }

  async function viewRooms() {
    titleEl.textContent = 'Meine Räume';
    view.innerHTML = '<div class="empty">Lade…</div>';
    let rooms = [];
    try { rooms = await api('/api/rooms'); } catch (e) { toast(e.message); }
    if (!rooms.length) {
      view.innerHTML = `<div class="empty fade-in"><div class="title">Keine offenen Räume</div>
        <p>Erstelle unter „Spiele“ einen neuen Raum.</p><p style="margin-top:16px"><a class="btn" href="#/">${icon('add')} Raum erstellen</a></p></div>`;
      return;
    }
    view.innerHTML = `<div class="stack fade-in"><div class="headline section" style="margin-top:8px">Meine Räume</div>
      <div class="grid">${rooms.map((r) => `
        <div class="card">
          <div class="row" style="justify-content:space-between"><div class="title">${esc(r.game.name)}</div>
            <span class="chip ${r.state === 'playing' ? 'ok' : ''}">${r.state === 'playing' ? 'Läuft' : 'Lobby'}</span></div>
          <div class="row"><span class="chip">${icon('group')} ${r.online}/${r.max} online</span></div>
          <div class="link-box"><code>${esc(r.link)}</code><button class="icon-btn" data-copy="${esc(r.link)}" aria-label="Link kopieren">${icon('copy')}</button></div>
          <div class="row">
            <a class="btn" href="#/room/${esc(r.token)}">${icon('play')} Öffnen</a>
            <button class="btn danger" data-del="${esc(r.token)}">${icon('delete')} Löschen</button>
          </div>
        </div>`).join('')}</div></div>`;
    view.querySelectorAll('[data-copy]').forEach((b) => (b.onclick = () => copy(b.dataset.copy)));
    view.querySelectorAll('[data-del]').forEach((b) => (b.onclick = async () => {
      if (!confirm('Raum wirklich löschen?')) return;
      try { await api('/api/rooms/' + b.dataset.del, { method: 'DELETE' }); toast('Raum gelöscht'); viewRooms(); }
      catch (e) { toast(e.message); }
    }));
  }

  function viewProfile() {
    titleEl.textContent = 'Profil';
    view.innerHTML = `<div class="stack fade-in" style="max-width:520px">
      <div class="card hero"><div class="row"><div class="avatar" style="width:64px;height:64px;font-size:1.6rem;background:var(--md-primary);color:var(--md-on-primary)">${esc(initial(me.name))}</div>
        <div><div class="title">${esc(me.name)}</div><div>${esc(me.email)}</div></div></div></div>
      <form class="card" id="prof">
        <div class="title">Anzeigename</div>
        <p class="muted">So sehen dich andere im Spiel. Dein Account wird automatisch über deine Mail angelegt.</p>
        <label class="field"><input name="name" value="${esc(me.name)}" maxlength="20" required><span>Name</span></label>
        <button class="btn" type="submit">${icon('check')} Speichern</button>
      </form></div>`;
    view.querySelector('#prof').onsubmit = async (e) => {
      e.preventDefault();
      try {
        me = await api('/api/me', { method: 'PUT', body: { name: new FormData(e.target).get('name') } });
        toast('Gespeichert'); paintMe(); viewProfile();
      } catch (er) { toast(er.message); }
    };
  }

  async function viewRoom(token) {
    document.body.classList.add('in-room');
    titleEl.textContent = 'Spielraum';
    view.innerHTML = '<div class="empty">Lade…</div>';
    let info;
    try { info = await api('/api/rooms/' + encodeURIComponent(token)); }
    catch (e) {
      view.innerHTML = `<div class="center-page"><div class="card"><div class="title">Raum nicht gefunden</div><p class="muted">${esc(e.message)}</p>
        <a class="btn" href="#/">${icon('back')} Zurück</a></div></div>`;
      return;
    }
    titleEl.textContent = info.game.name;
    view.innerHTML = '<div id="room"></div>';
    room = Ada.mountRoom(view.querySelector('#room'), {
      token, wsPath: '/ws/' + encodeURIComponent(token), shareLink: info.link,
      onExit: () => { room = null; location.hash = '#/'; },
    });
  }

  function paintMe() {
    const a = document.getElementById('me-avatar');
    a.textContent = initial(me.name); a.title = me.email;
  }

  function route() {
    leaveRoom();
    const h = location.hash.slice(1) || '/';
    const m = h.match(/^\/room\/([^/]+)/);
    if (m) { setActive('/'); return viewRoom(m[1]); }
    setActive(NAV[h] ? h : '/');
    if (h === '/rooms') return viewRooms();
    if (h === '/profile') return viewProfile();
    return viewGames();
  }

  (async function boot() {
    try {
      [me, games] = await Promise.all([api('/api/me'), api('/api/games')]);
    } catch (e) {
      view.innerHTML = `<div class="center-page"><div class="card"><div class="title">Nicht angemeldet</div>
        <p class="muted">${esc(e.message)}</p><button class="btn" id="reload">Neu laden</button></div></div>`;
      const rb = document.getElementById('reload'); if (rb) rb.onclick = () => location.reload();
      return;
    }
    paintMe();
    window.addEventListener('hashchange', route);
    route();
  })();
})();
