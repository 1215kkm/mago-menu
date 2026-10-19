// qconv.js — MAGO 고리 + 쓰레기통 캐릭터 그림.
// 상태는 전부 Rust(src-tauri/src/ring.rs)가 보낸다:
//   qconv:view(칸 · 마우스가 올라간 칸 · 쓰레기통 위인지 · 못 찾은 숨은 메뉴 수) · qconv:progress · qconv:done
//   qconv:cur([dx, dy] 고리 가운데에서 커서까지) · qconv:eat(먹인 수) · qconv:ate(먹은 결과) · qconv:found(시각으로 찾은 숨은 메뉴)
// 칸 배치는 Rust hit() 와 같아야 한다 — 12시에서 시작해 시계 방향, 반지름 R_IN~R_OUT. 쓰레기통 자리는 Rust TRASH_X/Y 와 같아야 한다.

const SIZE = 560;
const C = SIZE / 2;
const R_IN = 54;
const R_OUT = 148;
const TRASH = { x: C + 162, y: C + 162, eat: 66 };
const GAP = 1.6; // 칸 사이 틈 (도)
const NS = 'http://www.w3.org/2000/svg';

const T = window.__TAURI__;
const root = document.getElementById('ring');
let v = { shown: false, busy: false, preview: false, plan: { kind: 'wait', title: '', note: '', segs: [] }, hover: null, trash: false, left: 0, total: 0 };
let prog = null; // { done, total, frac }
let done = null; // { made, failed, firstError, movedTo }
let trashMsg = null; // 쓰레기통이 일할 때 가운데 글 { title, note }

function svg(tag, attrs, kids) {
  const n = document.createElementNS(NS, tag);
  for (const [k, val] of Object.entries(attrs || {})) n.setAttribute(k, String(val));
  for (const k of kids || []) if (k) n.appendChild(k);
  return n;
}
function h(tag, cls, kids) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  for (const k of kids || []) if (k != null) n.append(k);
  return n;
}
const pt = (r, deg) => {
  const a = (deg * Math.PI) / 180;
  return [C + r * Math.cos(a), C + r * Math.sin(a)];
};
/** 고리 한 칸 — 화면 각도(0 = 3시, 시계 방향 +) */
function sector(a0, a1, r0, r1) {
  const big = a1 - a0 > 180 ? 1 : 0;
  const [x0, y0] = pt(r1, a0);
  const [x1, y1] = pt(r1, a1);
  const [x2, y2] = pt(r0, a1);
  const [x3, y3] = pt(r0, a0);
  return `M${x0} ${y0} A${r1} ${r1} 0 ${big} 1 ${x1} ${y1} L${x2} ${y2} A${r0} ${r0} 0 ${big} 0 ${x3} ${y3} Z`;
}
function text(x, y, cls, s) {
  const t = svg('text', { x, y, class: cls, 'text-anchor': 'middle', 'dominant-baseline': 'middle' });
  t.textContent = s;
  return t;
}

/** 띄어쓰기 기준으로 max 글자 안팎씩 lines 줄까지 */
function wrap(msg, max, lines) {
  const out = [];
  let cur = '';
  for (const w of String(msg).split(' ')) {
    if (cur && (cur + ' ' + w).length > max) { out.push(cur); cur = w; } else cur = cur ? cur + ' ' + w : w;
  }
  if (cur) out.push(cur);
  if (out.length > lines) { out.length = lines; out[lines - 1] = out[lines - 1].slice(0, max - 1) + '…'; }
  return out;
}

// ── 고리 ────────────────────────────────────────────────────────────────────
// 10-19 CEO "칸을 고를 때마다 너무 깜빡인다 — 해당 버튼만 살짝 깜빡이게": 고리는 칸이 바뀔 때(뜰 때)만 만들고,
// 그 뒤엔 칸의 색과 가운데 글만 고친다.
let ringEl = null;
let ringKey = '';
let segEls = [];
let hubEl = null;
let lastOn = null;

