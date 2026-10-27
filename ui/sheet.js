// sheet.js — MAGO JPG 한 장 미리 보기 (sheet.html · Rust sheet.rs).
// 10-25 CEO "화질 떨어지면 안돼 … 완성파일 미리 보여줘 그중에 이미지들은 현재가 원본크기인걸 표시해두고 … 원본보다 커지면
// 화질이 떨어질수 있다 알려주고 그대로 저장할건지, 조절부터 할건지 선택하게 … 원본이 너무 커도 파일폭은 넘어가면 안돼".
// - 왼쪽(좁으면 위) 종이 = 저장될 JPG 그대로. 그림마다 「원본 크기 · 원본의 N%」 표시.
// - [크기 조절] 을 켜면 그림 아래 양쪽 모서리 동그라미를 끌어 폭을 바꾼다 (두 번 누르면 원본 크기). 파일 폭보다 크게는 안 된다.
// - 원본보다 큰 그림이 있으면 저장 전에 [그래도 저장] · [다시 조절] 을 묻는다.
// 10-28 CEO "모두 진행해" — PDF 하나로도 같은 창 (V.pdf). 칸마다 A4 쪽 하나, 그림 폭은 pt, 상한은 쪽 여백 안 (p.max).
// Rust: sheet_get · sheet_set(폭이 바뀔 때마다 — ✕ 로 닫으면 이대로 저장) · sheet_save · sheet_cancel · sheet_reveal · sheet_close
//       sheet:progress(저장 0~1) · sheet:new(새 미리 보기가 이 창에 왔다)

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const $ = (id) => document.getElementById(id);
const paper = $('paper');
const side = $('side');

let V = null; // sheet_get
let W = []; // 지금 그림 폭 (px)
let mode = 'look'; // look · adjust · confirm · saving · saved
let adjust = false; // [크기 조절] 켬
let saved = null; // sheet_save 결과
let savedW = null; // 저장한 때의 폭
let err = null;
let prog = 0;
const rows = []; // 칸마다 { box, tag }

function el(tag, attrs, kids) {
  const n = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on')) n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v === true ? '' : String(v));
  }
  for (const k of [].concat(kids || [])) if (k != null) n.append(k);
  return n;
}

const num = (n) => Math.round(n).toLocaleString('ko-KR');
/** 사람이 읽는 용량 (settings.js 와 같게) */
function size(b) {
  if (b < 1024 * 1024) return `${Math.max(1, Math.round(b / 1024))}KB`;
  return `${(b / 1024 / 1024).toFixed(1)}MB`;
}
const no = (p) => `${p.file + 1}`;

/** 그림 상태 — ok 원본 크기 · fit 파일 폭(PDF 는 쪽)에 맞춰 줄임 · small 원본보다 작게 · big 원본보다 크게 · pdf 문서 쪽 */
function state(k) {
  const p = V.parts[k];
  if (!p.image) return 'pdf';
  if (W[k] === p.ow) return 'ok';
  if (W[k] > p.ow) return 'big';
  return W[k] === p.max && p.ow > p.max ? 'fit' : 'small';
}
const PDF = () => Boolean(V && V.pdf);
const kindName = () => (PDF() ? 'PDF 하나로' : 'JPG 한 장');
/** 그림이 놓이는 바탕 폭 — JPG 는 파일 폭(px), PDF 는 그 쪽 폭(pt) */
const unit = (k) => (PDF() ? V.parts[k].pw : V.width);
const ratio = (k) => Math.round((W[k] / V.parts[k].ow) * 100);
const images = () => V.parts.map((p, k) => k).filter((k) => V.parts[k].image);
const bigs = () => images().filter((k) => state(k) === 'big');
const differs = (a) => images().some((k) => W[k] !== a[k]);
/** 처음(원본 크기)과 다른가 */
const changed = () => differs(V.parts.map((p) => p.w));

