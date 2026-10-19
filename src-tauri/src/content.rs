//! **숨은 메뉴 · 관리자가 고치는 내용** (2026-10-20 CEO "못 찾은 숨은 메뉴 갯수를 원형 정가운데에 작게",
//! "슈퍼관리자가 컨텐츠를 수정하거나 업데이트하면 마고메뉴 프로그램을 가져다 쓰는 사람들도 업데이트돼야 해").
//!
//! - 내용(숨은 메뉴 목록 · 여는 조건 · 쪽지 글 · 링크)은 저장소의 `content/content.json`.
//!   설정 창(늘 숨어서 떠 있다 — ui/content-sync.js)이 켜질 때와 6시간마다 GitHub 에서 받아 `content_set` 으로 넘긴다 → 여기서 검사하고 저장해 둔다.
//!   인터넷이 안 되면 마지막으로 받은 것, 그것도 없으면 exe 에 들어 있는 사본을 쓴다.
//! - 사람마다 다른 것(먹인 횟수 · 찾은 숨은 메뉴)은 `%APPDATA%\com.mago.app\prefs.json` 의 `mago`.
//! - 링크는 **내용 파일에 적힌 https 주소만** 연다 — 화면이 아무 주소나 열게 하지 않는다.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

const BUNDLED: &str = include_str!("../../content/content.json");
const CACHE_KEY: &str = "contentCache";
const PROGRESS_KEY: &str = "mago";
const SECRET: &str = "secret";

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub version: u32,
    pub hidden: Vec<Hidden>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hidden {
    pub id: String,
    pub name: String,
    pub unlock: Unlock,
    pub reward: Reward,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Unlock {
    /// 쓰레기통에 먹인 횟수(놓은 횟수)가 count 이상
    FeedTotal { count: u32 },
    /// 한 번에 먹인 파일 수가 count 이상
    FeedBatch { count: u32 },
    /// 메뉴를 연 시각(시)이 from 이상 to 미만
    Hour { from: u32, to: u32 },
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default)]
    pub link_label: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Progress {
    pub feeds: u32,
    pub discovered: Vec<String>,
}

fn check(c: &Content) -> Result<(), String> {
    if c.hidden.len() > 200 {
        return Err("숨은 메뉴가 너무 많아요".into());
    }
    let mut ids = std::collections::HashSet::new();
    for h in &c.hidden {
        if h.id.is_empty() || !ids.insert(h.id.clone()) {
            return Err(format!("숨은 메뉴 id 가 비었거나 겹쳐요: {}", h.id));
        }
        if let Some(l) = &h.reward.link {
            if !l.starts_with("https://") {
                return Err(format!("링크는 https 만 돼요: {l}"));
            }
        }
        if let Unlock::Hour { from, to } = h.unlock {
            if from > 24 || to > 24 {
                return Err(format!("시각은 0~24 예요: {}", h.id));
            }
        }
    }
    Ok(())
}

static CONTENT: Mutex<Option<Content>> = Mutex::new(None);

pub fn bundled() -> Content {
    serde_json::from_str(BUNDLED).expect("exe 에 넣은 content.json 이 깨졌다")
}

pub fn content() -> Content {
    let mut g = CONTENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = g.as_ref() {
        return c.clone();
    }
    let b = bundled();
    // 받아 둔 것이 exe 에 든 것보다 새것일 때만
    let c = crate::prefs::load()
        .get(CACHE_KEY)
        .cloned()
        .and_then(|v| serde_json::from_value::<Content>(v).ok())
        .filter(|c| check(c).is_ok() && c.version >= b.version)
        .unwrap_or(b);
    *g = Some(c.clone());
    c
}