function buildRing(segs) {
  const n = segs.length;
  const s = svg('svg', { viewBox: `0 0 ${SIZE} ${SIZE}`, width: SIZE, height: SIZE, class: 'qc' });
  s.appendChild(svg('defs', {}, [svg('linearGradient', { id: 'g', x1: 0, y1: 0, x2: 1, y2: 1 }, [
    svg('stop', { offset: 0, 'stop-color': 'var(--primary)' }), svg('stop', { offset: 1, 'stop-color': 'var(--secondary)' }),
  ])]));
  // 바탕 원 — 가운데 원과 칸 사이 틈으로 뒤 화면이 비치지 않게 통째로 깐다
  s.appendChild(svg('circle', { cx: C, cy: C, r: R_OUT, class: 'track' }));
  segEls = [];
  if (n) {
    const step = 360 / n;
    segs.forEach((g, i) => {
      const a0 = -90 + i * step + GAP / 2;
      const a1 = -90 + (i + 1) * step - GAP / 2;
      const grp = svg('g', { class: 'seg' });
      grp.appendChild(svg('path', { d: sector(a0, a1, R_IN + 4, R_OUT) }));
      const [lx, ly] = pt((R_IN + R_OUT) / 2 + 4, (a0 + a1) / 2);
      grp.appendChild(text(lx, ly - (g.sub ? 8 : 0), 'lb', g.label));
      if (g.sub) grp.appendChild(text(lx, ly + 10, 'sb', g.sub));
      s.appendChild(grp);
      segEls.push(grp);
    });
  }
  hubEl = svg('g', {});
  s.appendChild(hubEl);
  lastOn = null;
  return s;
}

/** 가운데 맨 아래 작은 글 — 10-20 CEO "못 찾은 숨은 메뉴 갯수를 원형 정가운데에 작게" */
function hiddenLine() {
  if (!v.total) return null;
  return text(C, C + 31, 'hc', v.left ? `숨은 메뉴 ${v.left}개` : '숨은 메뉴 다 찾음');
}

function drawHub(segs) {
  hubEl.replaceChildren();
  const add = (n) => n && hubEl.appendChild(n);
  add(svg('circle', { cx: C, cy: C, r: R_IN - 4, class: `hub${done ? (done.failed && !done.made ? ' bad' : ' good') : ''}` }));
  if (done) {
    if (done.made) {
      add(svg('path', { d: `M${C - 14} ${C - 12} l10 10 l19 -20`, class: 'tick' }));
      add(text(C, C + 13, 'ht small', `${done.made}개 만듦`));
      if (done.failed) add(text(C, C + 27, 'hn', `${done.failed}개 실패`));
      else if (done.movedTo) add(text(C, C + 27, 'hn', '다른 폴더에 저장'));
    } else {
      // 가운데 원 안에 들어가게 9자씩 세 줄까지
      const msg = String(done.firstError || '바꾸지 못했어요');
      const lines = [];
      for (let i = 0; i < msg.length && lines.length < 3; i += 9) lines.push(msg.slice(i, i + 9));
      if (msg.length > 27) lines[2] = lines[2].slice(0, 8) + '…';
      add(text(C, C - 20, 'ht small', '못 바꿨어요'));
      lines.forEach((l, k) => add(text(C, C - 3 + k * 13, 'hn', l)));
    }
  } else if (trashMsg) {
    add(text(C, C - 10, 'ht', trashMsg.title));
    wrap(trashMsg.note, 9, 2).forEach((l, k) => add(text(C, C + 7 + k * 12, 'hn', l)));
    add(hiddenLine());
  } else if (v.busy || prog) {
    const f = prog ? prog.frac : 0;
    const r = R_IN - 12;
    const len = 2 * Math.PI * r;
    add(svg('circle', { cx: C, cy: C, r, class: 'pbg' }));
    add(svg('circle', { cx: C, cy: C, r, class: 'pfg', 'stroke-dasharray': `${len * f} ${len}`, transform: `rotate(-90 ${C} ${C})` }));
    add(text(C, C, 'ht', `${Math.round(f * 100)}%`));
  } else {
    const hov = v.hover != null && segs[v.hover];
    const title = v.trash ? '쓰레기통' : hov ? hov.label : v.plan.title || '';
    const note = v.trash ? '놓으면 휴지통으로' : hov ? '놓으면 바꿔요' : v.plan.note || '';
    add(text(C, C - 14, 'ht', title.length > 9 ? title.slice(0, 8) + '…' : title));
    // 가운데 원(지름 100px) 안에 들어가게 9자 안팎에서 줄을 바꾼다 — 띄어쓰기에서 끊는다
    wrap(note, 9, 2).forEach((l, k) => add(text(C, C + 3 + k * 12, 'hn', l)));
    add(hiddenLine());
  }
}

