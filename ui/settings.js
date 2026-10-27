// settings.js — MAGO 설정 창.
// 컴뚝딱 app/modules/file-converter/quick.js 의 화면을 혼자 도는 페이지로 옮긴 것 (2026-10-20).
// 값은 Rust 가 들고 있다: qconv_settings_get / qconv_settings_set · autostart_get / autostart_set · qconv_preview
// · mago_state(먹인 횟수 · 찾은 숨은 메뉴) · secret_show([다시 보기]).
// 이 창은 숨어 있어도 늘 떠 있어서, 관리자가 고친 숨은 메뉴 내용(content.json)도 여기서 받는다 (content-sync.js).
import { syncContent } from './content-sync.js';

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);

const KEYS = [{ id: 'shift', label: 'Shift' }, { id: 'ctrl', label: 'Ctrl' }, { id: 'alt', label: 'Alt' }];
const IMAGE = [{ id: 'png', label: 'PNG' }, { id: 'jpg', label: 'JPG' }, { id: 'webp', label: 'WEBP' }, { id: 'bmp', label: 'BMP' }, { id: 'gif', label: 'GIF (움직이는 그림)' },
  { id: 'square', label: '정사각 (SNS 1080)' }, { id: 'small', label: '1MB 이하' }, { id: 'cutout', label: '누끼 (배경 지우기)' }];
// 글(TXT) → 문서 (Rust ring::TEXT_OPTS 와 같게)
const TEXT = [{ id: 'pdf', label: 'PDF' }, { id: 'hwp', label: '한글 (HWPX)' }, { id: 'word', label: 'Word' }, { id: 'excel', label: '엑셀 (탭마다 칸)' }];
const AUDIO = [{ id: 'mp3', label: 'MP3' }, { id: 'wav', label: 'WAV' }];
// 움직이는 GIF 한 장을 보여 주는 시간 (Rust anim::SPEEDS 와 같아야 한다)
const GIF_SPEEDS = [{ id: 60, label: '빠르게 0.06초' }, { id: 100, label: '보통 0.1초' }, { id: 200, label: '느리게 0.2초' }, { id: 500, label: '아주 느리게 0.5초' }];
// 고리 옆 캐릭터 (Rust ring::CHARACTERS · qconv.js CHARS 와 같게) — 버튼에 그 캐릭터 그림을 같이 보인다
const CHARS = [{ id: 'cat', label: '검은 고양이', img: './mago/cat/idle.webp', eats: '고양이가' }, { id: 'trash', label: '쓰레기통', img: './mago/idle.webp', eats: '쓰레기통이' }];
const PDF = [{ id: 'jpg-pages', label: 'JPG 쪽마다' }, { id: 'jpg-one', label: 'JPG 한 장으로' }, { id: 'png-pages', label: 'PNG 쪽마다' }, { id: 'png-one', label: 'PNG 한 장으로' }];
// 여러 종류를 같이 끌었을 때 · Word · 엑셀 · 한글 (Rust ring::MIX_OPTS = mix::OPTS 와 같게)
const MIX = [{ id: 'mix-pdf', label: 'PDF 하나로' }, { id: 'mix-pdfs', label: 'PDF 파일마다' }, { id: 'mix-jpg', label: 'JPG 한 장으로' }, { id: 'mix-jpgs', label: 'JPG 쪽마다' }];

const S = { s: null, run: false, err: null, toast: null, m: null };
const app = document.getElementById('app');

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

/** 두 칸 이상 중 하나 고르기 — 켜진 칸에 색 */
function seg(items, cur, pick) {
  return el('div', { class: 'seg', role: 'radiogroup' }, items.map((it) => el('button', {
    type: 'button', class: it.id === cur ? 'on' : null, role: 'radio', 'aria-checked': it.id === cur ? 'true' : 'false', onclick: () => it.id !== cur && pick(it.id),
  }, [].concat(it.label))));
}

/** 여러 개 고르기 — 누를 때마다 켜고 끈다 */
function chips(items, on, toggle) {
  return el('div', { class: 'chips' }, items.map((it) => {
    const a = on.includes(it.id);
    return el('button', { type: 'button', class: `chip${a ? ' on' : ''}`, 'aria-pressed': a ? 'true' : 'false', onclick: () => toggle(it.id, !a) }, [a ? '✓ ' : '', it.label]);
  }));
}

/** 사람이 읽는 용량 — 340KB · 12.3MB · 1.2GB (qconv.js 와 같게) */
function size(b) {
  if (b < 1024 * 1024) return b ? `${Math.max(1, Math.round(b / 1024))}KB` : '0MB';
  if (b < 1024 ** 3) return `${(b / 1024 / 1024).toFixed(b < 100 * 1024 * 1024 ? 1 : 0)}MB`;
  return `${(b / 1024 ** 3).toFixed(1)}GB`;
}

