// qconv.js — 끌다가 바로 바꾸기 고리 그림 (10-19 CEO "파일 끌다가 시프트 누르면 나오는 메뉴").
// 상태는 전부 Rust(imagery/quick.rs)가 보낸다: qconv:view(칸·마우스가 올라간 칸) · qconv:progress · qconv:done.
// 칸 배치는 Rust hit() 와 같아야 한다 — 12시에서 시작해 시계 방향, 반지름 R_IN~R_OUT.

const SIZE = 380;
const C = SIZE / 2;
const R_IN = 62;
const R_OUT = 172;
const GAP = 1.6; // 칸 사이 틈 (도)
const NS = 'http://www.w3.org/2000/svg';

const T = window.__TAURI__;
const root = document.getElementById('ring');
let v = { shown: false, busy: false, preview: false, plan: { kind: 'wait', title: '', note: '', segs: [] }, hover: null };
let prog = null; // { done, total, frac }
let done = null; // { made, failed, firstError, movedTo }

function svg(tag, attrs, kids) {
  const n = document.createElementNS(NS, tag);
  for (const [k, val] of Object.entries(attrs || {})) n.setAttribute(k, String(val));
  for (const k of kids || []) if (k) n.appendChild(k);
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

// 10-19 CEO "칸을 고를 때마다 너무 깜빡인다 — 해당 버튼만 살짝 깜빡이게": 예전엔 마우스가 칸을 옮길 때마다 고리를 통째로
// 새로 그려서 나타나기 효과까지 다시 돌았다. 이제 고리는 칸이 바뀔 때(뜰 때)만 만들고, 그 뒤엔 칸의 색과 가운데 글만 고친다.
let ringEl = null; // 지금 그려 둔 고리 <svg>
let ringKey = ''; // 어떤 칸들로 만든 고리인지
let segEls = [];
let hubEl = null; // 가운데 원 + 글을 담는 <g>
let lastOn = null;

function buildRing(segs, pop) {
  const n = segs.length;
  const s = svg('svg', { viewBox: `0 0 ${SIZE} ${SIZE}`, width: SIZE, height: SIZE, class: `qc${pop ? ' pop' : ''}` });
  s.appendChild(svg('defs', {}, [svg('linearGradient', { id: 'g', x1: 0, y1: 0, x2: 1, y2: 1 }, [
    svg('stop', { offset: 0, 'stop-color': 'var(--primary)' }), svg('stop', { offset: 1, 'stop-color': 'var(--secondary)' }),
  ])]));
  // 바탕 원 — 가운데 원과 칸 사이 틈으로 뒤 화면이 비치지 않게 통째로 깐다 (칸이 없을 때도 모양은 보인다)
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
      if (g.sub) grp.appendChild(text(lx, ly + 11, 'sb', g.sub));
      s.appendChild(grp);
      segEls.push(grp);
    });
  }
  hubEl = svg('g', {});
  s.appendChild(hubEl);
  lastOn = null;
  return s;
}

function drawHub(segs) {
  hubEl.replaceChildren();
  const add = (n) => hubEl.appendChild(n);
  add(svg('circle', { cx: C, cy: C, r: R_IN - 4, class: `hub${done ? (done.failed && !done.made ? ' bad' : ' good') : ''}` }));
  if (done) {
    if (done.made) {
      add(svg('path', { d: `M${C - 16} ${C - 12} l11 11 l21 -22`, class: 'tick' }));
      add(text(C, C + 16, 'ht small', `${done.made}개 만듦`));
      if (done.failed) add(text(C, C + 31, 'hn', `${done.failed}개 실패`));
      else if (done.movedTo) add(text(C, C + 31, 'hn', '다른 폴더에 저장'));
    } else {
      // 가운데 원 안에 들어가게 11자씩 세 줄까지
      const msg = String(done.firstError || '바꾸지 못했어요');
      const lines = [];
      for (let i = 0; i < msg.length && lines.length < 3; i += 11) lines.push(msg.slice(i, i + 11));
      if (msg.length > 33) lines[2] = lines[2].slice(0, 10) + '…';
      add(text(C, C - 22, 'ht small', '못 바꿨어요'));
      lines.forEach((l, k) => add(text(C, C - 2 + k * 14, 'hn', l)));
    }
  } else if (v.busy || prog) {
    const f = prog ? prog.frac : 0;
    const r = R_IN - 12;
    const len = 2 * Math.PI * r;
    add(svg('circle', { cx: C, cy: C, r, class: 'pbg' }));
    add(svg('circle', { cx: C, cy: C, r, class: 'pfg', 'stroke-dasharray': `${len * f} ${len}`, transform: `rotate(-90 ${C} ${C})` }));
    add(text(C, C, 'ht', `${Math.round(f * 100)}%`));
  } else {
    const hov = v.hover != null && segs[v.hover];
    add(text(C, C - 10, 'ht', hov ? `${hov.label}${hov.sub ? ' ' + hov.sub : ''}` : v.plan.title || ''));
    // 가운데 원(지름 116px) 안에 들어가게 10자 안팎에서 줄을 바꾼다 — 띄어쓰기에서 끊는다
    wrap(hov ? '놓으면 바꿔요' : v.plan.note || '', 10, 2).forEach((l, k) => add(text(C, C + 10 + k * 13, 'hn', l)));
  }
}

function render() {
  if (!v.shown) {
    root.replaceChildren();
    ringEl = null;
    ringKey = '';
    return;
  }
  const segs = (v.plan && v.plan.segs) || [];
  const key = segs.map((g) => `${g.id}|${g.label}|${g.sub}`).join(',');
  if (!ringEl || key !== ringKey) {
    const pop = !ringEl; // 나타나기 효과는 고리가 처음 뜰 때 한 번만
    ringEl = buildRing(segs, pop);
    ringKey = key;
    root.replaceChildren(ringEl);
  }
  const on = v.hover;
  segEls.forEach((grp, i) => {
    grp.classList.toggle('on', on === i);
    if (on !== i) grp.classList.remove('flash');
    grp.classList.toggle('dim', Boolean(v.busy || done) && on !== i);
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

T.event.listen('qconv:view', (e) => {
  const nv = e.payload;
  if (!nv.shown || !v.shown) { prog = null; done = null; }
  v = nv;
  render();
});
T.event.listen('qconv:progress', (e) => { prog = e.payload; render(); });
T.event.listen('qconv:done', (e) => { done = e.payload; prog = null; render(); });
T.core.invoke('qconv_view').then((nv) => { if (nv) { v = nv; render(); } }).catch(() => {});
render();
