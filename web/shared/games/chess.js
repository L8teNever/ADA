/* Schach – Oberfläche. Das Brett kommt vom Server (inkl. erlaubter Züge), hier wird nur gezeichnet. */
(function () {
  const { esc, initial } = Ada;
  const GLYPH = { p: '♟', n: '♞', b: '♝', r: '♜', q: '♛', k: '♚' };
  const glyph = (ch) => (GLYPH[ch.toLowerCase()] || '') + '︎';
  const piece = (ch) => `<span class="pc ${ch === ch.toUpperCase() ? 'w' : 'b'}">${glyph(ch)}</span>`;
  const sqName = (sq) => 'abcdefgh'[sq % 8] + (Math.floor(sq / 8) + 1);

  let sel = null, promo = null, lastTurn = '';

  const END = {
    stalemate: 'Patt – Remis', draw_50: 'Remis (50-Züge-Regel)', draw_material: 'Remis (zu wenig Material)',
    draw_repetition: 'Remis (dreifache Stellungswiederholung)', draw_agreed: 'Remis vereinbart',
  };

  Ada.games.chess = {
    render(el, v, ctx) {
      const { players } = ctx;
      const name = (id) => esc((players[id] || {}).name || '?');
      const playing = v.status === 'playing';
      const flipped = v.you === 'b';
      const myTurn = playing && v.you && v.turn === v.you;
      if (!myTurn || v.turn + v.moves.length !== lastTurn) { sel = null; promo = null; }
      lastTurn = v.turn + v.moves.length;

      const targets = new Set(sel != null ? v.legal.filter((m) => m[0] === sel).map((m) => m[1]) : []);
      const movable = new Set(v.legal.map((m) => m[0]));
      let cells = '';
      for (let i = 0; i < 64; i++) {
        const row = Math.floor(i / 8), col = i % 8;
        const r = flipped ? row : 7 - row, f = flipped ? 7 - col : col;
        const sq = r * 8 + f;
        const ch = v.board[sq];
        const cls = ['ch-sq', (r + f) % 2 === 0 ? 'dark' : '',
          v.last && (v.last[0] === sq || v.last[1] === sq) ? 'last' : '',
          sel === sq ? 'sel' : '', v.check === sq ? 'check' : '',
          targets.has(sq) ? 'move' : '', targets.has(sq) && ch ? 'cap' : '',
          movable.has(sq) || targets.has(sq) ? 'mine' : ''].join(' ');
        const co = (col === 0 ? `<span class="co r">${r + 1}</span>` : '') + (row === 7 ? `<span class="co f">${'abcdefgh'[f]}</span>` : '');
        cells += `<button class="${cls}" data-sq="${sq}" aria-label="${sqName(sq)}">${co}${ch ? piece(ch) : ''}</button>`;
      }

      const pid = { w: v.white, b: v.black };
      const me = v.you || 'w';
      const opp = me === 'w' ? 'b' : 'w';
      const caps = (c) => (c === 'w' ? v.by_w : v.by_b).map((x) => glyph(x)).join('');
      const bar = (c) => `<div class="ch-bar ${playing && v.turn === c ? 'turn' : ''}">
        <span class="dot ${c}"></span><div class="avatar ${c === 'w' ? '' : 'c2'}">${esc(initial((players[pid[c]] || {}).name))}</div>
        <div class="nm">${name(pid[c])}${pid[c] === ctx.you ? ' (du)' : ''}</div><div class="caps">${caps(c)}</div></div>`;

      let text;
      if (playing) {
        text = myTurn ? 'Du bist dran' : (v.you ? `${name(pid[v.turn])} ist dran` : `${name(pid[v.turn])} ist am Zug`);
        if (v.check != null) text += ' – Schach!';
      } else if (v.status === 'checkmate') text = `Schachmatt – ${v.winner === ctx.you ? 'Du gewinnst! 🎉' : name(v.winner) + ' gewinnt'}`;
      else if (v.status === 'resigned') text = v.winner === ctx.you ? `${name(pid[opp])} hat aufgegeben – du gewinnst! 🎉` : `${name(v.winner === pid.w ? pid.b : pid.w)} hat aufgegeben`;
      else text = END[v.status] || 'Partie beendet';

      let actions = '';
      if (playing && v.you) {
        if (v.draw_offer && v.draw_offer !== v.you) actions += `<button class="btn tonal" data-act="accept">Remis annehmen</button><button class="btn text" data-act="decline">Ablehnen</button>`;
        else if (v.draw_offer === v.you) actions += `<span class="chip">Remis angeboten</span>`;
        else actions += `<button class="btn tonal" data-act="offer">Remis anbieten</button>`;
        actions += `<button class="btn danger" data-act="resign">Aufgeben</button>`;
      } else if (!playing && ctx.isHost) actions = `<button class="btn" data-act="lobby">Zurück zur Lobby</button>`;

      let moves = '';
      for (let i = 0; i < v.moves.length; i += 2) moves += `<span><b>${i / 2 + 1}.</b> ${esc(v.moves[i])}${v.moves[i + 1] ? ' ' + esc(v.moves[i + 1]) : ''}</span>`;

      el.innerHTML = `<div class="ch">
        <div class="ch-board-col">
          ${bar(opp)}
          <div class="ch-board-wrap"><div class="ch-board">${cells}</div>
            ${promo ? `<div class="ch-promo"><div class="box">${['q', 'r', 'b', 'n'].map((p) => `<button data-promo="${p}">${piece(me === 'w' ? p.toUpperCase() : p)}</button>`).join('')}</div></div>` : ''}
          </div>
          ${bar(me)}
        </div>
        <div class="ch-side">
          <div class="ch-status"><span class="chip ${myTurn ? 'ok' : ''}">${text}</span><div class="ch-actions">${actions}</div></div>
          <div class="ch-moves">${moves}</div>
        </div>
      </div>`;

      const rerender = () => Ada.games.chess.render(el, v, ctx);
      el.querySelectorAll('[data-sq]').forEach((b) => (b.onclick = () => {
        if (!myTurn) return;
        const sq = Number(b.dataset.sq);
        if (sel != null && v.legal.some((m) => m[0] === sel && m[1] === sq)) {
          const isPawn = v.board[sel].toLowerCase() === 'p';
          const lastRank = Math.floor(sq / 8) === (v.you === 'w' ? 7 : 0);
          if (isPawn && lastRank) { promo = { from: sel, to: sq }; return rerender(); }
          const from = sel; sel = null;
          return ctx.send({ type: 'move', from: sqName(from), to: sqName(sq) });
        }
        sel = movable.has(sq) ? (sel === sq ? null : sq) : null;
        rerender();
      }));
      el.querySelectorAll('[data-promo]').forEach((b) => (b.onclick = () => {
        const p = promo; promo = null; sel = null;
        ctx.send({ type: 'move', from: sqName(p.from), to: sqName(p.to), promo: b.dataset.promo });
      }));
      const on = (a, fn) => { const b = el.querySelector(`[data-act=${a}]`); if (b) b.onclick = fn; };
      on('resign', () => { if (confirm('Wirklich aufgeben?')) ctx.send({ type: 'resign' }); });
      on('offer', () => ctx.send({ type: 'offer_draw' }));
      on('accept', () => ctx.send({ type: 'accept_draw' }));
      on('decline', () => ctx.send({ type: 'decline_draw' }));
      on('lobby', () => ctx.toLobby());
    },
  };
})();