const row = (label, node) => el('div', { class: 'row' }, [el('span', { class: 'lab' }, [label]), node]);

function toast(msg, bad) {
  S.toast = { msg, bad };
  render();
  clearTimeout(toast.t);
  toast.t = setTimeout(() => { S.toast = null; render(); }, 2600);
}

async function set(patch, msg) {
  const before = S.s;
  S.s = { ...S.s, ...patch }; // 바로 바뀐 것처럼 보이고, 실패하면 되돌린다
  render();
  try {
    S.s = await invoke('qconv_settings_set', { patch });
    if (msg) toast(msg);
  } catch (e) {
    S.s = before;
    toast(String(e), true);
  }
  render();
}

function render() {
  const s = S.s;
  if (!s) {
    app.replaceChildren(el('p', { class: 'muted' }, [S.err || '설정을 읽는 중…']));
    return;
  }
  const keyName = (KEYS.find((k) => k.id === s.key) || KEYS[0]).label;
  const who = CHARS.find((c) => c.id === s.character) || CHARS[0];
  const toggle = (field) => (id, add) => set({ [field]: add ? s[field].concat(id) : s[field].filter((x) => x !== id) });

  const head = el('header', { class: 'head' }, [
    el('div', {}, [el('h1', {}, ['MAGO']), el('p', { class: 'muted' }, ['탐색기에서 ', el('kbd', {}, [keyName]), ' 를 누른 채 파일을 끌면 커서 둘레에 원형 메뉴가 떠요.'])]),
    el('span', { class: `state${s.enabled ? ' on' : ''}` }, [s.enabled ? '켜져 있어요' : '꺼져 있어요']),
  ]);

  const main = el('section', { class: 'card' }, [
    el('h2', {}, ['설정']),
    row('쓰기', seg([{ id: 'off', label: '끔' }, { id: 'on', label: '켬' }], s.enabled ? 'on' : 'off', (v) => set({ enabled: v === 'on' }, v === 'on' ? `켰어요 — 탐색기에서 ${keyName} 를 누른 채 파일을 끌어 보세요.` : '껐어요.'))),
    row('누를 키', seg(KEYS, s.key, (v) => set({ key: v }))),
    row('캐릭터', seg(CHARS.map((c) => ({ id: c.id, label: [el('img', { class: 'who', src: c.img, alt: '' }), c.label] })), who.id,
      (v) => set({ character: v }, `다음에 메뉴를 열면 ${CHARS.find((c) => c.id === v).eats} 기다려요.`))),
    row('그림일 때', chips(IMAGE, s.image, toggle('image'))),
    s.image.includes('gif') ? row('GIF 한 장', seg(GIF_SPEEDS, s.gifMs, (v) => set({ gifMs: v }))) : null,
    row('소리일 때', chips(AUDIO, s.audio, toggle('audio'))),
    row('PDF 일 때', chips(PDF, s.pdf, toggle('pdf'))),
    row('글(TXT)일 때', chips(TEXT, s.text || [], toggle('text'))),
    row('여러 종류·문서', chips(MIX, s.mix || [], toggle('mix'))),
    row('윈도우 켤 때', seg([{ id: 'off', label: '같이 안 켜기' }, { id: 'on', label: '트레이에 같이 켜기' }], S.run ? 'on' : 'off', async (v) => {
      const want = v === 'on';
      S.run = want; render();
      try {
        S.run = (await invoke('autostart_set', { on: want })).on;
        toast(want ? '윈도우를 켜면 트레이에 같이 켜져요.' : '윈도우를 켤 때 같이 켜지 않아요.');
      } catch (e) { S.run = !want; toast(String(e), true); }
      render();
    })),
    el('div', { class: 'actions' }, [el('button', { type: 'button', class: 'btn', onclick: () => invoke('qconv_preview').catch((e) => toast(String(e), true)) }, ['원형 메뉴 미리 보기'])]),
    el('p', { class: 'hint' }, [`이 프로그램이 켜져 있을 때만 돼요 (창을 닫아도 트레이에 있으면 돼요). JPG 는 화질 ${s.quality}, PDF 는 ${s.dpi}dpi 로 만들어요.`]),
    el('p', { class: 'hint' }, [`메뉴 위에 놓은 파일은 복사로만 받아서 원본이 지워지거나 옮겨지지 않아요. ${who.label}에게 먹인 파일만 윈도우 휴지통으로 가요.`]),
  ]);

  const m = S.m;
  const secret = m && el('section', { class: 'card' }, [
    el('h2', {}, ['숨은 메뉴']),
    el('p', { class: 'muted' }, [`찾은 것 ${m.found.length} / ${m.total} · ${who.eats} 먹은 파일 ${m.files || 0}개 · ${size(m.bytes || 0)} (${m.feeds}번)`]),
    m.found.length
      ? el('ul', { class: 'found' }, m.found.map((f) => el('li', {}, [
        el('span', {}, [f.name]),
        el('button', { type: 'button', class: 'btn', onclick: () => invoke('secret_show', { id: f.id }).catch((e) => toast(String(e), true)) }, ['다시 보기']),
      ])))
      : el('p', { class: 'hint' }, [`아직 하나도 못 찾았어요. 원형 메뉴 오른쪽 아래 ${who.label}에게 파일을 먹여 보세요.`]),
    m.left ? el('p', { class: 'hint' }, [`못 찾은 숨은 메뉴가 ${m.left}개 더 있어요.${(m.hints || []).length ? ' 힌트:' : ''}`]) : null,
    // 10-25 대표님 "숨은 미션 힌트를 설명에" — 못 찾은 것만, 찾으면 사라진다
    m.left && (m.hints || []).length ? el('ul', { class: 'clues' }, m.hints.map((t) => el('li', {}, [t]))) : null,
  ]);

  const how = el('section', { class: 'card' }, [
    el('h2', {}, ['이렇게 써요']),
    el('ol', { class: 'steps' }, [
      el('li', {}, ['탐색기(다운로드 폴더 등)에서 바꿀 파일을 고르세요. 여러 개도 돼요.']),
      el('li', {}, [el('kbd', {}, [keyName]), ' 를 누르고 있는 동안 원형 메뉴가 떠요. 끄는 도중에 눌러도 되고, 떼면 닫혀요.']),
      el('li', {}, ['원하는 칸 위에서 놓으면 ', el('b', {}, ['원본 옆에']), ' 바뀐 파일이 생겨요. 원본은 그대로예요.']),
      el('li', {}, ['오른쪽 아래 ', el('b', {}, [who.label]), '에게 놓으면 냠냠 먹고 윈도우 휴지통으로 보내요. 휴지통에서 되살릴 수 있어요.']),
      el('li', {}, ['그만두려면 가운데에 놓거나 Esc. 메뉴 밖 다른 폴더에 놓으면 윈도우가 평소처럼 옮겨요.']),
    ]),
    el('p', { class: 'hint' }, ['여러 종류를 같이: 그림 · PDF · 글(TXT) · Word · 엑셀 · 한글을 한꺼번에 끌면 메뉴 위에 번호 목록이 떠요. 그 순서대로 PDF 하나 · JPG 한 장으로 잇거나, 파일마다 PDF · 쪽마다 JPG 로 만들어요. Word · 엑셀 · 한글 파일은 이 PC 에 깔린 그 프로그램을 화면에 띄우지 않고 써요 (없으면 docx · xlsx · hwpx 는 글만 옮겨요).']),
    el('p', { class: 'hint' }, ['GIF 칸: 여러 장을 같이 끌어 놓으면 이름 순서(1, 2, … 10)로 이어 움직이는 GIF 하나를 만들어요. 동작이 칸칸이 그려진 한 장을 놓으면 칸을 나눠 움직여요. 긴 변은 800px 까지 줄여요.']),
    el('p', { class: 'hint' }, ['바꿀 수 있는 것: 그림 png · jpg · webp · bmp · gif 끼리, 정사각(인스타 1080×1080, 가운데를 잘라요), 1MB 이하 JPG(카톡·메일용), 누끼(배경을 지운 투명 PNG — 처음 한 번 누끼 도구 179MB 를 받고, 사진은 PC 밖으로 나가지 않아요), 소리 wav ↔ mp3, PDF → jpg · png (쪽마다 · 한 장으로), 글(TXT) → PDF · 한글 · Word · 엑셀(탭마다 칸). 브라우저에서 그림 · 링크를 끌면 다운로드 폴더에 저장 · 바로가기 · 카톡으로(복사해 두고 카카오톡을 열어요).']),
  ]);

  const foot = el('footer', { class: 'foot' }, [
    el('span', { class: 'muted' }, ['창을 닫으면 트레이로 숨어요.']),
    el('button', { type: 'button', class: 'btn ghost', onclick: () => invoke('app_quit') }, ['프로그램 끝내기']),
  ]);

  const kids = [head, main, secret, how, foot].filter(Boolean);
  if (S.toast) kids.push(el('div', { class: `toast${S.toast.bad ? ' bad' : ''}`, role: 'status' }, [S.toast.msg]));
  app.replaceChildren(...kids);
}

(async () => {
  try {
    S.s = await invoke('qconv_settings_get');
    S.run = (await invoke('autostart_get')).on;
  } catch (e) { S.err = `설정을 읽지 못했어요: ${e}`; }
  render();
  loadMago();
})();

async function loadMago() {
  try { S.m = await invoke('mago_state'); render(); } catch (e) { console.error(e); }
}
// 창을 다시 열 때(트레이) 숨은 메뉴 수가 바뀌었을 수 있다
document.addEventListener('visibilitychange', () => { if (!document.hidden) loadMago(); });
syncContent(() => loadMago());
