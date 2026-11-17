/* UNO – Oberfläche. Server schickt pro Spieler eine Sicht (view), hier wird nur gezeichnet. */
(function () {
  const { esc, initial, icon } = Ada;
  const COLOR_NAME = { red: 'Rot', yellow: 'Gelb', green: 'Grün', blue: 'Blau' };
  let pending = null; // Wild-Karte, bei der noch die Farbe gewählt wird
  let mounted = null;
  let lastHw = 56;

  /** Kartenbreite so wählen, dass die ganze Hand ohne Scrollen sichtbar ist (umbrechen statt abschneiden). */
  function fitHand(el) {
    const hand = el.querySelector('.uno-hand');
    if (!hand) return;
    const n = hand.children.length;
    if (!n) return;
    const W = hand.clientWidth || el.clientWidth;
    const gap = 6;
    const maxW = Math.max(40, Math.min(86, window.innerHeight * 0.13));
    const minW = 46;
    let w = minW;
    for (let rows = 1; rows <= 6; rows++) {
      const per = Math.ceil(n / rows);
      const cand = Math.min(maxW, (W - (per - 1) * gap) / per);
      if (cand >= 56 || rows === 6) { w = Math.max(32, cand); break; }
      w = cand;
    }
    lastHw = Math.floor(w);
    if (hand.style.getPropertyValue('--hw') !== lastHw + 'px') hand.style.setProperty('--hw', lastHw + 'px');
  }
  window.addEventListener('resize', () => mounted && mounted.isConnected && fitHand(mounted));

  function label(kind) {
    if (kind <= 9) return String(kind);
    return { 10: '⊘', 11: '⇄', 12: '+2', 13: '★', 14: '+4' }[kind];
  }
  function card(c, cls = '', attrs = '', tag = 'div') {
    return `<${tag} class="uno-card c-${c.color} ${cls}" ${attrs}><span>${label(c.kind)}</span></${tag}>`;
  }
  function logText(e, players) {
    const n = (id) => esc((players[id] || {}).name || '?');
    switch (e.a) {
      case 'play': return `${n(e.p)} spielt ${e.card.color === 'wild' ? '' : (COLOR_NAME[e.card.color] || '') + ' '}${label(e.card.kind)}${e.card.color === 'wild' ? ' → ' + (COLOR_NAME[e.color] || '') : ''}`;
      case 'draw': return `${n(e.p)} zieht eine Karte`;
      case 'forced': return `${n(e.p)} muss ${e.n} Karten ziehen`;
      case 'uno': return `${n(e.p)}: UNO!`;
      default: return '';
    }
  }

  Ada.games.uno = {
    render(el, v, ctx) {
      const { you, players } = ctx;
      const myTurn = v.current === you && v.winner == null;
      const name = (id) => esc((players[id] || {}).name || '?');
      if (!myTurn) pending = null;

      const opps = v.players.filter((p) => p.id !== you).map((p) => `
        <div class="uno-opp ${p.id === v.current && v.winner == null ? 'turn' : ''}">
          <div class="avatar">${esc(initial((players[p.id] || {}).name))}</div>
          <div><div class="label">${name(p.id)}</div></div>
          <div class="count">${p.cards}</div>
        </div>`).join('');

      const hand = v.hand.map((c) => {
        const ok = v.playable.includes(c.id);
        return card(c, ok ? 'playable' : (myTurn ? 'dim' : ''), `data-card="${c.id}" ${ok ? '' : 'disabled'}`, 'button');
      }).join('');

      let status;
      if (v.winner != null) {
        status = `<div class="uno-win fade-in"><div class="display">🎉</div>
          <div class="headline">${v.winner === you ? 'Du hast gewonnen!' : name(v.winner) + ' hat gewonnen!'}</div>
          ${ctx.isHost ? `<p style="margin-top:16px"><button class="btn" data-act="lobby">Zurück zur Lobby</button></p>` : `<p class="muted">Der Host startet die nächste Runde.</p>`}</div>`;
      } else if (myTurn) {
        status = `<div class="uno-status"><span class="chip ok">Du bist dran</span></div>`;
      } else {
        status = `<div class="uno-status"><span class="chip">${name(v.current)} ist dran</span></div>`;
      }

      const last = v.log.length ? logText(v.log[v.log.length - 1], players) : '';
      const canDraw = myTurn && v.drew == null;
      Ada.patch(el, `<div class="uno">
        <div class="uno-opps">${opps}</div>
        <div class="uno-table">
          <div class="uno-pile">
            ${canDraw ? `<button class="uno-card back playable" data-act="draw" aria-label="Karte ziehen"><span>UNO</span></button>` : `<div class="uno-card back"><span>UNO</span></div>`}
            <span class="label muted">${v.deck} im Stapel</span>
          </div>
          <div class="uno-pile">
            ${card(v.top)}
            <span class="uno-color ${v.color}" title="Aktuelle Farbe: ${COLOR_NAME[v.color] || ''}"></span>
          </div>
          <span class="dir chip" title="Spielrichtung">${v.dir === 1 ? '↻' : '↺'}</span>
        </div>
        ${status}
        <div class="uno-log">${last}</div>
        <div class="uno-actions">
          ${myTurn && v.drew != null ? `<button class="btn tonal" data-act="pass">Passen</button>` : ''}
        </div>
        <div class="uno-hand" style="--hw: ${lastHw}px;">${hand}</div>
        <dialog class="colors"><div class="title">Welche Farbe?</div><div class="uno-colors">
          ${['red', 'yellow', 'green', 'blue'].map((c) => `<button data-color="${c}" style="background:${{ red: '#e53935', yellow: '#fdd835', green: '#43a047', blue: '#1e88e5' }[c]}" aria-label="${COLOR_NAME[c]}"></button>`).join('')}
        </div></dialog>
      </div>`);

      mounted = el; fitHand(el);
      const dlg = el.querySelector('dialog.colors');
      if (pending != null && myTurn) { if (!dlg.open) dlg.showModal(); } else if (dlg.open) dlg.close();
      dlg.oncancel = () => { pending = null; };
      el.querySelectorAll('[data-color]').forEach((b) => (b.onclick = () => {
        const id = pending; pending = null; dlg.close();
        if (id != null) ctx.send({ type: 'play', card: id, color: b.dataset.color });
      }));
      el.querySelectorAll('button[data-card]').forEach((b) => (b.onclick = () => {
        const id = Number(b.dataset.card);
        const c = v.hand.find((x) => x.id === id);
        if (!c || !v.playable.includes(id)) return;
        if (c.color === 'wild') { pending = id; dlg.showModal(); }
        else ctx.send({ type: 'play', card: id });
      }));
      const d = el.querySelector('[data-act=draw]'); if (d) d.onclick = () => ctx.send({ type: 'draw' });
      const p = el.querySelector('[data-act=pass]'); if (p) p.onclick = () => ctx.send({ type: 'pass' });
      const l = el.querySelector('[data-act=lobby]'); if (l) l.onclick = ctx.toLobby;
    },
  };
})();
