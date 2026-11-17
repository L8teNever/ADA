/* Öffentliche Spieleseite: game.kulbarts.com/<token> */
(function () {
  const { esc, icon, api } = Ada;
  const view = document.getElementById('view');
  const token = (location.pathname.split('/')[1] || '').toLowerCase();

  function guestKey() {
    let k = localStorage.getItem('ada.guest.key');
    if (!k || !/^[a-zA-Z0-9]{16,64}$/.test(k)) {
      const b = crypto.getRandomValues(new Uint8Array(16));
      k = Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
      localStorage.setItem('ada.guest.key', k);
    }
    return k;
  }

  function page(html) { view.innerHTML = `<div class="center-page"><div class="card fade-in">${html}</div></div>`; }

  async function start() {
    if (!token) {
      return page(`<div class="headline">Willkommen 👋</div>
        <p class="muted">Öffne den Einladungslink, den dir jemand geschickt hat, um bei einem Spiel mitzumachen.</p>`);
    }
    let info;
    try { info = await api('/api/rooms/' + encodeURIComponent(token)); }
    catch (e) {
      return page(`<div class="headline">Raum nicht gefunden</div><p class="muted">Der Link ist ungültig oder der Raum wurde geschlossen.</p>`);
    }
    const full = info.online >= info.max;
    page(`<div class="label muted">EINLADUNG</div>
      <div class="display">${info.game ? esc(info.game.name) : 'Spiele-Lobby'}</div>
      <p class="muted">${info.game ? esc(info.game.description) : 'Tritt mit deinem Namen bei – der Host wählt, was gespielt wird.'}</p>
      <div class="row"><span class="chip">${icon('group')} ${info.online} online</span>
        <span class="chip ${info.state === 'playing' ? '' : 'ok'}">${info.state === 'playing' ? 'Läuft gerade' : 'Wartet auf Spieler'}</span></div>
      <form id="f" class="stack">
        <label class="field"><input name="name" maxlength="20" required autocomplete="nickname" placeholder="Dein Name" value="${esc(localStorage.getItem('ada.guest.name') || '')}"><span>Name</span></label>
        <button class="btn big" type="submit" ${full ? 'disabled' : ''}>${icon('play')} Mitspielen</button>
      </form>`);
    const input = view.querySelector('input[name=name]');
    input.focus();
    view.querySelector('#f').onsubmit = (e) => {
      e.preventDefault();
      const name = input.value.trim();
      if (!name) return;
      localStorage.setItem('ada.guest.name', name);
      view.innerHTML = '<div id="room"></div>';
      Ada.mountRoom(view.querySelector('#room'), {
        token, wsPath: '/ws/' + encodeURIComponent(token), name, key: guestKey(),
        onExit: () => start(),
      });
    };
  }
  start();
})();