function render() {
  if (!v.shown) {
    root.replaceChildren();
    ringEl = null;
    ringKey = '';
    resetMago();
    return;
  }
  const segs = (v.plan && v.plan.segs) || [];
  const key = segs.map((g) => `${g.id}|${g.label}|${g.sub}`).join(',');
  if (!ringEl || key !== ringKey) {
    const pop = !ringEl; // 나타나기 효과는 고리가 처음 뜰 때 한 번만
    ringEl = buildRing(segs);
    ringKey = key;
    if (pop) {
      root.replaceChildren(h('div', 'stage pop', [ringEl, mago.el]));
      wake();
    } else {
      root.firstChild.replaceChild(ringEl, root.firstChild.querySelector('svg.qc'));
    }
  }
  const on = v.hover;
  segEls.forEach((grp, i) => {
    grp.classList.toggle('on', on === i);
    if (on !== i) grp.classList.remove('flash');
    grp.classList.toggle('dim', Boolean(v.busy || done || trashMsg) && on !== i);
  });
  // 새로 올라간 칸만 짧게 반짝 — 애니메이션을 다시 걸려면 클래스를 뗐다 붙인다
  if (on != null && on !== lastOn && segEls[on]) {
    const g = segEls[on];
    g.classList.remove('flash');
    void g.getBoundingClientRect();
    g.classList.add('flash');
  }
  lastOn = on;
  drawHub(segs);
}

// ── 쓰레기통 캐릭터 ─────────────────────────────────────────────────────────
// 그림은 ui/mago/*.webp (같은 바닥선 · 높이 240 → 화면에서 120). 두 장을 겹쳐 두고 바꿀 때 살짝 겹쳐 넘긴다(깜빡임 없이).
const POSES = ['idle', 'wait', 'wait_heart', 'feed_near', 'eating', 'chewing', 'satisfied', 'more_please', 'full', 'poop_start', 'poop_done', 'menu_open', 'crown', 'love', 'sleep'];
const FOODS = ['food_1', 'food_2', 'food_3', 'food_4'];
const src = (n) => `./mago/${n}.webp`;
for (const n of [...POSES, ...FOODS, 'fx_hearts', 'fx_spark']) { const i = new Image(); i.src = src(n); }

const mago = (() => {
  const a = h('img', 'pose on');
  const b = h('img', 'pose');
  a.alt = b.alt = '';
  a.src = src('idle');
  const breath = h('div', 'breath', [a, b]);
  const squash = h('div', 'squash', [breath]);
  const tilt = h('div', 'tilt', [squash]);
  const fx = h('div', 'fx');
  const bubble = h('div', 'bubble');
  const zzz = h('div', 'zzz', [h('span', '', ['z']), h('span', '', ['z']), h('span', '', ['Z'])]);
  const plus = h('div', 'plus', ['+']);
  const el = h('div', 'mago', [tilt, zzz, plus, fx, bubble]);
  el.style.left = `${TRASH.x - 85}px`;
  el.style.top = `${TRASH.y - 74}px`;
  return { el, imgs: [a, b], tilt, squash, fx, bubble, pose: 'idle' };
})();

