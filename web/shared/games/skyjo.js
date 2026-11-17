/* Skyjo – Oberfläche. Der Server schickt die Sicht pro Spieler; hier wird nur gezeichnet. */
(function () {
  const { esc } = Ada;
  let mode = 'place'; // nach dem Ziehen: 'place' = Karte ersetzen, 'dflip' = abwerfen & verdeckte Karte aufdecken

  const cls = (v) => (v < 0 ? 'v-neg' : v === 0 ? 'v-zero' : v <= 4 ? 'v-low' : v <= 8 ? 'v-mid' : 'v-high');
  const face = (v, extra = '', attrs = '', tag = 'div') => `<${tag} class="sj-card ${cls(v)} ${extra}" ${attrs}><span>${v}</span></${tag}>`;

  function slotHtml(s, i, pick) {
    if (s.gone) return `<div class="sj-card gone"></div>`;
    const p = pick ? `data-slot="${i}"` : '';
    const tag = pick ? 'button' : 'div';
    if (s.up) return face(s.v, pick ? 'pick' : '', p, tag);
    return `<${tag} class="sj-card back ${pick ? 'pick' : ''}" ${p}></${tag}>`;
  }

  function logText(e, name) {
    switch (e.a) {
      case 'place': return `${name(e.p)} legt ${e.v} und wirft ${e.old} ab`;
      case 'dflip': return `${name(e.p)} wirft ${e.v} ab und deckt ${e.flip} auf`;
      case 'column': return `${name(e.p)} räumt eine Reihe mit ${e.v} ab`;
      case 'ender': return `${name(e.p)} hat alles aufgedeckt – letzte Runde!`;
      default: return '';
    }
  }

  Ada.games.skyjo = {
    render(el, v, ctx) {
      const { you, players } = ctx;
      const name = (id) => esc((players[id] || {}).name || '?');
      const me = v.players.find((p) => p.id === you);
      const myTurn = v.phase === 'turn' && v.current === you;
      if (!(myTurn && v.holding && !v.from_discard)) mode = 'place';

      /* Status-Text und was gerade anklickbar ist */
      let text = '', pickAny = false, pickDown = false;
      if (v.phase === 'setup') {
        const left = 2 - (me ? me.flipped : 0);
        text = left > 0 ? `Decke ${left} Karte${left > 1 ? 'n' : ''} auf` : 'Warte auf die anderen…';
        pickDown = left > 0;
      } else if (v.phase === 'turn') {
        if (myTurn && !v.holding) text = 'Du bist dran: Zieh vom Stapel oder nimm die Ablagekarte';
        else if (myTurn && v.from_discard) { text = 'Tippe auf die Karte, die du ersetzen willst'; pickAny = true; }
        else if (myTurn) {
          if (mode === 'dflip') { text = 'Tippe auf eine verdeckte Karte zum Aufdecken'; pickDown = true; }
          else { text = 'Ersetze eine Karte – oder wirf die Karte ab'; pickAny = true; }
        } else text = `${name(v.current)} ist dran`;
      }
      const badge = v.ender != null && v.phase === 'turn'
        ? `<span class="chip accent">${name(v.ender)} hat alles aufgedeckt – letzte Runde</span>` : '';

      const opps = v.players.filter((p) => p.id !== you).map((p) => `
        <div class="sj-opp ${p.id === v.current && v.phase === 'turn' ? 'turn' : ''}">
          <div class="nm">${name(p.id)}</div>
          <div class="sj-grid mini">${p.slots.map((s) => slotHtml(s, 0, false)).join('')}</div>
          <div class="pts">${p.shown} offen · ${p.total} gesamt</div>
        </div>`).join('');

      const canDraw = myTurn && !v.holding;
      const topCard = v.discard != null ? face(v.discard, canDraw || (myTurn && v.holding && !v.from_discard) ? 'pick' : '', canDraw ? 'data-act="take"' : (myTurn && v.holding && !v.from_discard ? 'data-act="drop"' : ''), canDraw || (myTurn && v.holding && !v.from_discard) ? 'button' : 'div') : `<div class="sj-card empty"></div>`;
      const heldCard = v.holding
        ? `<div class="sj-pile">${v.held != null ? face(v.held) : `<div class="sj-card back"></div>`}<span class="cap">${v.current === you ? 'In deiner Hand' : name(v.current) + ' hält'}</span></div>` : '';

      let myGrid = '';
      if (me) {
        myGrid = me.slots.map((s, i) => {
          const pick = !s.gone && ((pickAny) || (pickDown && !s.up));
          return slotHtml(s, i, pick);
        }).join('');
      }

      let result = '';
      if (v.phase === 'roundover' || v.phase === 'over') {
        const rows = v.last.map((r) => {
          const pl = v.players.find((p) => p.id === r.id);
          return `<tr class="${r.id === you ? 'me' : ''}"><td>${name(r.id)}</td><td class="n">${r.round}${r.doubled ? ' (×2)' : ''}</td><td class="n">${pl ? pl.total : ''}</td></tr>`;
        }).join('');
        result = `<div class="sj-result fade-in">
          <div class="title">${v.phase === 'over' ? (v.winner === you ? '🎉 Du hast gewonnen!' : `🏆 ${name(v.winner)} gewinnt!`) : `Runde ${v.round} vorbei`}</div>
          <table><tr><th>Spieler</th><th class="n">Runde</th><th class="n">Gesamt</th></tr>${rows}</table>
          ${v.phase === 'roundover'
            ? `<button class="btn" data-act="next">Nächste Runde</button>`
            : `<p>Das Spiel endet ab ${v.end_score} Punkten. Wenige Punkte gewinnen.</p>${ctx.isHost ? `<button class="btn" data-act="lobby">Zurück zur Lobby</button>` : ''}`}
        </div>`;
      }

      const last = v.log.length ? logText(v.log[v.log.length - 1], name) : '';
      const actions = myTurn && v.holding && !v.from_discard
        ? `<div class="sj-actions">${mode === 'dflip'
          ? `<button class="btn tonal" data-act="place-mode">Doch ersetzen</button>`
          : `<button class="btn tonal" data-act="dflip-mode">Abwerfen &amp; aufdecken</button>`}</div>` : '';

      el.innerHTML = `<div class="sj">
        <div class="sj-opps">${opps}</div>
        <div class="sj-side">
          <div class="sj-piles">
            <div class="sj-pile">${canDraw
              ? `<button class="sj-card back pick" data-act="draw" aria-label="Karte ziehen"></button>`
              : `<div class="sj-card back"></div>`}<span class="cap">Stapel · ${v.draw}</span></div>
            <div class="sj-pile">${topCard}<span class="cap">Ablage</span></div>
            ${heldCard}
          </div>
          <div class="sj-status"><span class="chip ${myTurn || (v.phase === 'setup' && pickDown) ? 'ok' : ''}">${text}</span>${badge}</div>
          ${actions}
          <div class="sj-log">${last}</div>
        </div>
        <div class="sj-me">
          <div class="label">DEINE KARTEN · ${me ? me.shown : 0} offen · ${me ? me.total : 0} gesamt · Runde ${v.round}</div>
          <div class="sj-grid">${myGrid}</div>
        </div>
        ${result}
      </div>`;

      const on = (sel, fn) => el.querySelectorAll(sel).forEach((b) => (b.onclick = () => fn(b)));
      on('[data-slot]', (b) => {
        const slot = Number(b.dataset.slot);
        if (v.phase === 'setup') ctx.send({ type: 'flip', slot });
        else if (mode === 'dflip') ctx.send({ type: 'discard_flip', slot });
        else ctx.send({ type: 'place', slot });
      });
      on('[data-act=draw]', () => ctx.send({ type: 'draw' }));
      on('[data-act=take]', () => ctx.send({ type: 'take_discard' }));
      on('[data-act=drop]', () => { mode = 'dflip'; Ada.games.skyjo.render(el, v, ctx); });
      on('[data-act=dflip-mode]', () => { mode = 'dflip'; Ada.games.skyjo.render(el, v, ctx); });
      on('[data-act=place-mode]', () => { mode = 'place'; Ada.games.skyjo.render(el, v, ctx); });
      on('[data-act=next]', () => ctx.send({ type: 'next' }));
      on('[data-act=lobby]', () => ctx.toLobby());
    },
  };
})();
