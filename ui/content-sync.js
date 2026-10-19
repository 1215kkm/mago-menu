// content-sync.js — 관리자가 고친 숨은 메뉴 내용을 받아 Rust 에 넘긴다 (10-20 CEO "슈퍼관리자가 컨텐츠를 수정하거나 업데이트하면
// 마고 메뉴 프로그램을 가져다 쓰는 사람들도 업데이트돼야 해").
// 원본: GitHub 1215kkm/mago-menu 저장소 main 의 content/content.json. 켤 때 한 번, 그 뒤 6시간마다.
// 판(version)이 지금 것보다 높을 때만 Rust(content_set)가 받아 저장한다. 인터넷이 안 되면 조용히 넘어간다.

const URL = 'https://raw.githubusercontent.com/1215kkm/mago-menu/main/content/content.json';
const EVERY = 6 * 60 * 60 * 1000;

export function syncContent(onChange) {
  const once = async () => {
    try {
      const r = await fetch(URL, { cache: 'no-store' });
      if (!r.ok) return;
      const changed = await window.__TAURI__.core.invoke('content_set', { content: await r.json() });
      if (changed && onChange) onChange();
    } catch (e) {
      console.warn('[MAGO] 숨은 메뉴 내용을 못 받음', e);
    }
  };
  once();
  setInterval(once, EVERY);
}