let act = null; // null | 'eat' | 'after' | 'found'
let eatStart = 0;
let lastMove = 0;
let cur = null; // [dx, dy]
let timers = [];
const later = (ms, f) => timers.push(setTimeout(f, ms));

function setPose(name) {
  if (mago.pose === name) return;
  mago.pose = name;
  const [on, off] = mago.imgs[0].classList.contains('on') ? mago.imgs : [mago.imgs[1], mago.imgs[0]];
  off.src = src(name);
  const flip = () => { if (mago.pose === name) { off.classList.add('on'); on.classList.remove('on'); } };
  if (off.complete) flip(); else off.decode().then(flip, flip);
  mago.el.classList.toggle('sleeping', name === 'sleep');
  mago.el.classList.toggle('opened', name === 'menu_open');
}

function say(msg, sub, bad) {
  mago.bubble.replaceChildren(h('b', '', [msg]), sub ? h('span', '', [sub]) : null);
  mago.bubble.classList.toggle('bad', Boolean(bad));
  mago.bubble.classList.add('on');
}
const hush = () => mago.bubble.classList.remove('on');

/** 머리 위 효과 (하트 · 반짝) — 떠오르며 사라진다 */
function burst(name) {
  const i = h('img', `fx-${name}`);
  i.src = src(name);
  mago.fx.replaceChildren(i);
}

function resetMago() {
  timers.forEach(clearTimeout);
  timers = [];
  act = null;
  trashMsg = null;
  cur = null;
  hush();
  mago.fx.replaceChildren();
  mago.squash.classList.remove('chomp', 'shake');
  mago.tilt.style.transform = '';
  setPose('idle');
}

function wake() {
  lastMove = Date.now();
}

/** 커서까지 거리로 표정 — 멀면 숨쉬기, 다가오면 입을 벌리고, 위에 오면 덥석 */
function follow() {
  if (act || !cur) return;
  const dx = C + cur[0] - TRASH.x;
  const dy = C + cur[1] - TRASH.y;
  const d = Math.hypot(dx, dy);
  // 커서 쪽으로 살짝 기운다 (±7°)
  const lean = d < 260 ? Math.max(-7, Math.min(7, dx / 18)) : 0;
  mago.tilt.style.transform = `rotate(${lean}deg)`;
  if (d <= TRASH.eat) setPose('feed_near');
  else if (d < 120) setPose('wait_heart');
  else if (d < 200) {
    if (mago.pose !== 'wait') burst('fx_spark');
    setPose('wait');
  } else if (mago.pose !== 'sleep') setPose('idle');
}

// 가만히 4.5초 — 꾸벅꾸벅 (커서가 쓰레기통 가까이 있으면 안 잔다)
setInterval(() => {
  if (!v.shown || act || mago.pose !== 'idle') return;
  if (Date.now() - lastMove > 4500) setPose('sleep');
}, 500);

/** 끌던 파일이 음식이 되어 입으로 날아간다 */
function throwFood(n) {
  const stage = root.firstChild;
  if (!stage) return;
  const from = cur ? [C + cur[0], C + cur[1]] : [TRASH.x - 40, TRASH.y - 60];
  const mouth = [TRASH.x + 2, TRASH.y - 16];
  for (let k = 0; k < Math.min(n, 4); k++) {
    const f = h('img', 'food');
    f.src = src(FOODS[(Math.random() * FOODS.length) | 0]);
    f.style.left = `${from[0] - 22}px`;
    f.style.top = `${from[1] - 22}px`;
    stage.appendChild(f);
    const mx = mouth[0] - from[0];
    const my = mouth[1] - from[1];
    f.animate([
      { transform: 'translate(0,0) scale(1) rotate(0deg)', opacity: 1 },
      { transform: `translate(${mx * 0.5}px, ${my * 0.5 - 40}px) scale(.85) rotate(160deg)`, opacity: 1, offset: 0.55 },
      { transform: `translate(${mx}px, ${my}px) scale(.35) rotate(320deg)`, opacity: 0 },
    ], { duration: 380, delay: k * 110, easing: 'cubic-bezier(.3,.7,.4,1)', fill: 'both' }).onfinish = () => f.remove();
  }
}

