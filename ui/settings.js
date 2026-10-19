// settings.js — 시프트 메뉴 설정 창.
// 컴뚝딱 app/modules/file-converter/quick.js 의 화면을 혼자 도는 페이지로 옮긴 것 (2026-10-20).
// 값은 Rust 가 들고 있다: qconv_settings_get / qconv_settings_set · autostart_get / autostart_set · qconv_preview.

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);

const KEYS = [{ id: 'shift', label: 'Shift' }, { id: 'ctrl', label: 'Ctrl' }, { id: 'alt', label: 'Alt' }];
const IMAGE = [{ id: 'png', label: 'PNG' }, { id: 'jpg', label: 'JPG' }, { id: 'webp', label: 'WEBP' }, { id: 'bmp', label: 'BMP' }, { id: 'gif', label: 'GIF' }];
const AUDIO = [{ id: 'mp3', label: 'MP3' }, { id: 'wav', label: 'WAV' }];
const PDF = [{ id: 'jpg-pages', label: 'JPG 쪽마다' }, { id: 'jpg-one', label: 'JPG 한 장으로' }, { id: 'png-pages', label: 'PNG 쪽마다' }, { id: 'png-one', label: 'PNG 한 장으로' }];

const S = { s: null, run: false, err: null, toast: null };
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
  }, [it.label])));
}

/** 여러 개 고르기 — 누를 때마다 켜고 끈다 */
function chips(items, on, toggle) {
  return el('div', { class: 'chips' }, items.map((it) => {
    const a = on.includes(it.id);
    return el('button', { type: 'button', class: `chip${a ? ' on' : ''}`, 'aria-pressed': a ? 'true' : 'false', onclick: () => toggle(it.id, !a) }, [a ? '✓ ' : '', it.label]);
  }));
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
  const toggle = (field) => (id, add) => set({ [field]: add ? s[field].concat(id) : s[field].filter((x) => x !== id) });

  const head = el('header', { class: 'head' }, [
    el('div', {}, [el('h1', {}, ['시프트 메뉴']), el('p', { class: 'muted' }, ['탐색기에서 ', el('kbd', {}, [keyName]), ' 를 누른 채 파일을 끌면 커서 둘레에 원형 메뉴가 떠요.'])]),
    el('span', { class: `state${s.enabled ? ' on' : ''}` }, [s.enabled ? '켜져 있어요' : '꺼져 있어요']),
  ]);

  const main = el('section', { class: 'card' }, [
    el('h2', {}, ['설정']),
    row('쓰기', seg([{ id: 'off', label: '끔' }, { id: 'on', label: '켬' }], s.enabled ? 'on' : 'off', (v) => set({ enabled: v === 'on' }, v === 'on' ? `켰어요 — 탐색기에서 ${keyName} 를 누른 채 파일을 끌어 보세요.` : '껐어요.'))),
    row('누를 키', seg(KEYS, s.key, (v) => set({ key: v }))),
    row('그림일 때', chips(IMAGE, s.image, toggle('image'))),
    row('소리일 때', chips(AUDIO, s.audio, toggle('audio'))),
    row('PDF 일 때', chips(PDF, s.pdf, toggle('pdf'))),
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
    el('p', { class: 'hint' }, ['메뉴 위에 놓은 파일은 복사로만 받아서 원본이 지워지거나 옮겨지지 않아요.']),
  ]);

  const how = el('section', { class: 'card' }, [
    el('h2', {}, ['이렇게 써요']),
    el('ol', { class: 'steps' }, [
      el('li', {}, ['탐색기(다운로드 폴더 등)에서 바꿀 파일을 고르세요. 여러 개도 돼요.']),
      el('li', {}, [el('kbd', {}, [keyName]), ' 를 누르고 있는 동안 원형 메뉴가 떠요. 끄는 도중에 눌러도 되고, 떼면 닫혀요.']),
      el('li', {}, ['원하는 칸 위에서 놓으면 ', el('b', {}, ['원본 옆에']), ' 바뀐 파일이 생겨요. 원본은 그대로예요.']),
      el('li', {}, ['그만두려면 가운데에 놓거나 Esc. 메뉴 밖 다른 폴더에 놓으면 윈도우가 평소처럼 옮겨요.']),
    ]),
    el('p', { class: 'hint' }, ['바꿀 수 있는 것: 그림 png · jpg · webp · bmp · gif 끼리, 소리 wav ↔ mp3, PDF → jpg · png (쪽마다 · 한 장으로).']),
  ]);

  const foot = el('footer', { class: 'foot' }, [
    el('span', { class: 'muted' }, ['창을 닫으면 트레이로 숨어요.']),
    el('button', { type: 'button', class: 'btn ghost', onclick: () => invoke('app_quit') }, ['프로그램 끝내기']),
  ]);

  const kids = [head, main, how, foot];
  if (S.toast) kids.push(el('div', { class: `toast${S.toast.bad ? ' bad' : ''}`, role: 'status' }, [S.toast.msg]));
  app.replaceChildren(...kids);
}

(async () => {
  try {
    S.s = await invoke('qconv_settings_get');
    S.run = (await invoke('autostart_get')).on;
  } catch (e) { S.err = `설정을 읽지 못했어요: ${e}`; }
  render();
})();
