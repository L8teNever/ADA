/* Skribbl – Oberfläche. Zeichenstriche kommen als leichte Events (Ada.games.skribbl.event),
 * der komplette Zustand liefert Phase, Punkte, Chat und alle bisherigen Striche (für späte Mitspieler). */
(function () {
  const { esc, initial } = Ada;
  const W = 800, H = 600;
  const COLORS = ['#ffffff', '#000000', '#7f7f7f', '#c1c1c1', '#ef130b', '#ff7100', '#ffe400', '#00cc00', '#00b2ff', '#231fd3', '#a300ba', '#d37caa', '#a0522d', '#63300d', '#005c20', '#ffc89a'];
  const WIDTHS = [3, 8, 16, 30];
  const S = { root: null, c2: null, ops: [], turnKey: '', tool: 'brush', color: 1, width: 1, drawing: false, op: null, buf: [], timer: 0, deadline: 0, total: 1, phase: '', v: null, ctx: null, tick: 0, toolsKey: '' };

  const rgb = (hex) => [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)];

  /* ---------- Zeichnen ---------- */
  function clearCanvas() { S.c2.fillStyle = '#fff'; S.c2.fillRect(0, 0, W, H); }
  function drawPts(op, pts) {
    const c = S.c2, p = op.p;
    c.lineCap = 'round'; c.lineJoin = 'round';
    c.strokeStyle = c.fillStyle = COLORS[op.c] || '#000'; c.lineWidth = WIDTHS[op.w] || 8;
    const seq = [];
    if (p.length >= 2) seq.push(p[p.length - 2], p[p.length - 1]);
    seq.push(...pts);
    if (seq.length === 2) { c.beginPath(); c.arc(seq[0], seq[1], c.lineWidth / 2, 0, Math.PI * 2); c.fill(); }
    else { c.beginPath(); c.moveTo(seq[0], seq[1]); for (let i = 2; i < seq.length; i += 2) c.lineTo(seq[i], seq[i + 1]); c.stroke(); }
  }
  function appendPts(op, pts) { drawPts(op, pts); op.p.push(...pts); }
  function floodFill(x, y, ci) {
    const img = S.c2.getImageData(0, 0, W, H), d = img.data;
    const at = (px, py) => (py * W + px) * 4;
    const i0 = at(x, y), tr = d[i0], tg = d[i0 + 1], tb = d[i0 + 2];
    const [fr, fg, fb] = rgb(COLORS[ci] || '#000000');
    if (Math.abs(tr - fr) + Math.abs(tg - fg) + Math.abs(tb - fb) < 12) return;
    const tol = 70;
    const match = (i) => Math.abs(d[i] - tr) + Math.abs(d[i + 1] - tg) + Math.abs(d[i + 2] - tb) <= tol;
    const stack = [[x, y]];
    while (stack.length) {
      let [px, py] = stack.pop();
      while (px >= 0 && match(at(px, py))) px--;
      px++;
      let up = false, down = false;
      while (px < W && match(at(px, py))) {
        const i = at(px, py); d[i] = fr; d[i + 1] = fg; d[i + 2] = fb; d[i + 3] = 255;
        if (py > 0) { const m = match(at(px, py - 1)); if (m && !up) { stack.push([px, py - 1]); up = true; } else if (!m) up = false; }
        if (py < H - 1) { const m = match(at(px, py + 1)); if (m && !down) { stack.push([px, py + 1]); down = true; } else if (!m) down = false; }
        px++;
      }
    }
    S.c2.putImageData(img, 0, 0);
  }
  function redraw() {
    clearCanvas();
    for (const op of S.ops) {
      if (op.k === 's') { const tmp = { c: op.c, w: op.w, p: [] }; if (op.p.length) drawPts(tmp, op.p); }
      else floodFill(op.x, op.y, op.c);
    }
  }
  const sig = (ops) => ops.length + ':' + ops.reduce((n, o) => n + (o.p ? o.p.length : 1), 0);

  /* ---------- Aufbau ---------- */
  function build(el) {
    el.innerHTML = `<div class="sk">
      <div class="sk-bar"><span class="sk-clock">–</span><div class="sk-word"></div><span class="sk-rd"></span></div>
      <div class="sk-main">
        <div class="sk-left">
          <div class="sk-stage"><canvas width="${W}" height="${H}"></canvas><div class="sk-ov"></div></div>
          <div class="sk-tools"></div>
        </div>
        <div class="sk-right"><div class="sk-pl"></div>
          <div class="sk-chat"><div class="sk-log"></div>
            <form class="sk-form"><input maxlength="60" autocomplete="off" autocapitalize="off" enterkeyhint="send"><button class="btn" type="submit" aria-label="Senden">➤</button></form></div>
        </div>
      </div></div>`;
    S.root = el.firstElementChild;
    const canvas = S.root.querySelector('canvas');
    S.c2 = canvas.getContext('2d', { willReadFrequently: true });
    clearCanvas();
    S.ops = []; S.turnKey = '';
    wirePointer(canvas);
    S.root.querySelector('.sk-form').onsubmit = (e) => {
      e.preventDefault();
      const input = e.target.querySelector('input'); const t = input.value.trim();
      if (t) S.ctx.send({ type: 'guess', text: t });
      input.value = '';
    };
    S.root.querySelector('.sk-tools').onclick = onTools;
    S.root.querySelector('.sk-ov').onclick = onOverlay;
    clearInterval(S.tick);
    S.tick = setInterval(updateClock, 250);
  }

  const canDraw = () => S.v && S.v.phase === 'drawing' && S.v.drawer === S.ctx.you;

  function wirePointer(canvas) {
    const pos = (e) => { const r = canvas.getBoundingClientRect(); return [Math.max(0, Math.min(W, Math.round(((e.clientX - r.left) * W) / r.width))), Math.max(0, Math.min(H, Math.round(((e.clientY - r.top) * H) / r.height)))]; };
    const flush = () => {
      clearTimeout(S.timer); S.timer = 0;
      if (S.op && S.buf.length >= 2) { S.ctx.send({ type: 'seg', start: false, c: S.op.c, w: S.op.w, p: S.buf }); S.buf = []; }
    };
    canvas.onpointerdown = (e) => {
      if (!canDraw()) return;
      e.preventDefault(); canvas.setPointerCapture(e.pointerId);
      const [x, y] = pos(e);
      if (S.tool === 'fill') {
        S.ops.push({ k: 'f', c: S.color, x, y }); floodFill(x, y, S.color);
        return S.ctx.send({ type: 'fill', c: S.color, x, y });
      }
      const c = S.tool === 'eraser' ? 0 : S.color;
      S.op = { k: 's', c, w: S.width, p: [] }; S.ops.push(S.op); S.buf = []; S.drawing = true;
      appendPts(S.op, [x, y]);
      S.ctx.send({ type: 'seg', start: true, c, w: S.width, p: [x, y] });
    };
    canvas.onpointermove = (e) => {
      if (!S.drawing || !S.op) return;
      const evs = e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
      const p = S.op.p;
      for (const ev of (evs.length ? evs : [e])) {
        const [x, y] = pos(ev);
        if (Math.hypot(x - p[p.length - 2], y - p[p.length - 1]) < 2.5) continue;
        appendPts(S.op, [x, y]); S.buf.push(x, y);
      }
      if (S.buf.length >= 2 && !S.timer) S.timer = setTimeout(flush, 45);
    };
    const end = () => { if (S.drawing) { S.drawing = false; flush(); S.op = null; } };
    canvas.onpointerup = end; canvas.onpointercancel = end;
  }

  function onTools(e) {
    const b = e.target.closest('button'); if (!b) return;
    if (b.dataset.col != null) { S.color = Number(b.dataset.col); if (S.tool === 'eraser') S.tool = 'brush'; }
    else if (b.dataset.w != null) S.width = Number(b.dataset.w);
    else if (b.dataset.tool) S.tool = b.dataset.tool;
    else if (b.dataset.act === 'undo') { S.ops.pop(); redraw(); S.ctx.send({ type: 'undo' }); }
    else if (b.dataset.act === 'clear') { S.ops = []; clearCanvas(); S.ctx.send({ type: 'clear' }); }
    paintTools();
  }
  function onOverlay(e) {
    const b = e.target.closest('button'); if (!b) return;
    if (b.dataset.pick != null) S.ctx.send({ type: 'pick', index: Number(b.dataset.pick) });
    if (b.dataset.act === 'lobby') S.ctx.toLobby();
  }

  function paintTools() {
    const box = S.root.querySelector('.sk-tools');
    if (!canDraw()) { box.innerHTML = ''; S.toolsKey = ''; return; }
    if (!S.toolsKey) {
      box.innerHTML = `<div class="sk-sw">${COLORS.map((c, i) => `<button data-col="${i}" style="background:${c}" aria-label="Farbe ${i + 1}"></button>`).join('')}</div>
        <div class="sk-tl"><span data-g="w">${WIDTHS.map((w, i) => `<button class="wd" data-w="${i}" aria-label="Dicke ${i + 1}"><i style="width:${4 + i * 5}px;height:${4 + i * 5}px"></i></button>`).join('')}</span>
          <button data-tool="brush" aria-label="Pinsel">✏️</button><button data-tool="eraser" aria-label="Radierer">🧽</button><button data-tool="fill" aria-label="Füllen">🪣</button>
          <span class="sp"></span><button data-act="undo" aria-label="Rückgängig">↶</button><button data-act="clear" aria-label="Alles löschen">🗑</button></div>`;
      S.toolsKey = '1';
    }
    box.querySelectorAll('[data-col]').forEach((b) => b.classList.toggle('on', Number(b.dataset.col) === S.color && S.tool !== 'eraser'));
    box.querySelectorAll('[data-w]').forEach((b) => b.classList.toggle('on', Number(b.dataset.w) === S.width));
    box.querySelectorAll('[data-tool]').forEach((b) => b.classList.toggle('on', b.dataset.tool === S.tool));
  }

  function updateClock() {
    if (!S.root || !S.root.isConnected) { clearInterval(S.tick); return; }
    const el = S.root.querySelector('.sk-clock');
    if (S.phase === 'over' || S.phase === '') { el.textContent = '–'; el.classList.remove('low'); return; }
    const left = Math.max(0, Math.ceil((S.deadline - performance.now()) / 1000));
    el.textContent = left + ' s';
    el.classList.toggle('low', S.phase === 'drawing' && left <= 10);
  }

  function lineHtml(l, name) {
    const who = l.p != null ? `<b>${name(l.p)}</b>` : '';
    switch (l.kind) {
      case 'correct': return `<div class="l correct">✓ ${who} ${esc(l.text)}</div>`;
      case 'sys': return `<div class="l sys">${esc(l.text)}</div>`;
      case 'close': return `<div class="l close">${esc(l.text)}</div>`;
      case 'chat': return `<div class="l chat">${who}: ${esc(l.text)}</div>`;
      default: return `<div class="l">${who}: ${esc(l.text)}</div>`;
    }
  }
  const nameOf = (id) => esc(((S.ctx && S.ctx.players[id]) || {}).name || '?');

  function renderChat(lines) {
    const log = S.root.querySelector('.sk-log');
    const atEnd = log.scrollTop + log.clientHeight >= log.scrollHeight - 30;
    Ada.patch(log, lines.map((l) => lineHtml(l, nameOf)).join(''));
    if (atEnd) log.scrollTop = log.scrollHeight;
  }

  /* ---------- Zustand ---------- */
  function render(el, v, ctx) {
    S.ctx = ctx; S.v = v;
    if (!S.root || !el.contains(S.root)) build(el);
    const you = ctx.you, isDrawer = v.drawer === you, name = nameOf;

    // Neue Runde/Zug: Fläche leeren
    const key = v.round + '-' + v.drawer;
    if (S.turnKey !== key) { S.turnKey = key; S.ops = JSON.parse(JSON.stringify(v.ops)); S.drawing = false; S.op = null; redraw(); }
    else if (!(isDrawer && v.phase === 'drawing') && sig(S.ops) !== sig(v.ops)) { S.ops = JSON.parse(JSON.stringify(v.ops)); redraw(); }

    S.phase = v.phase; S.deadline = performance.now() + v.remaining_ms; S.total = v.total_ms || 1;
    updateClock();

    // Wort-Zeile
    const wordEl = S.root.querySelector('.sk-word');
    let wh = '';
    if (v.phase === 'choosing') wh = isDrawer ? 'Wähle ein Wort' : `${name(v.drawer)} wählt ein Wort…`;
    else if (v.phase === 'drawing') {
      if (v.word) wh = `${esc(v.word)}<small>${isDrawer ? 'Zeichne das Wort!' : 'Du hast es erraten!'}</small>`;
      else wh = `${v.hint.map(esc).join(' ')}<small>${name(v.drawer)} zeichnet · ${v.word_len} Buchstaben</small>`;
    } else if (v.phase === 'turnend') wh = `Das Wort war<small style="font-size:1em;letter-spacing:.14em;color:inherit">${esc(v.word)}</small>`;
    else wh = 'Spiel vorbei';
    if (wordEl.dataset.h !== wh) { wordEl.innerHTML = wh; wordEl.dataset.h = wh; }
    S.root.querySelector('.sk-rd').textContent = `Runde ${v.round}/${v.rounds}`;

    // Overlay
    const ov = S.root.querySelector('.sk-ov');
    const sorted = [...v.scores].sort((a, b) => b.score - a.score);
    let html = '';
    if (v.phase === 'choosing') {
      html = isDrawer
        ? `<div class="big">Wähle ein Wort</div><div class="choices">${v.choices.map((w, i) => `<button class="btn" data-pick="${i}">${esc(w)}</button>`).join('')}</div><div>Sonst wird automatisch eins gewählt.</div>`
        : `<div class="big">${name(v.drawer)}</div><div>wählt gerade ein Wort…</div>`;
    } else if (v.phase === 'turnend') {
      html = `<div class="big">Das Wort war „${esc(v.word)}“</div><table>${[...v.scores].sort((a, b) => b.turn - a.turn).map((s) => `<tr class="${s.id === you ? 'me' : ''}"><td>${name(s.id)}</td><td class="n">${s.turn ? '+' + s.turn : '–'}</td></tr>`).join('')}</table>`;
    } else if (v.phase === 'over') {
      const medal = ['🥇', '🥈', '🥉'];
      html = `<div class="big">🏆 ${sorted[0].id === you ? 'Du gewinnst!' : name(sorted[0].id) + ' gewinnt!'}</div><table>${sorted.map((s, i) => `<tr class="${s.id === you ? 'me' : ''}"><td>${medal[i] || i + 1 + '.'} ${name(s.id)}</td><td class="n">${s.score}</td></tr>`).join('')}</table>${ctx.isHost ? '<button class="btn" data-act="lobby">Zurück zur Lobby</button>' : ''}`;
    }
    ov.classList.toggle('on', !!html);
    Ada.patch(ov, html);

    S.root.querySelector('.sk-stage').classList.toggle('can-draw', canDraw());
    paintTools();

    // Spieler
    Ada.patch(S.root.querySelector('.sk-pl'), sorted.map((s) => `<div class="sk-p ${s.id === v.drawer ? 'drawer' : ''} ${s.guessed ? 'ok' : ''}">
      <span class="avatar">${esc(initial((ctx.players[s.id] || {}).name))}</span>${name(s.id)}${s.id === v.drawer && v.phase === 'drawing' ? ' ✏️' : ''}${s.guessed ? ' ✓' : ''}<b>${s.score}</b>${s.turn ? `<small>+${s.turn}</small>` : ''}</div>`).join(''));

    const input = S.root.querySelector('.sk-form input');
    const knows = isDrawer || (v.scores.find((s) => s.id === you) || {}).guessed;
    input.placeholder = v.phase === 'drawing' ? (knows ? 'Chat (nur für alle, die das Wort kennen)' : 'Tippe deine Antwort…') : 'Nachricht…';
    renderChat(v.chat);
  }

  /* ---------- Leichte Ereignisse (Striche, Chat) ---------- */
  function event(d) {
    if (!S.root || !S.root.isConnected) return;
    if (d.t === 'chat') {
      const log = S.root.querySelector('.sk-log');
      const atEnd = log.scrollTop + log.clientHeight >= log.scrollHeight - 30;
      log.insertAdjacentHTML('beforeend', lineHtml(d.line, nameOf));
      if (atEnd) log.scrollTop = log.scrollHeight;
      return;
    }
    if (S.v && S.v.drawer === S.ctx.you) return; // eigene Striche sind schon da
    if (d.t === 'seg') {
      let op;
      if (d.start) { op = { k: 's', c: d.c, w: d.w, p: [] }; S.ops.push(op); } else op = S.ops[S.ops.length - 1];
      if (op && op.k === 's') appendPts(op, d.p);
    } else if (d.t === 'fill') { S.ops.push({ k: 'f', c: d.c, x: d.x, y: d.y }); floodFill(d.x, d.y, d.c); }
    else if (d.t === 'undo') { S.ops.pop(); redraw(); }
    else if (d.t === 'clear') { S.ops = []; clearCanvas(); }
  }

  Ada.games.skribbl = { render, event };
})();