function startEat(n) {
  act = 'eat';
  eatStart = Date.now();
  hush();
  mago.tilt.style.transform = '';
  trashMsg = { title: '냠냠', note: `${n}개 휴지통으로` };
  render();
  throwFood(n);
  setPose('feed_near');
  // 우적우적 — 먹기 · 씹기를 번갈아, 씹을 때마다 몸이 살짝 눌린다
  let k = 0;
  const chomp = () => {
    if (act !== 'eat') return;
    setPose(k % 2 ? 'chewing' : 'eating');
    mago.squash.classList.remove('chomp');
    void mago.squash.offsetWidth;
    mago.squash.classList.add('chomp');
    k++;
    later(170, chomp);
  };
  later(300, chomp);
}

function afterEat(r) {
  // 씹는 모습은 1.3초는 보여 준다
  const wait = Math.max(0, 1300 - (Date.now() - eatStart));
  later(wait, () => {
    act = 'after';
    mago.squash.classList.remove('chomp');
    v.left = r.left;
    v.total = r.total;
    if (r.error) {
      setPose('more_please');
      trashMsg = { title: '못 먹었어요', note: r.error };
      say('못 먹었어요', r.error, true);
      render();
      return;
    }
    if (!r.found) {
      setPose('satisfied');
      burst('fx_hearts');
      trashMsg = { title: '먹었어요', note: `${r.count}개 휴지통으로` };
      say(`냠! ${r.count}개 먹었어요`, '휴지통에서 되살릴 수 있어요');
      render();
      return;
    }
    // 숨은 메뉴 — 배부름 → 끙 → 응가 → 숨은 메뉴가 열린다
    setPose('full');
    trashMsg = { title: '배불러요', note: '어… 뭔가 나와요' };
    render();
    later(650, () => { setPose('poop_start'); mago.squash.classList.add('shake'); });
    later(1650, () => { mago.squash.classList.remove('shake'); setPose('poop_done'); });
    later(2450, () => {
      setPose('menu_open');
      burst('fx_spark');
      trashMsg = { title: '숨은 메뉴!', note: r.found };
      say('숨은 메뉴를 찾았어요', `「${r.found}」 — 곧 열려요`);
      render();
    });
  });
}

T.event.listen('qconv:view', (e) => {
  const nv = e.payload;
  if (!nv.shown || !v.shown) { prog = null; done = null; }
  v = nv;
  render();
});
T.event.listen('qconv:progress', (e) => { prog = e.payload; render(); });
T.event.listen('qconv:done', (e) => { done = e.payload; prog = null; render(); });
T.event.listen('qconv:cur', (e) => {
  cur = e.payload;
  wake();
  if (mago.pose === 'sleep' && !act) setPose('idle');
  follow();
});
T.event.listen('qconv:eat', (e) => startEat(e.payload || 1));
T.event.listen('qconv:ate', (e) => afterEat(e.payload));
T.event.listen('qconv:found', (e) => {
  // 시각으로 찾은 숨은 메뉴 (메뉴를 열자마자) — 왕관을 쓰고 알려 준다. 고리를 닫으면 쪽지 방이 열린다
  act = 'found';
  setPose('crown');
  burst('fx_spark');
  say('숨은 메뉴를 찾았어요', `「${e.payload}」 — 메뉴를 닫으면 열려요`);
  later(2200, () => { if (act === 'found') { act = null; hush(); follow(); } });
});
T.core.invoke('qconv_view').then((nv) => { if (nv) { v = nv; render(); } }).catch(() => {});
render();
