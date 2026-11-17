/* Monopoly – Oberfläche. Der Server liefert Brett, Besitz und Phase; hier wird gezeichnet und Aktionen gesendet. */
(function () {
  const { esc, initial } = Ada;
  const GROUP_COL = ['#955436', '#aae0fa', '#d93a96', '#f7941d', '#ed1b24', '#fef200', '#1fb25a', '#0072bb', '#6b6b6b', '#9a9a9a'];
  const PCOL = ['#e53935', '#1e88e5', '#43a047', '#fb8c00', '#8e24aa', '#00acc1'];
  const ICON = { go: '🏁', chance: '❓', chest: '🎁', tax: '💸', jail: '🔒', free: '🅿️', gojail: '👮', rail: '🚂', util: '💡' };
  const eur = (n) => `${n} €`;

  const ui = { tab: 'props', info: null, draft: null, bid: '' };
  
  function gridPos(pos) {
    if (pos <= 10) return { r: 11, c: 11 - pos, side: pos === 0 || pos === 10 ? 'b' : 'b' };
    if (pos <= 20) return { r: 11 - (pos - 10), c: 1, side: 'l' };
    if (pos <= 30) return { r: 1, c: 1 + (pos - 20), side: 't' };
    return { r: 1 + (pos - 30), c: 11, side: 'r' };
  }
  const isCorner = (p) => p % 10 === 0;
  const members = (v, g) => v.board.map((s, i) => (s.g === g && (s.t === 'prop' || s.t === 'rail' || s.t === 'util') ? i : -1)).filter((i) => i >= 0);

  Ada.games.monopoly = {
    render(el, v, ctx) {
      const you = ctx.you;
      const pl = v.players;
      const idxOf = (id) => pl.findIndex((p) => p.id === id);
      const col = (id) => PCOL[idxOf(id) % PCOL.length];
      const name = (id) => esc((ctx.players[id] || {}).name || '?');
      const me = pl.find((p) => p.id === you);
      const myTurn = v.current === you && !(me && me.out) && v.phase !== 'over';
      const tradeOpen = !!v.trade;
      const canManage = myTurn && (v.phase === 'roll' || v.phase === 'end');
      const canRaise = myTurn && v.phase === 'debt';
      const logText = (e) => `${name(e.p)} ${esc(e.t)}${e.q ? ' ' + name(e.q) : ''}`;

      /* ---------- Brett ---------- */
      let squares = '';
      for (let pos = 0; pos < 40; pos++) {
        const s = v.board[pos], o = v.own[pos], g = gridPos(pos);
        const corner = isCorner(pos);
        const owned = o && o.o != null;
        let band = '';
        if (s.t === 'prop') {
          const pips = o && o.h === 5 ? '<i class="htl"></i>' : '<i class="hs"></i>'.repeat(o ? o.h : 0);
          band = `<div class="band" style="background:${GROUP_COL[s.g]}">${pips}</div>`;
        } else if (s.t === 'rail' || s.t === 'util') band = `<div class="band" style="background:${GROUP_COL[s.g]}"></div>`;
        const price = s.t === 'prop' || s.t === 'rail' || s.t === 'util' ? `<span class="pr">${s.p}</span>` : s.t === 'tax' ? `<span class="pr">${s.p}</span>` : '';
        const ic = ICON[s.t] && s.t !== 'prop' ? `<span class="ic">${ICON[s.t]}</span>` : '';
        const toks = pl.filter((p) => !p.out && p.pos === pos).map((p) => `<i class="tok ${p.id === you ? 'me' : ''}" style="background:${col(p.id)}"></i>`).join('');
        const cls = `mo-sq side-${g.side} ${corner ? 'corner' : ''} ${owned ? 'owned' : ''} ${o && o.m ? 'mort' : ''}`;
        squares += `<button class="${cls}" data-sq="${pos}" style="grid-row:${g.r};grid-column:${g.c};${owned ? `--own:${col(o.o)}` : ''}" aria-label="${esc(s.n)}">
          ${band}<div class="body">${ic}<span class="nm">${esc(s.n)}</span>${price}</div><div class="toks">${toks}</div></button>`;
      }

      /* ---------- Mitte: Würfel + Aktionen je nach Phase ---------- */
      const sqName = (p) => esc(v.board[p].n);
      const mini = (p) => { const s = v.board[p]; const c = s.g != null ? GROUP_COL[s.g] : '#999'; return `<div class="mo-mini"><span class="band" style="background:${c}"></span><div><div class="t">${esc(s.n)}</div><div class="sub">${eur(s.p)}</div></div></div>`; };
      let center = '';
      const dice = v.dice ? `<div class="row"><div class="mo-die" data-k="a${v.rolls}">${v.dice[0]}</div><div class="mo-die" data-k="b${v.rolls}">${v.dice[1]}</div></div>` : '';
      const recentCard = v.card && v.log.slice(-4).some((e) => e.t.startsWith('zieht eine Karte'));
      const cardNote = recentCard ? `<div class="mo-note">${v.card.deck === 'chance' ? '❓ Ereignis' : '🎁 Gemeinschaft'}: ${esc(v.card.text)}</div>` : '';
      const offerLines = (o) => [o.money ? eur(o.money) : '', ...o.props.map((p) => sqName(p)), o.cards ? `${o.cards}× Freikarte` : ''].filter(Boolean).join(', ') || 'nichts';

      if (v.phase === 'over') {
        center = `<div class="t" style="font-size:1.3em">🏆 ${v.winner === you ? 'Du hast gewonnen!' : name(v.winner) + ' gewinnt!'}</div>${ctx.isHost ? '<button class="btn" data-act="lobby">Zurück zur Lobby</button>' : ''}`;
      } else if (v.trade && v.trade.to === you) {
        center = `<div class="t">Handelsangebot von ${name(v.trade.from)}</div>
          <div class="sub">Du bekommst: ${offerLines(v.trade.give)}</div><div class="sub">Du gibst: ${offerLines(v.trade.take)}</div>
          <div class="row"><button class="btn" data-act="trade_accept">Annehmen</button><button class="btn tonal" data-act="trade_decline">Ablehnen</button></div>`;
      } else if (v.phase === 'roll') {
        if (myTurn) {
          center = `${dice}${cardNote}<div class="t">Du bist dran</div>`;
          if (me.jail) {
            center += `<div class="sub">Du sitzt im Gefängnis (Pasch = frei)</div><div class="row"><button class="btn" data-act="roll">Würfeln</button>
              <button class="btn tonal small" data-act="pay_jail">50 € zahlen</button>${me.cards ? '<button class="btn tonal small" data-act="use_card">Freikarte</button>' : ''}</div>`;
          } else center += `<button class="btn" data-act="roll" ${tradeOpen ? 'disabled' : ''}>🎲 Würfeln</button>`;
          if (tradeOpen && v.trade.from === you) center += `<div class="sub">Angebot an ${name(v.trade.to)} offen <button class="btn text small" data-act="trade_cancel">zurückziehen</button></div>`;
        } else center = `${dice}${cardNote}<div class="t">${name(v.current)} ist am Zug</div>`;
      } else if (v.phase === 'buy') {
        center = `${dice}${cardNote}${mini(v.buy)}`;
        center += myTurn
          ? `<div class="row"><button class="btn" data-act="buy" ${me.money < v.board[v.buy].p ? 'disabled' : ''}>Kaufen</button><button class="btn tonal" data-act="decline_buy">Auktion</button></div>`
          : `<div class="sub">${name(v.current)} überlegt zu kaufen…</div>`;
      } else if (v.phase === 'auction' && v.auction) {
        const a = v.auction, iPassed = a.passed.includes(you), can = me && !me.out && !iPassed && me.money > a.bid;
        center = `<div class="t">🔨 Auktion</div>${mini(a.pos)}<div>Höchstgebot: <b>${eur(a.bid)}</b>${a.bidder ? ' von ' + name(a.bidder) : ''}</div>`;
        if (can) {
          center += `<div class="row">${[10, 50, 100].map((d) => `<button class="btn tonal small" data-bid="${a.bid + d}" ${me.money < a.bid + d ? 'disabled' : ''}>+${d}</button>`).join('')}</div>
            <div class="row"><input type="number" inputmode="numeric" min="${a.bid + 1}" max="${me.money}" placeholder="${a.bid + 1}" value="${esc(ui.bid)}" data-bidin><button class="btn small" data-act="bidcustom">Bieten</button><button class="btn text small" data-act="pass">Passen</button></div>`;
        } else center += `<div class="sub">${iPassed ? 'Du hast gepasst.' : 'Du kannst nicht mehr überbieten.'}</div>`;
      } else if (v.phase === 'debt' && v.debt) {
        const d = v.debt;
        center = `${cardNote}<div class="t">${myTurn ? 'Du musst' : name(v.current) + ' muss'} ${eur(d.amount)} zahlen</div>
          <div class="sub">${d.creditor ? 'an ' + name(d.creditor) : 'an die Bank'}</div>`;
        if (myTurn) center += `<div class="sub">Verkaufe Häuser oder nimm Hypotheken auf (Tab „Grundstücke“).</div>
          <div class="row"><button class="btn" data-act="pay_debt" ${me.money < d.amount ? 'disabled' : ''}>Bezahlen (${eur(me.money)})</button><button class="btn danger small" data-act="surrender">Aufgeben</button></div>`;
      } else {
        center = `${dice}${cardNote}`;
        if (myTurn) {
          center += `<div class="t">${v.extra ? 'Pasch! Nochmal dran' : 'Dein Zug'}</div><button class="btn" data-act="end_turn" ${tradeOpen ? 'disabled' : ''}>${v.extra ? '🎲 Nochmal würfeln' : 'Zug beenden'}</button>`;
          if (tradeOpen && v.trade.from === you) center += `<div class="sub">Angebot an ${name(v.trade.to)} offen <button class="btn text small" data-act="trade_cancel">zurückziehen</button></div>`;
        } else center += `<div class="t">${name(v.current)} ist am Zug</div>`;
      }

      /* ---------- Spielerleiste ---------- */
      const top = pl.map((p, i) => `<button class="mo-pl ${p.id === v.current && v.phase !== 'over' ? 'turn' : ''} ${p.out ? 'out' : ''}" data-pl="${p.id}">
        <span class="dot" style="background:${PCOL[i % PCOL.length]}">${esc(initial((ctx.players[p.id] || {}).name))}</span>${name(p.id)}${p.jail ? ' 🔒' : ''}<span class="m">${eur(p.money)}</span></button>`).join('');

      /* ---------- Tabs ---------- */
      const myProps = v.own.map((o, i) => (o && o.o === you ? i : -1)).filter((i) => i >= 0);
      const rowsProps = () => {
        if (!me || me.out) return '<div class="sub">Du bist ausgeschieden.</div>';
        if (!myProps.length) return '<div class="sub">Dir gehört noch nichts. Kaufe Grundstücke, wenn du darauf landest.</div>';
        let html = '';
        const groups = [...new Set(myProps.map((p) => v.board[p].g))].sort((a, b) => a - b);
        for (const g of groups) {
          const all = members(v, g), mine = all.filter((p) => v.own[p].o === you);
          const full = mine.length === all.length;
          html += `<div class="mo-gh">${g < 8 ? (full ? 'MONOPOL · ' : '') + mine.length + '/' + all.length : g === 8 ? 'BAHNHÖFE' : 'WERKE'}</div>`;
          const minH = Math.min(...all.map((p) => v.own[p].h)), maxH = Math.max(...all.map((p) => v.own[p].h));
          const anyMort = all.some((p) => v.own[p].m);
          for (const p of mine) {
            const s = v.board[p], o = v.own[p];
            const btns = [];
            if (s.t === 'prop' && full && !anyMort && o.h < 5 && o.h <= minH && me.money >= s.h && canManage) btns.push(`<button class="btn tonal" data-act="build" data-pos="${p}">Bauen ${eur(s.h)}</button>`);
            if (s.t === 'prop' && o.h > 0 && o.h >= maxH && (canManage || canRaise)) btns.push(`<button class="btn tonal" data-act="sell_house" data-pos="${p}">Verkaufen +${eur(s.h / 2)}</button>`);
            if (!o.m && o.h === 0 && (s.t !== 'prop' || all.every((x) => v.own[x].h === 0)) && (canManage || canRaise)) btns.push(`<button class="btn tonal" data-act="mortgage" data-pos="${p}">Hypothek +${eur(s.p / 2)}</button>`);
            if (o.m && canManage) { const cost = Math.ceil((s.p / 2) * 1.1); btns.push(`<button class="btn tonal" data-act="unmortgage" data-pos="${p}" ${me.money < cost ? 'disabled' : ''}>Auslösen ${eur(cost)}</button>`); }
            const st = o.m ? 'Hypothek' : o.h === 5 ? 'Hotel' : o.h ? o.h + ' Haus' + (o.h > 1 ? 'er' : '') : '';
            html += `<div class="mo-row"><span class="chipc" style="background:${GROUP_COL[s.g]}"></span><div class="grow"><b>${esc(s.n)}</b> <span class="st">${st}</span></div>${btns.join('')}<button class="btn text" data-sq="${p}">Info</button></div>`;
          }
        }
        return html;
      };
      const rowsPlayers = () => pl.map((p, i) => {
        const ps = v.own.map((o, k) => (o && o.o === p.id ? k : -1)).filter((k) => k >= 0);
        const chips = ps.map((k) => `<span class="chipc" style="display:inline-block;width:10px;height:16px;margin-right:2px;background:${GROUP_COL[v.board[k].g]};opacity:${v.own[k].m ? .35 : 1}" title="${esc(v.board[k].n)}"></span>`).join('');
        const canTrade = p.id !== you && !p.out && canManage && !tradeOpen;
        return `<div class="mo-row"><span class="chipc" style="background:${PCOL[i % PCOL.length]};height:34px"></span><div class="grow"><b>${name(p.id)}</b> <span class="st">${p.out ? 'ausgeschieden' : eur(p.money) + ' · ' + esc(v.board[p.pos].n) + (p.jail ? ' · Gefängnis' : '') + (p.cards ? ' · ' + p.cards + '× Freikarte' : '')}</span><div>${chips}</div></div>
          ${canTrade ? `<button class="btn tonal" data-trade="${p.id}">Handeln</button>` : ''}</div>`;
      }).join('') + `<div class="st">Bank: ${v.houses_left} Häuser · ${v.hotels_left} Hotels</div>`;
      const rowsLog = () => [...v.log].reverse().map((e) => `<div class="mo-log">${logText(e)}</div>`).join('') || '<div class="sub">Noch nichts passiert.</div>';
      const tabBody = ui.tab === 'props' ? rowsProps() : ui.tab === 'players' ? rowsPlayers() : rowsLog();

      /* ---------- Modals ---------- */
      let modal = '';
      if (ui.info != null) {
        const s = v.board[ui.info], o = v.own[ui.info];
        const own = o && o.o != null ? `<b>${name(o.o)}</b>${o.m ? ' (Hypothek)' : ''}` : 'Bank';
        let rows = '';
        if (s.t === 'prop') rows = `<tr><td>Miete</td><td>${eur(s.r[0])}</td></tr><tr><td>Mit ganzer Farbgruppe</td><td>${eur(s.r[0] * 2)}</td></tr>` +
          [1, 2, 3, 4].map((h) => `<tr><td>Mit ${h} Haus${h > 1 ? 'er' : ''}</td><td>${eur(s.r[h])}</td></tr>`).join('') + `<tr><td>Mit Hotel</td><td>${eur(s.r[5])}</td></tr><tr><td>Haus / Hotel kostet</td><td>${eur(s.h)}</td></tr>`;
        else if (s.t === 'rail') rows = '<tr><td>1 Bahnhof</td><td>25 €</td></tr><tr><td>2 Bahnhöfe</td><td>50 €</td></tr><tr><td>3 Bahnhöfe</td><td>100 €</td></tr><tr><td>4 Bahnhöfe</td><td>200 €</td></tr>';
        else if (s.t === 'util') rows = '<tr><td>1 Werk</td><td>4× Augenzahl</td></tr><tr><td>2 Werke</td><td>10× Augenzahl</td></tr>';
        else if (s.t === 'tax') rows = `<tr><td>Steuer</td><td>${eur(s.p)}</td></tr>`;
        modal = `<div class="mo-modal" data-close><div class="mo-sheet">
          <div class="head">${s.g != null ? `<span class="chipc" style="width:14px;height:40px;border-radius:5px;background:${GROUP_COL[s.g]}"></span>` : `<span class="ic" style="font-size:2rem">${ICON[s.t] || ''}</span>`}
            <div><div class="title">${esc(s.n)}</div>${s.p && s.t !== 'tax' ? `<div class="sub muted">Preis ${eur(s.p)} · Hypothek ${eur(s.p / 2)}</div>` : ''}</div></div>
          ${s.t === 'prop' || s.t === 'rail' || s.t === 'util' ? `<div>Besitzer: ${own}${o && o.h ? ` · ${o.h === 5 ? 'Hotel' : o.h + ' Haus/Häuser'}` : ''}</div>` : ''}
          <table>${rows}</table><button class="btn" data-close>Schließen</button></div></div>`;
      }
      if (ui.draft) {
        const d = ui.draft;
        const tradable = (id) => v.own.map((o, i) => (o && o.o === id ? i : -1)).filter((i) => i >= 0)
          .filter((i) => v.board[i].t !== 'prop' || members(v, v.board[i].g).every((m) => v.own[m].h === 0));
        const partner = pl.find((p) => p.id === d.to);
        const pick = (id, side) => tradable(id).map((i) => `<label class="${d[side].props.has(i) ? 'on' : ''}" style="--gc:${GROUP_COL[v.board[i].g]}"><input type="checkbox" data-tp="${side}:${i}" ${d[side].props.has(i) ? 'checked' : ''}>${esc(v.board[i].n)}${v.own[i].m ? ' (H)' : ''}</label>`).join('') || '<span class="st">keine handelbaren Grundstücke</span>';
        modal = `<div class="mo-modal"><div class="mo-sheet">
          <div class="title">Handel mit ${name(d.to)}</div>
          <div class="mo-two">
            <div><h4>Du gibst</h4><input class="mo-money" type="number" inputmode="numeric" min="0" max="${me.money}" placeholder="Geld (€)" value="${d.give.money || ''}" data-tm="give">
              <div class="mo-pick">${pick(you, 'give')}</div>${me.cards ? `<label class="st"><input type="checkbox" data-tc="give" ${d.give.cards ? 'checked' : ''}> Freikarte</label>` : ''}</div>
            <div><h4>Du bekommst</h4><input class="mo-money" type="number" inputmode="numeric" min="0" max="${partner ? partner.money : 0}" placeholder="Geld (€)" value="${d.take.money || ''}" data-tm="take">
              <div class="mo-pick">${pick(d.to, 'take')}</div>${partner && partner.cards ? `<label class="st"><input type="checkbox" data-tc="take" ${d.take.cards ? 'checked' : ''}> Freikarte</label>` : ''}</div>
          </div>
          <div class="row"><button class="btn" data-act="trade_send">Angebot senden</button><button class="btn text" data-act="trade_close">Abbrechen</button></div></div></div>`;
      }

      Ada.patch(el, `<div class="mo">
        <div class="mo-top">${top}</div>
        <div class="mo-board">${squares}<div class="mo-center">${center}</div></div>
        <div class="mo-panel"><div class="mo-tabs">
          ${[['props', 'Grundstücke'], ['players', 'Spieler'], ['log', 'Protokoll']].map(([k, l]) => `<button class="${ui.tab === k ? 'on' : ''}" data-tab="${k}">${l}</button>`).join('')}</div>
          <div class="mo-body">${tabBody}</div></div>${modal}</div>`);

      /* ---------- Ereignisse ---------- */
      const again = () => Ada.games.monopoly.render(el, v, ctx);
      const send = (a) => ctx.send(a);
      el.querySelectorAll('[data-tab]').forEach((b) => (b.onclick = () => { ui.tab = b.dataset.tab; again(); }));
      el.querySelectorAll('[data-sq]').forEach((b) => (b.onclick = () => { ui.info = Number(b.dataset.sq); again(); }));
      el.querySelectorAll('[data-close]').forEach((b) => (b.onclick = (e) => { if (e.target === b) { ui.info = null; again(); } }));
      el.querySelectorAll('[data-pl]').forEach((b) => (b.onclick = () => { ui.tab = 'players'; again(); }));
      el.querySelectorAll('[data-trade]').forEach((b) => (b.onclick = () => {
        ui.draft = { to: Number(b.dataset.trade), give: { money: 0, props: new Set(), cards: 0 }, take: { money: 0, props: new Set(), cards: 0 } }; again();
      }));
      el.querySelectorAll('[data-bid]').forEach((b) => (b.onclick = () => send({ type: 'bid', amount: Number(b.dataset.bid) })));
      const bi = el.querySelector('[data-bidin]'); if (bi) bi.oninput = () => { ui.bid = bi.value; };
      el.querySelectorAll('[data-tp]').forEach((c) => (c.onchange = () => {
        const [side, i] = c.dataset.tp.split(':'); const set = ui.draft[side].props; c.checked ? set.add(Number(i)) : set.delete(Number(i)); again();
      }));
      el.querySelectorAll('[data-tc]').forEach((c) => (c.onchange = () => { ui.draft[c.dataset.tc].cards = c.checked ? 1 : 0; }));
      el.querySelectorAll('[data-tm]').forEach((c) => (c.oninput = () => { ui.draft[c.dataset.tm].money = Math.max(0, Number(c.value) || 0); }));

      el.querySelectorAll('[data-act]').forEach((b) => (b.onclick = () => {
        const a = b.dataset.act;
        if (a === 'lobby') return ctx.toLobby();
        if (a === 'bidcustom') { const n = Number((el.querySelector('[data-bidin]') || {}).value); ui.bid = ''; return n ? send({ type: 'bid', amount: n }) : Ada.toast('Gib ein Gebot ein'); }
        if (a === 'trade_close') { ui.draft = null; return again(); }
        if (a === 'trade_send') {
          const d = ui.draft; ui.draft = null;
          return send({ type: 'trade_propose', to: d.to, give: { money: d.give.money, props: [...d.give.props], cards: d.give.cards }, take: { money: d.take.money, props: [...d.take.props], cards: d.take.cards } });
        }
        if (a === 'surrender' && !confirm('Wirklich aufgeben? Du scheidest aus dem Spiel aus.')) return;
        const msg = { type: a };
        if (b.dataset.pos != null) msg.pos = Number(b.dataset.pos);
        send(msg);
      }));
    },
  };
})();