/// 새 내용을 받는다. 지금 것보다 판이 낮거나 같으면 무시. 돌려주는 값: 바뀌었나
pub fn set(v: Value) -> Result<bool, String> {
    let c: Content = serde_json::from_value(v.clone()).map_err(|e| format!("내용 파일을 알아보지 못했어요: {e}"))?;
    check(&c)?;
    if c.version <= content().version {
        return Ok(false);
    }
    crate::prefs::section_set(CACHE_KEY, serde_json::to_value(&c).map_err(|e| e.to_string())?)?;
    crate::log(&format!("[내용] 새 내용 판 {} (숨은 메뉴 {}개)", c.version, c.hidden.len()));
    *CONTENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
    Ok(true)
}

pub fn progress() -> Progress {
    crate::prefs::load().get(PROGRESS_KEY).cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

fn save_progress(p: &Progress) {
    if let Ok(v) = serde_json::to_value(p) {
        let _ = crate::prefs::section_set(PROGRESS_KEY, v);
    }
}

/// (아직 못 찾은 수, 전체 수)
pub fn left() -> (u32, u32) {
    let c = content();
    let p = progress();
    let found = c.hidden.iter().filter(|h| p.discovered.contains(&h.id)).count();
    ((c.hidden.len() - found) as u32, c.hidden.len() as u32)
}

/// 조건을 맞춘 숨은 메뉴 중 **처음 찾은 것**을 표시하고 돌려준다 (여럿이면 첫 번째만 — 나머지는 다음 기회에)
fn discover(p: &mut Progress, ok: impl Fn(&Unlock) -> bool) -> Option<Hidden> {
    let c = content();
    let h = c.hidden.into_iter().find(|h| !p.discovered.contains(&h.id) && ok(&h.unlock))?;
    p.discovered.push(h.id.clone());
    crate::log(&format!("[숨은 메뉴] 찾음: {}", h.id));
    Some(h)
}

/// 쓰레기통에 `batch` 개를 먹였다
pub fn on_feed(batch: usize) -> (u32, Option<Hidden>) {
    let mut p = progress();
    p.feeds += 1;
    let feeds = p.feeds;
    let found = discover(&mut p, |u| match u {
        Unlock::FeedTotal { count } => feeds >= *count,
        Unlock::FeedBatch { count } => batch as u32 >= *count,
        _ => false,
    });
    save_progress(&p);
    (feeds, found)
}

/// 메뉴가 열렸다 (시각 조건)
pub fn on_open(hour: u32) -> Option<Hidden> {
    let mut p = progress();
    let found = discover(&mut p, |u| match u {
        Unlock::Hour { from, to } => hour >= *from && hour < *to,
        _ => false,
    });
    if found.is_some() {
        save_progress(&p);
    }
    found
}

// ── 쪽지 방 창 ────────────────────────────────────────────────────────────────

/// 찾은 숨은 메뉴의 쪽지를 화면 가득 연다 (secret.html — 어두워지고 작은 불빛 아래 글)
pub fn show_secret(app: &AppHandle, id: &str) {
    let url = tauri::WebviewUrl::App(format!("secret.html#{id}").into());
    if let Some(w) = app.get_webview_window(SECRET) {
        let _ = w.close();
    }
    let work = app
        .get_webview_window("settings")
        .and_then(|w| w.primary_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let mut b = tauri::WebviewWindowBuilder::new(app, SECRET, url)
        .title("MAGO 숨은 방")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(true);
    if let Some(m) = work {
        let s = m.scale_factor();
        let size = m.size();
        let pos = m.position();
        b = b.inner_size(size.width as f64 / s, size.height as f64 / s).position(pos.x as f64 / s, pos.y as f64 / s);
    } else {
        b = b.maximized(true);
    }
    if let Err(e) = b.build() {
        crate::log(&format!("[숨은 방] 창을 못 엶: {e}"));
    }
}

// ── command ──────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    id: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    feeds: u32,
    left: u32,
    total: u32,
    version: u32,
    found: Vec<Found>,
}

#[tauri::command]
pub fn mago_state() -> State {
    let c = content();
    let p = progress();
    let (left, total) = left();
    let found = c.hidden.iter().filter(|h| p.discovered.contains(&h.id)).map(|h| Found { id: h.id.clone(), name: h.name.clone() }).collect();
    State { feeds: p.feeds, left, total, version: c.version, found }
}

/// 설정 창(content-sync.js)이 GitHub 에서 받은 content.json 을 넘긴다
#[tauri::command]
pub fn content_set(content: Value) -> Result<bool, String> {
    let changed = set(content)?;
    if changed {
        crate::ring::refresh_counts();
    }
    Ok(changed)
}

/// 쪽지 방이 보여 줄 글 — **찾은 것만** 준다
#[tauri::command]
pub fn mago_reward(id: String) -> Result<Reward, String> {
    if !progress().discovered.contains(&id) {
        return Err("아직 찾지 않은 숨은 메뉴예요".into());
    }
    content().hidden.into_iter().find(|h| h.id == id).map(|h| h.reward).ok_or_else(|| "그 숨은 메뉴는 이제 없어요".into())
}

/// 설정 창의 [다시 보기]
#[tauri::command]
pub fn secret_show(app: AppHandle, id: String) -> Result<(), String> {
    if !progress().discovered.contains(&id) {
        return Err("아직 찾지 않은 숨은 메뉴예요".into());
    }
    show_secret(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn secret_close(app: AppHandle) {
    if let Some(w) = app.get_webview_window(SECRET) {
        let _ = w.close();
    }
}

/// [자세히 보기] — 내용 파일에 적힌 https 링크만 기본 브라우저로
#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    let known = content().hidden.iter().any(|h| h.reward.link.as_deref() == Some(url.as_str()));
    if !known || !url.starts_with("https://") {
        return Err("열 수 없는 주소예요".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(&url, None::<&str>).map_err(|e| format!("브라우저를 열지 못했어요: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_content_is_valid() {
        let c = bundled();
        check(&c).unwrap();
        assert!(c.hidden.len() >= 3);
    }

    #[test]
    fn bad_content_is_refused() {
        let bad = serde_json::json!({ "version": 99, "hidden": [
            { "id": "a", "name": "a", "unlock": { "type": "feedTotal", "count": 1 }, "reward": { "title": "t", "body": "b", "link": "http://x" } }
        ]});
        let c: Content = serde_json::from_value(bad).unwrap();
        assert!(check(&c).is_err(), "http 링크는 막아야 한다");
        let dup = serde_json::json!({ "version": 99, "hidden": [
            { "id": "a", "name": "a", "unlock": { "type": "hour", "from": 0, "to": 5 }, "reward": { "title": "t", "body": "b" } },
            { "id": "a", "name": "b", "unlock": { "type": "hour", "from": 0, "to": 5 }, "reward": { "title": "t", "body": "b" } }
        ]});
        assert!(check(&serde_json::from_value(dup).unwrap()).is_err());
    }

    #[test]
    fn unlock_rules() {
        let mut p = Progress::default();
        // feedTotal 5 는 다섯 번째에 열린다
        for _ in 0..4 {
            p.feeds += 1;
            let f = p.feeds;
            assert!(discover(&mut p, |u| matches!(u, Unlock::FeedTotal { count } if f >= *count)).is_none());
        }
        p.feeds += 1;
        let f = p.feeds;
        assert_eq!(discover(&mut p, |u| matches!(u, Unlock::FeedTotal { count } if f >= *count)).unwrap().id, "trash_note");
        // 한 번 찾으면 다시 안 나온다
        assert!(discover(&mut p, |u| matches!(u, Unlock::FeedTotal { count } if f >= *count)).is_none());
        assert_eq!(discover(&mut p, |u| matches!(u, Unlock::Hour { from, to } if 3 >= *from && 3 < *to)).unwrap().id, "night_owl");
        assert_eq!(discover(&mut p, |u| matches!(u, Unlock::FeedBatch { count } if 12 >= *count)).unwrap().id, "big_bite");
        assert_eq!(p.discovered.len(), 3);
    }
}