/** 다 그린 크기 — Rust Sheet::layout 과 같은 셈 (너무 길면 전체를 같은 비율로 줄인다) */
function layout() {
  const total = V.parts.reduce((s, p, k) => s + (W[k] * p.oh) / p.ow, 0);
  const g = Math.min(1, V.maxHeight / Math.max(total, 1), Math.sqrt(V.maxPixels / Math.max(V.width * total, 1)));
  return { w: Math.max(1, Math.round(V.width * g)), h: Math.round(total * g), g };
}

function tagText(k) {
  const p = V.parts[k];
  const name = p.name;
  if (!p.image) return { cls: 'pdf', main: p.pages > 1 ? `${name} · ${p.page}/${p.pages}쪽` : name, sub: '' };
  const st = state(k);
  const main = {
    ok: `${name} · 원본 크기`,
    fit: `${name} · ${PDF() ? '쪽' : '파일 폭'}에 맞춰 원본의 ${ratio(k)}%`,
    small: `${name} · 원본의 ${ratio(k)}%`,
    big: `${name} · 원본의 ${ratio(k)}% · 흐려질 수 있어요`,
  }[st];
  if (PDF()) return { cls: st, main, sub: `A4 쪽 폭의 ${Math.round((W[k] / p.pw) * 100)}% · 원본 ${num(p.ow)} × ${num(p.oh)}px` };
  const h = Math.round((W[k] * p.oh) / p.ow);
  return { cls: st, main, sub: `${num(W[k])} × ${num(h)}px · 원본 ${num(p.ow)} × ${num(p.oh)}px` };
}

// ── 종이 (저장될 모습) ──────────────────────────────────────────────────────

function build() {
  paper.replaceChildren();
  rows.length = 0;
  // 종이 폭: 창 안에서 넓게, 파일 폭(px)보다 크게는 보이지 않게 (작은 파일을 부풀려 보이지 않는다). PDF 는 쪽을 차례로 (A4 를 화면 크기 안팎으로)
  paper.classList.toggle('pdf', PDF());
  paper.style.maxWidth = PDF() ? '720px' : `${V.width}px`;
  V.parts.forEach((p, k) => {
    const img = p.thumb ? el('img', { src: p.thumb, alt: p.name, draggable: 'false' }) : el('span', { class: 'nothumb' }, ['미리 보기를 못 그렸어요']);
    const box = el('div', { class: 'box' }, [img]);
    // 창의 보안 규칙(CSP)이 style 속성 글을 막는다 — 스크립트로 넣는다
    box.style.aspectRatio = `${p.ow} / ${p.oh}`;
    // 첫 쪽(그림 · PDF 첫 쪽)에만 이름표 — PDF 의 다음 쪽들은 쪽 번호만
    const tag = el('div', { class: 'tag' });
    // PDF 하나로 — 그림은 A4 쪽 가운데. 쪽 모양은 그 쪽 크기(pt) 그대로
    const pg = PDF() ? el('div', { class: 'pg' }, [box]) : null;
    if (pg) pg.style.aspectRatio = `${p.pw} / ${p.ph}`;
    const row = el('figure', { class: `part${p.image ? ' img' : ''}` }, [pg || box, tag]);
    if (p.image) {
      for (const s of ['l', 'r']) box.append(el('span', { class: `grip ${s}`, 'aria-hidden': 'true', onpointerdown: (e) => grab(e, k) }));
      box.addEventListener('dblclick', () => adjust && setW(k, p.ow, true));
      // 파일 폭보다 넓은 그림은 원본 크기로 둘 수 없다 — [파일 폭에 맞추기] 만. 이름표(왼쪽 위)와 겹치지 않게 오른쪽 위에
      row.append(el('div', { class: 'acts' }, [
        p.ow <= p.max ? el('button', { type: 'button', class: 'mini', onclick: () => setW(k, p.ow, true) }, ['원본 크기']) : null,
        p.ow !== p.max ? el('button', { type: 'button', class: 'mini', onclick: () => setW(k, p.max, true) }, [PDF() ? '쪽에 맞추기' : '파일 폭에 맞추기']) : null,
      ]));
    }
    paper.append(row);
    rows.push({ row, box, tag, pg });
    update(k);
  });
}

