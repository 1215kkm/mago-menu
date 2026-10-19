// secret.js — MAGO 숨은 방 (secret.html#<숨은 메뉴 id>).
// 어두워짐 → 작은 불빛 → 쪽지. [자세히 보기] 는 쪽지가 빨려 들어가듯 작아진 뒤 브라우저로 연다. ✕ · Esc · 바깥 누르기로 닫는다.

const T = window.__TAURI__;
const invoke = (cmd, args) => T.core.invoke(cmd, args);
const id = decodeURIComponent(location.hash.slice(1));
const $ = (s) => document.getElementById(s);
let leaving = false;

function close() {
  if (leaving) return;
  leaving = true;
  document.body.classList.add('out');
  setTimeout(() => invoke('secret_close'), 260);
}

async function go(url) {
  if (leaving) return;
  leaving = true;
  // 빨려 들어가기 — 쪽지가 돌며 불빛 속으로 작아지고, 어둠이 한 번 더 짙어진다
  document.body.classList.add('suck');
  setTimeout(async () => {
    try { await invoke('open_link', { url }); } catch (e) { console.error(e); }
    invoke('secret_close');
  }, 820);
}

(async () => {
  try {
    const r = await invoke('mago_reward', { id });
    $('t').textContent = r.title;
    $('body').textContent = r.body;
    if (r.link) {
      $('go').textContent = r.linkLabel || '자세히 보기';
      $('go').onclick = () => go(r.link);
      $('go-row').hidden = false;
    }
  } catch (e) {
    $('t').textContent = '쪽지를 찾지 못했어요';
    $('body').textContent = String(e);
  }
  // 어둠이 먼저, 불빛, 그다음 쪽지 (규칙 35 — 나타날 때는 천천히)
  requestAnimationFrame(() => document.body.classList.add('in'));
  setTimeout(() => ($('go').hidden ? $('close') : $('go')).focus(), 900);
})();

$('close').onclick = close;
document.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
document.querySelector('.dark').addEventListener('click', close);
