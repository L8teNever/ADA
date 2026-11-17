/* Gemeinsame Helfer für Portal und Spieleseite */
(function () {
  const ICONS = {
    add: 'M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z',
    close: 'M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z',
    copy: 'M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z',
    play: 'M8 5v14l11-7z',
    home: 'M10 20v-6h4v6h5v-8h3L12 3 2 12h3v8z',
    group: 'M16 11c1.66 0 2.99-1.34 2.99-3S17.66 5 16 5c-1.66 0-3 1.34-3 3s1.34 3 3 3zm-8 0c1.66 0 2.99-1.34 2.99-3S9.66 5 8 5C6.34 5 5 6.34 5 8s1.34 3 3 3zm0 2c-2.33 0-7 1.17-7 3.5V19h14v-2.5c0-2.33-4.67-3.5-7-3.5zm8 0c-.29 0-.62.02-.97.05 1.16.84 1.97 1.97 1.97 3.45V19h6v-2.5c0-2.33-4.67-3.5-7-3.5z',
    person: 'M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z',
    back: 'M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z',
    forward: 'M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z',
    link: 'M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z',
    check: 'M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z',
    delete: 'M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z',
    edit: 'M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z',
    cards: 'M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2zm6 5.5c-2 0-3.5 1.9-3.5 4.5s1.5 4.5 3.5 4.5 3.5-1.9 3.5-4.5S14 7.5 12 7.5z',
  };
  const EVENODD = { cards: 1 };

  const Ada = (window.Ada = { games: {} });

  Ada.esc = (s) =>
    String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  Ada.icon = (name) =>
    `<svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d="${ICONS[name] || ''}"${EVENODD[name] ? ' fill-rule="evenodd"' : ''}/></svg>`;

  Ada.initial = (name) => (String(name || '?').trim()[0] || '?').toUpperCase();

  Ada.api = async (path, opts = {}) => {
    const res = await fetch(path, {
      method: opts.method || 'GET',
      headers: opts.body ? { 'Content-Type': 'application/json' } : undefined,
      body: opts.body ? JSON.stringify(opts.body) : undefined,
      credentials: 'same-origin',
    });
    let data = null;
    try { data = await res.json(); } catch (_) { /* leer */ }
    if (!res.ok) throw new Error((data && data.error) || `Fehler ${res.status}`);
    return data;
  };

  let toastTimer;
  Ada.toast = (msg) => {
    let el = document.querySelector('.snackbar');
    if (!el) { el = document.createElement('div'); el.className = 'snackbar'; el.setAttribute('role', 'status'); document.body.append(el); }
    el.textContent = msg;
    requestAnimationFrame(() => el.classList.add('show'));
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => el.classList.remove('show'), 2600);
  };

  Ada.copy = async (text) => {
    try { await navigator.clipboard.writeText(text); }
    catch (_) {
      const t = document.createElement('textarea'); t.value = text; document.body.append(t); t.select();
      try { document.execCommand('copy'); } catch (__) { /* egal */ }
      t.remove();
    }
    Ada.toast('Link kopiert');
  };

  /** Token aus Link oder Code ziehen (…/abc123 → abc123) */
  Ada.tokenFrom = (input) => {
    const s = String(input || '').trim().replace(/[?#].*$/, '').replace(/\/+$/, '');
    return (s.split('/').pop() || '').toLowerCase();
  };
})();