/** 칸 하나만 다시 (규칙 52 — 바뀐 것만) */
function update(k) {
  const { row, box, tag } = rows[k];
  const p = V.parts[k];
  box.style.width = `${Math.min(100, (W[k] / unit(k)) * 100)}%`;
  const t = tagText(k);
  row.dataset.state = t.cls;
  let lab = tag.querySelector('.lab');
  if (!lab) tag.prepend((lab = el('span', { class: 'lab' })));
  lab.replaceChildren(el('i', { class: 'no' }, [no(p)]), el('span', { class: 'tx' }, [el('b', {}, [t.main]), t.sub ? el('small', {}, [t.sub]) : null]));
  // PDF 의 2쪽부터는 이름표를 작게
  tag.hidden = !p.image && p.page > 1;
  row.classList.toggle('more', !p.image && p.page > 1);
}

function zoomText() {
  if (PDF()) {
    $('zoom').textContent = '쪽마다 A4 로 저장돼요 · 화면에는 줄여 보여요';
    return;
  }
  const shown = paper.getBoundingClientRect().width;
  const z = Math.round((shown / V.width) * 100);
  $('zoom').textContent = z >= 100 ? '실제 크기로 보여요' : `화면에는 ${z}% 크기로 보여요 · 저장은 원래 크기로`;
}

// ── 크기 바꾸기 ───────────────────────────────────────────────────────────

let pushTimer = 0;
function setW(k, px, push) {
  const p = V.parts[k];
  const min = Math.max(16, Math.round(unit(k) * 0.03));
  let w = Math.max(min, Math.min(p.max, Math.round(px)));
  // 원본 크기 근처면 원본에 붙인다 (돌아가기 쉽게)
  if (Math.abs(w - p.ow) <= unit(k) * 0.015 && p.ow <= p.max) w = p.ow;
  if (W[k] === w) return;
  W[k] = w;
  update(k);
  if (mode === 'confirm' || mode === 'saved') mode = 'adjust';
  renderSide();
  if (push) {
    clearTimeout(pushTimer);
    pushTimer = setTimeout(() => invoke('sheet_set', { widths: W }).catch(() => {}), 120);
  }
}

function grab(e, k) {
  if (!adjust || mode === 'saving') return;
  e.preventDefault();
  const grip = e.currentTarget;
  try { grip.setPointerCapture(e.pointerId); } catch {}
  rows[k].row.classList.add('dragging');
  const move = (ev) => {
    const r = (rows[k].pg || paper).getBoundingClientRect();
    const cx = r.left + r.width / 2;
    setW(k, ((Math.abs(ev.clientX - cx) * 2) / r.width) * unit(k), false);
  };
  const up = () => {
    grip.removeEventListener('pointermove', move);
    grip.removeEventListener('pointerup', up);
    grip.removeEventListener('pointercancel', up);
    rows[k].row.classList.remove('dragging');
    invoke('sheet_set', { widths: W }).catch(() => {});
  };
  grip.addEventListener('pointermove', move);
  grip.addEventListener('pointerup', up);
  grip.addEventListener('pointercancel', up);
}

function setAdjust(on) {
  adjust = on;
  document.body.classList.toggle('adjusting', on);
  if (mode !== 'saving') mode = on ? 'adjust' : saved && !differs(savedW) ? 'saved' : 'look';
  renderSide();
}

function reset() {
  V.parts.forEach((p, k) => (W[k] = p.w));
  rows.forEach((_, k) => update(k));
  invoke('sheet_set', { widths: W }).catch(() => {});
  if (mode === 'confirm') mode = 'adjust';
  renderSide();
}

// ── 저장 ─────────────────────────────────────────────────────────────────

async function save(force) {
  if (mode === 'saving') return;
  if (!force && bigs().length) {
    mode = 'confirm';
    renderSide();
    return;
  }
  mode = 'saving';
  prog = 0;
  err = null;
  renderSide();
  try {
    saved = await invoke('sheet_save', { widths: W });
    V.saved = saved.file;
    savedW = W.slice();
    mode = 'saved';
    setAdjust(false);
  } catch (e) {
    err = String(e);
    mode = adjust ? 'adjust' : 'look';
  }
  renderSide();
}

// ── 오른쪽 판 ────────────────────────────────────────────────────────────

