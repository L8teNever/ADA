/* Blackjack – Oberfläche. Der Server schickt pro Spieler eine Sicht (view); hier wird nur gezeichnet. */
(function () {
  const { esc, initial } = Ada;
  const SUITS = ['♠', '♥', '♦', '♣'];
  const RANKS = { 1: 'A', 11: 'J', 12: 'Q', 13: 'K' };
  const RESULT = { win: 'Gewonnen', lose: 'Verloren', push: 'Unentschieden', blackjack: 'Blackjack!', bust: 'Bust' };
  const CHIPS = [10, 50, 100, 500];
  let S = null;          // letzter Zustand (für lokale Neuzeichnung beim Einsatz-Wählen)
  let pend = 0, lastBet = 10, lastRound = 0;
  let clockEnd = 0, clockTimer = null;

  const rank = (r) => RANKS[r] || String(r);
  // Karten: feste 3D-Struktur (Vorder-/Rückseite), damit der Abgleich das Element behält und die
  // verdeckte Dealer-Karte beim Aufdecken umklappt. Neue Karten fliegen gestaffelt aus dem Schuh.
  const delays = new Map(); // "hand|index" -> Verzögerung in s (nur für neu erschienene Karten)
  let delayRound = -1;
  function card(c, key, i) {
    const k = key + '|' + i;
    const d = delays.get(k) || 0;
    const red = c && (c.s === 1 || c.s === 2);
    const face = c
      ? `<span class="cr">${rank(c.r)}<i>${SUITS[c.s]}</i></span><span class="cm">${SUITS[c.s]}</span><span class="cr br">${rank(c.r)}<i>${SUITS[c.s]}</i></span>`
      : '';
    return `<div class="pc ${c ? '' : 'down'} ${red ? 'red' : ''}" data-k="${S.v.round}-${key}-${i}" style="--d:${d}s">
      <div class="pc-in"><div class="face front">${face}</div><div class="face back"></div></div></div>`;
  }
  // Staffelung wie am echten Tisch: erst jeder eine Karte (Dealer zuletzt), dann die zweite.
  function planDelays(v) {
    if (v.round !== delayRound) { delays.clear(); delayRound = v.round; }
    const hands = [];
    v.seats.forEach((s, si) => s.hands.forEach((h, hi) => hands.push({ key: s.id + '.' + hi, n: h.cards.length, order: si })));
    hands.push({ key: 'dealer', n: v.dealer.cards.length, order: v.seats.length });
    const per = v.seats.length + 1;
    for (const h of hands) {
      let first = -1;
      for (let i = 0; i < h.n; i++) if (!delays.has(h.key + '|' + i)) { if (first < 0) first = i; delays.set(h.key + '|' + i, -1); }
      if (first < 0) continue;
      for (let i = first; i < h.n; i++) {
        const step = (i - first) * (first === 0 ? per : 1) + (first === 0 ? h.order : 0);
        delays.set(h.key + '|' + i, Math.round(step * 0.22 * 100) / 100);
      }
    }
  }
  const chipStack = (n) => {
    if (!n) return '';
    const parts = []; let rest = n;
    for (const c of [500, 100, 50, 10]) while (rest >= c && parts.length < 6) { parts.push(c); rest -= c; }
    return `<div class="bj-stack">${parts.map((c, i) => `<span class="chp c${c}" style="--i:${i}"></span>`).join('')}<b>${money(n)}</b></div>`;
  };
  const money = (n) => Number(n).toLocaleString('de-DE');
  const sign = (n) => (n > 0 ? '+' : n < 0 ? '−' : '±') + money(Math.abs(n));

  function handHtml(h, mine, key) {
    let tag = '';
    if (h.state === 'result') tag = `<span class="bj-tag r-${h.result}">${RESULT[h.result] || ''}</span>`;
    else if (h.state === 'bust') tag = `<span class="bj-tag r-bust">Bust</span>`;
    else if (h.state === 'blackjack') tag = `<span class="bj-tag r-blackjack">Blackjack!</span>`;
    else if (h.state === 'stand') tag = `<span class="bj-tag">Halt</span>`;
    return `<div class="bj-hand ${mine ? 'mine' : ''}">
      <div class="bj-cards">${h.cards.map((c, i) => card(c, key, i)).join('')}</div>
      <div class="bj-hmeta">${h.cards.length ? `<span class="bj-val">${h.value}${h.soft ? ' <small>soft</small>' : ''}</span>` : ''}${tag}${chipStack(h.bet)}</div>
    </div>`;
  }

  function seatHtml(s, v, ctx, mine) {
    const p = ctx.players[s.id] || {};
    const turn = v.current === s.id;
    let foot = '';
    if (v.phase === 'betting') foot = !s.can_bet ? 'ohne Chips' : s.has_bet ? `Einsatz ${money(s.hands[0] ? s.hands[0].bet : 0)} ✓` : 'setzt …';
    else if (!s.active) foot = s.can_bet ? 'sitzt aus' : 'ohne Chips';
    else if (v.phase === 'results') foot = `<b class="${s.delta > 0 ? 'plus' : s.delta < 0 ? 'minus' : ''}">${sign(s.delta)}</b>`;
    else if (turn) foot = 'ist dran';
    let banner = '';
    if (mine && v.phase === 'results' && s.active && s.hands.length) {
      const r = s.hands.map((h) => h.result);
      const best = r.includes('blackjack') ? 'blackjack' : s.delta > 0 ? 'win' : s.delta < 0 ? (r.every((x) => x === 'bust') ? 'bust' : 'lose') : 'push';
      banner = `<div class="bj-banner b-${best}" data-k="ban-${v.round}"><span>${RESULT[best]}</span><small>${sign(s.delta)}</small></div>`;
    }
    return `<div class="bj-seat ${mine ? 'me' : ''} ${turn ? 'turn' : ''} ${!s.active && v.phase !== 'betting' ? 'idle' : ''}">${banner}
      <div class="bj-seat-head"><div class="avatar">${esc(initial(p.name))}</div>
        <b class="bj-name">${esc(p.name || '?')}${mine ? ' (du)' : ''}</b>
        <span class="bj-chips"><span class="chp c100 mini"></span>${money(s.chips)}</span></div>
      <div class="bj-hands">${s.hands.map((h, hi) => handHtml(h, mine, s.id + '.' + hi)).join('')}</div>
      <div class="bj-foot">${foot}</div>
    </div>`;
  }

  function controls(v, me, ctx) {
    if (!me) return `<div class="bj-note">Du schaust zu.</div>`;
    if (v.phase === 'betting') {
      if (!me.can_bet) return `<div class="bj-note">Du hast keine Chips mehr – du schaust zu.</div>`;
      if (me.has_bet) return `<div class="bj-note">Einsatz <b>${money(me.hands[0].bet)}</b> gesetzt – warte auf die anderen.</div>
        <button class="btn text" data-act="unbet">Einsatz ändern</button>`;
      return `<div class="bj-bet-row">${CHIPS.map((n) => `<button class="bj-chip c${n}" data-add="${n}" ${me.chips < n ? 'disabled' : ''}><span>${n}</span></button>`).join('')}</div>
        <div class="bj-bet-row">
          <button class="btn tonal" data-act="clear">Löschen</button>
          <button class="btn tonal" data-act="allin">Alles</button>
        </div>
        <button class="btn big" data-act="bet" ${pend >= v.min_bet && pend <= me.chips ? '' : 'disabled'}>Setzen · ${money(pend)}</button>`;
    }
    if (v.phase === 'playing') {
      if (v.current === ctx.you) {
        return `<div class="bj-actions">
          <button class="btn big" data-act="hit" ${v.can.hit ? '' : 'disabled'}>Karte</button>
          <button class="btn big tonal" data-act="stand" ${v.can.stand ? '' : 'disabled'}>Halten</button>
          <button class="btn tonal" data-act="double" ${v.can.double ? '' : 'disabled'}>Verdoppeln</button>
          <button class="btn tonal" data-act="split" ${v.can.split ? '' : 'disabled'}>Teilen</button></div>`;
      }
      const cur = ctx.players[v.current] || {};
      return `<div class="bj-note">${me.active ? `<b>${esc(cur.name || '?')}</b> ist dran …` : 'Du sitzt diese Runde aus.'}</div>`;
    }
    if (v.phase === 'dealer') return `<div class="bj-note">Der Dealer zieht …</div>`;
    // Ergebnis
    const res = me.active
      ? `<div class="bj-note">Ergebnis: <b class="${me.delta > 0 ? 'plus' : me.delta < 0 ? 'minus' : ''}">${sign(me.delta)}</b> Chips</div>
         <button class="btn big" data-act="next" ${me.ready ? 'disabled' : ''}>${me.ready ? 'Warte …' : 'Weiter'}</button>`
      : `<div class="bj-note">Nächste Runde gleich …</div>`;
    return res;
  }

  function standings(v, ctx) {
    const rows = [...v.seats].sort((a, b) => b.chips - a.chips).map((s, i) => {
      const p = ctx.players[s.id] || {};
      return `<div class="list-item"><span class="bj-medal">${['🥇', '🥈', '🥉'][i] || i + 1}</span>
        <div class="grow"><div class="body">${esc(p.name || '?')}${s.id === ctx.you ? ' (du)' : ''}</div></div>
        <b>${money(s.chips)} Chips</b></div>`;
    }).join('');
    return `<div class="bj-over fade-in"><div class="display">🏁</div><div class="headline">Partie vorbei</div>
      <div class="list">${rows}</div>
      ${ctx.isHost ? `<button class="btn big" data-act="lobby">Zurück zur Lobby</button>` : `<p class="muted">Der Host wählt das nächste Spiel.</p>`}</div>`;
  }

  function updateClock() {
    const el = document.querySelector('.bj-clock');
    if (!el) { clearInterval(clockTimer); clockTimer = null; return; }
    if (!clockEnd) { el.textContent = ''; return; }
    el.textContent = Math.max(0, Math.ceil((clockEnd - performance.now()) / 1000)) + ' s';
  }

  const PHASE = { betting: 'Einsätze', playing: 'Spielzug', dealer: 'Dealer', results: 'Ergebnis' };

  function draw(el, v, ctx) {
    S = { el, v, ctx };
    const me = v.seats.find((s) => s.id === ctx.you) || null;
    if (me && me.has_bet && me.hands[0]) lastBet = me.hands[0].bet;
    if (v.phase === 'betting' && v.round !== lastRound) {
      lastRound = v.round;
      pend = me && me.chips >= v.min_bet ? Math.min(lastBet, me.chips) : 0;
    }
    if (me) pend = Math.min(pend, me.chips);
    clockEnd = ['betting', 'playing', 'results'].includes(v.phase) && !v.over ? performance.now() + v.left_ms : 0;

    if (v.over) { Ada.patch(el, `<div class="bj">${standings(v, ctx)}</div>`); bind(el, v, ctx); return; }

    planDelays(v);
    const d = v.dealer;
    const dealer = `<div class="bj-dealer">
      <div class="bj-shoe" aria-hidden="true"><span></span></div>
      <div class="bj-seat-head"><b class="bj-name">Dealer</b>${d.value != null ? `<span class="bj-val ${d.value > 21 ? 'bust' : ''}">${d.value}</span>` : ''}</div>
      <div class="bj-cards">${d.cards.length ? d.cards.map((c, i) => card(c, 'dealer', i)).join('') : '<div class="pc ghost"></div><div class="pc ghost"></div>'}</div>
      <svg class="bj-arc" viewBox="0 0 400 56" aria-hidden="true"><path id="bjArc" d="M10 8 Q200 62 390 8" fill="none"/>
        <text><textPath href="#bjArc" startOffset="50%" text-anchor="middle">BLACKJACK ZAHLT 3 : 2 · DEALER HÄLT AB 17</textPath></text></svg>
    </div>`;
    const others = v.seats.filter((s) => s.id !== ctx.you).map((s) => seatHtml(s, v, ctx, false)).join('');
    Ada.patch(el, `<div class="bj">
      <div class="bj-top"><span class="bj-pill">Runde ${v.round}/${v.rounds}</span><span class="bj-pill gold">${PHASE[v.phase]}</span><span class="bj-pill bj-clock"></span></div>
      <div class="bj-main">
        <div class="bj-board">${dealer}${others ? `<div class="bj-others">${others}</div>` : ''}</div>
        <div class="bj-side">
          ${me ? seatHtml(me, v, ctx, true) : ''}
          <div class="bj-controls">${controls(v, me, ctx)}</div>
        </div>
      </div></div>`);
    updateClock();
    if (!clockTimer) clockTimer = setInterval(updateClock, 250);
    bind(el, v, ctx);
  }

  function bind(el, v, ctx) {
    const me = v.seats.find((s) => s.id === ctx.you);
    const act = (name, fn) => { const b = el.querySelector(`[data-act=${name}]`); if (b) b.onclick = fn; };
    el.querySelectorAll('[data-add]').forEach((b) => (b.onclick = () => {
      pend = Math.min(me.chips, pend + Number(b.dataset.add)); draw(S.el, S.v, S.ctx);
    }));
    act('clear', () => { pend = 0; draw(S.el, S.v, S.ctx); });
    act('allin', () => { pend = me.chips; draw(S.el, S.v, S.ctx); });
    act('bet', () => ctx.send({ type: 'bet', amount: pend }));
    act('unbet', () => ctx.send({ type: 'unbet' }));
    for (const a of ['hit', 'stand', 'double', 'split', 'next']) act(a, () => ctx.send({ type: a }));
    act('lobby', ctx.toLobby);
  }

  Ada.games.blackjack = { render: draw };
})();