function btn(label, cls, onclick, extra) {
  return el('button', { type: 'button', class: `btn ${cls || ''}`, onclick, disabled: mode === 'saving', ...extra }, [label]);
}

function facts() {
  const L = layout();
  const imgs = images();
  const pages = V.parts.length - imgs.length;
  const what = [imgs.length ? `그림 ${imgs.length}장` : null, pages ? `문서 ${pages}쪽` : null].filter(Boolean).join(' · ');
  return el('dl', { class: 'facts' }, [
    el('dt', {}, ['파일 이름']), el('dd', {}, [V.name]),
    el('dt', {}, ['크기']), el('dd', {}, [PDF() ? `${V.parts.length}쪽 · 그림은 A4` : `${num(L.w)} × ${num(L.h)}px`]),
    el('dt', {}, ['들어간 것']), el('dd', {}, [what]),
    el('dt', {}, ['저장할 곳']), el('dd', { class: 'path' }, [V.dir]),
  ]);
}

/** 그림 크기 한 줄 요약 + 알릴 것 */
function notes() {
  const out = [];
  const imgs = images();
  if (imgs.length) {
    const c = { ok: 0, fit: 0, small: 0, big: 0 };
    imgs.forEach((k) => c[state(k)]++);
    if (c.ok === imgs.length) out.push(el('p', { class: 'note ok' }, [`그림 ${imgs.length}장 모두 원본 크기예요. 화질 그대로예요.`]));
    else {
      const parts = [];
      if (c.ok) parts.push(`원본 크기 ${c.ok}장`);
      if (c.fit) parts.push(PDF() ? `쪽보다 커서 쪽에 맞춰 줄인 것 ${c.fit}장` : `파일 폭보다 넓어 폭에 맞춰 줄인 것 ${c.fit}장`);
      if (c.small) parts.push(`원본보다 작게 ${c.small}장`);
      if (c.big) parts.push(`원본보다 크게 ${c.big}장`);
      out.push(el('p', { class: `note ${c.big ? 'warn' : 'ok'}` }, [parts.join(' · ')]));
    }
    if (c.big && mode !== 'confirm') out.push(el('p', { class: 'note warn' }, ['원본보다 키운 그림은 흐려질 수 있어요.']));
  }
  const L = layout();
  if (!PDF() && L.g < 0.99) out.push(el('p', { class: 'note warn' }, [`너무 길어서 전체를 ${Math.round(L.g * 100)}% 로 줄여 저장해요.`]));
  if (V.failed) out.push(el('p', { class: 'note bad' }, [`${V.failed}개는 넣지 못했어요 — ${V.firstError}`]));
  if (V.textOnly) out.push(el('p', { class: 'note' }, [`문서 ${V.textOnly}개는 이 PC 에 그 프로그램이 없어 글만 옮겼어요.`]));
  return out;
}

function renderSide() {
  if (!V) return;
  const kids = [el('h1', {}, [`${kindName()} 미리 보기`]), facts(), ...notes()];
  const canAdjust = images().length > 0;
  if (err) kids.push(el('p', { class: 'note bad' }, [`저장하지 못했어요 — ${err}`]));

  if (mode === 'confirm') {
    const list = bigs().map((k) => `${no(V.parts[k])}번 ${V.parts[k].name} (원본의 ${ratio(k)}%)`);
    kids.push(el('div', { class: 'ask' }, [
      el('b', {}, ['원본보다 크게 키운 그림이 있어요']),
      el('p', {}, [list.join(', ')]),
      el('p', {}, ['원본보다 크게 저장하면 흐려질 수 있어요.']),
      el('div', { class: 'go' }, [btn('그래도 저장', 'primary', () => save(true)), btn('다시 조절', '', () => { mode = 'adjust'; renderSide(); })]),
    ]));
  } else if (mode === 'saving') {
    kids.push(el('div', { class: 'bar' }, [el('i', { style: `width:${Math.round(prog * 100)}%` })]), el('p', { class: 'note' }, [`저장하는 중… ${Math.round(prog * 100)}%`]));
  } else if (mode === 'saved') {
    kids.push(el('div', { class: 'done' }, [
      el('b', {}, ['저장했어요']),
      el('p', {}, [`${saved.file.split(/[\\/]/).pop()} · ${size(saved.bytes)} · ${PDF() ? `${saved.pages}쪽` : `${num(saved.w)} × ${num(saved.h)}px`}`]),
      el('div', { class: 'go' }, [btn('폴더에서 보기', 'primary', () => invoke('sheet_reveal').catch((e) => { err = String(e); renderSide(); })), btn('닫기', '', () => invoke('sheet_close'))]),
    ]));
    if (canAdjust) kids.push(el('p', { class: 'hint' }, ['크기를 바꿔 다시 저장하면 이 파일을 고쳐요.']), seg());
  } else {
    // look · adjust
    const main = V.saved ? '이 크기로 다시 저장' : changed() ? '이 크기로 저장' : '그대로 저장';
    if (canAdjust) {
      kids.push(seg());
      if (adjust) kids.push(el('p', { class: 'hint' }, [`그림 아래 양쪽 모서리의 동그라미를 끌어 크기를 바꿔요. 그림을 두 번 누르면 원본 크기로. ${PDF() ? '쪽 여백(1cm) 밖으로는' : `파일 폭(${num(V.width)}px)보다`} 크게는 안 돼요.`]));
    }
    const go = [btn(main, 'primary big', () => save(false))];
    if (adjust && changed()) go.push(btn('처음대로', '', reset));
    kids.push(el('div', { class: 'go' }, go));
  }
  if (mode !== 'saved') {
    kids.push(el('div', { class: 'foot' }, [
      el('span', {}, [V.saved ? '✕ 로 닫으면 저장한 파일은 그대로예요.' : '✕ 로 닫으면 보이는 그대로 저장돼요.']),
      btn(V.saved ? '저장한 파일은 두고 닫기' : '저장 안 함', 'ghost', () => (V.saved ? invoke('sheet_close') : invoke('sheet_cancel'))),
    ]));
  }
  side.replaceChildren(...kids);
}

/** [그대로 보기 | 크기 조절] — 두 칸, 켜진 쪽에 색 (규칙 40) */
function seg() {
  return el('div', { class: 'seg', role: 'radiogroup', 'aria-label': '그림 크기' }, [
    el('button', { type: 'button', role: 'radio', class: adjust ? null : 'on', 'aria-checked': String(!adjust), onclick: () => adjust && setAdjust(false), disabled: mode === 'saving' }, ['지금 크기로 보기']),
    el('button', { type: 'button', role: 'radio', class: adjust ? 'on' : null, 'aria-checked': String(adjust), onclick: () => !adjust && setAdjust(true), disabled: mode === 'saving' }, ['크기 조절']),
  ]);
}

// ── 시작 ─────────────────────────────────────────────────────────────────

async function load() {
  try {
    V = await invoke('sheet_get');
  } catch (e) {
    side.replaceChildren(el('h1', {}, ['미리 보기']), el('p', { class: 'note bad' }, [String(e)]), el('div', { class: 'go' }, [el('button', { type: 'button', class: 'btn', onclick: () => invoke('sheet_close') }, ['닫기'])]));
    return;
  }
  W = V.parts.map((p) => p.w);
  saved = null;
  savedW = null;
  err = null;
  mode = 'look';
  adjust = false;
  document.body.classList.remove('adjusting');
  document.title = `MAGO — ${kindName()} 미리 보기`;
  build();
  renderSide();
  requestAnimationFrame(zoomText);
  $('stage').scrollTop = 0;
}

T.event.listen('sheet:progress', (e) => {
  prog = Math.max(prog, Number(e.payload) || 0);
  if (mode === 'saving') {
    const bar = side.querySelector('.bar i');
    if (bar) bar.style.width = `${Math.round(prog * 100)}%`;
    const t = side.querySelector('.bar + .note');
    if (t) t.textContent = `저장하는 중… ${Math.round(prog * 100)}%`;
  }
});
T.event.listen('sheet:new', load);
new ResizeObserver(() => V && zoomText()).observe(paper);
load();
