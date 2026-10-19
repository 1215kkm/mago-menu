//! 사용자가 정한 값을 **앱을 껐다 켜도 기억한다.** (`%APPDATA%\com.mago.app\prefs.json`)
//!
//! ## 왜 생겼나 — 저장 번호가 거짓말을 하고 있었다
//! 화면은 "번호가 저절로 이어져요" 라고 적혀 있는데, 다음 번호는 메모리에만 있었다.
//! 앱을 껐다 켜면 01 로 돌아가고 **어제 저장한 `_01` 을 덮어썼다.** 여기서 디스크에 남긴다.
//!
//! ## 번호의 열쇠는 (저장 폴더 + 이름 앞부분) 이다
//! 번호가 존재하는 이유는 딱 하나 — *같은 폴더 안에서 같은 이름끼리 부딪히지 않게*.
//! 그래서 부딪힘이 일어나는 범위를 그대로 열쇠로 쓴다.
//! - 문서를 열쇠로 하면: 같은 문서를 다른 폴더에 저장할 때 번호가 괜히 뛴다 (그 폴더는 비어 있는데 _07 부터).
//! - 폴더만 열쇠로 하면: 한 폴더에 서로 다른 문서를 저장할 때 서로의 번호를 밀어 올린다.
//! - (폴더 + 이름 앞부분) 이면 둘 다 안 생긴다. 문서 이름을 쓰든 날짜를 쓰든 직접 쓰든 같은 규칙이다.
//!
//! 번호는 **저장이 성공한 뒤에만** 올린다. 실패했는데 번호가 오르면 다음 파일이 `_03` 부터 시작한다.
//! 그리고 실제 파일 이름은 `tmp::unique_path` 가 한 번 더 확인한다 — 이미 있으면 다음 번호로 건너뛴다.
//!
//! ## 사용자 파일은 건드리지 않는다
//! 여기 적히는 건 우리 설정뿐이다. 사진·PSD·저장 폴더 안의 파일에는 손대지 않는다.

#![allow(dead_code)] // 컴뚝딱에서 가져온 그대로 — 일부(번호 이어 붙이기·다운로드 폴더)는 안 쓴다
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Mutex;

/// 화면이 통째로 맡겨 두는 칸 (저장 폴더·이름 규칙·형식·워터마크·프리셋·필터·GIF…).
/// Rust 는 내용을 해석하지 않는다 — 화면이 아는 모양 그대로 두었다가 그대로 돌려준다.
/// 그래야 화면에 항목이 하나 늘 때마다 Rust 를 고치지 않아도 된다.
const UI_KEY: &str = "ui";
/// 저장 번호 칸. `"<폴더>|<이름 앞부분>": 다음 번호`
const SEQ_KEY: &str = "saveSeq";

/// 파일을 매번 읽지 않도록 한 벌 들고 있는다. 쓰기는 항상 파일까지 간다 (앱이 갑자기 꺼져도 남게).
static CACHE: Mutex<Option<Value>> = Mutex::new(None);

/// 테스트가 진짜 설정 파일을 건드리지 않게 하는 자리. 평소엔 언제나 `None`.
static PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn path() -> Option<PathBuf> {
    if let Some(p) = PATH_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(p);
    }
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join("com.mago.app").join("prefs.json"))
}

fn empty() -> Value {
    Value::Object(Map::new())
}

/// 통째로 읽기. 파일이 없거나 깨졌으면 빈 값 — **설정이 깨졌다고 앱이 안 뜨면 안 된다.**
pub fn load() -> Value {
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(v) = c.as_ref() {
        return v.clone();
    }
    let v = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(empty);
    *c = Some(v.clone());
    v
}

fn store(v: Value) -> Result<(), String> {
    let p = path().ok_or("APPDATA 를 찾지 못했습니다.")?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("설정 폴더를 만들지 못했습니다: {e}"))?;
    }
    let body = serde_json::to_string_pretty(&v).map_err(|e| format!("설정을 만들지 못했습니다: {e}"))?;
    std::fs::write(&p, body).map_err(|e| format!("설정을 저장하지 못했습니다: {e}"))?;
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
    Ok(())
}

/// 위 한 겹만 합친다. 화면이 바뀐 항목만 보내면 나머지는 그대로 남는다.
fn merge_into(root: &mut Value, section: &str, patch: &Value) {
    let obj = root.as_object_mut().expect("루트는 객체");
    let slot = obj.entry(section.to_string()).or_insert_with(empty);
    if !slot.is_object() {
        *slot = empty();
    }
    let (Some(dst), Some(src)) = (slot.as_object_mut(), patch.as_object()) else { return };
    for (k, v) in src {
        dst.insert(k.clone(), v.clone());
    }
}

/// 번호 열쇠. 대소문자·구분자 방향·끝 구분자를 정리한다 — 같은 폴더를 다르게 적어도 같은 칸을 쓰게.
/// (폴더 선택창은 `D:\사진`, 화면 코드가 만든 값은 `D:/사진` 처럼 섞여서 온다)
pub fn seq_key(folder: &str, base: &str) -> String {
    let f = folder.trim().replace('/', "\\");
    let f = f.trim_end_matches('\\').to_lowercase();
    format!("{f}|{base}")
}

/// 이 (폴더, 이름) 으로 다음에 쓸 번호. 처음이면 1.
pub fn seq_peek(folder: &str, base: &str) -> u32 {
    load()
        .get(SEQ_KEY)
        .and_then(|m| m.get(seq_key(folder, base)))
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .clamp(1, 999_999) as u32
}

/// **저장이 성공한 뒤에만** 부른다. `used` 는 방금 실제로 쓴 번호.
pub fn seq_commit(folder: &str, base: &str, used: u32) -> Result<(), String> {
    let mut root = load();
    let patch = serde_json::json!({ seq_key(folder, base): used.saturating_add(1) });
    merge_into(&mut root, SEQ_KEY, &patch);
    store(root)
}

/// Rust 가 직접 쓰는 설정 한 칸을 통째로 바꾼다 (예: `quickConvert` — 화면이 꺼져 있어도 감시 스레드가 읽는다)
pub fn section_set(section: &str, value: Value) -> Result<(), String> {
    let mut root = load();
    root.as_object_mut().expect("루트는 객체").insert(section.to_string(), value);
    store(root)
}

// ── command ──────────────────────────────────────────────────────────────────

/// 화면이 시작할 때 한 번. 저장해 둔 값이 없으면 빈 객체.
#[tauri::command]
pub fn prefs_get() -> Value {
    load().get(UI_KEY).cloned().unwrap_or_else(empty)
}

/// 화면이 바뀐 항목만 보낸다.
#[tauri::command]
pub fn prefs_set(patch: Value) -> Result<Value, String> {
    if !patch.is_object() {
        return Err("설정은 객체여야 합니다.".into());
    }
    let mut root = load();
    merge_into(&mut root, UI_KEY, &patch);
    store(root)?;
    Ok(prefs_get())
}

/// 화면이 "다음에 저장될 파일" 을 그릴 때 물어본다.
#[tauri::command]
pub fn save_seq_peek(folder: String, base: String) -> u32 {
    if folder.trim().is_empty() || base.trim().is_empty() {
        return 1;
    }
    seq_peek(&folder, &base)
}

/// 테스트 전용 — 진짜 설정 파일 대신 임시 파일을 보게 하고, 들고 있던 값도 버린다.
#[cfg(test)]
pub(crate) fn use_test_path(p: Option<PathBuf>) {
    *PATH_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = p;
    forget_cache();
}

/// 테스트 전용 — 파일에서 다시 읽게 만든다 (앱을 껐다 켠 것과 같은 상태).
#[cfg(test)]
pub(crate) fn forget_cache() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 파일을 건드리지 않고 합치기만 확인한다 (진짜 prefs.json 은 테스트가 손대지 않는다).
    #[test]
    fn merge_keeps_the_keys_it_was_not_told_about() {
        let mut root = serde_json::json!({ "ui": { "folder": "D:/old", "format": "png" } });
        merge_into(&mut root, "ui", &serde_json::json!({ "format": "jpg" }));
        assert_eq!(root["ui"]["folder"], "D:/old", "안 보낸 항목은 그대로 남아야 한다");
        assert_eq!(root["ui"]["format"], "jpg");
    }

    #[test]
    fn merge_makes_the_section_when_it_is_missing_or_broken() {
        let mut root = serde_json::json!({});
        merge_into(&mut root, "ui", &serde_json::json!({ "a": 1 }));
        assert_eq!(root["ui"]["a"], 1);
        let mut broken = serde_json::json!({ "ui": "이건 객체가 아니다" });
        merge_into(&mut broken, "ui", &serde_json::json!({ "a": 1 }));
        assert_eq!(broken["ui"]["a"], 1, "깨진 칸은 새로 만든다 — 앱이 죽으면 안 된다");
    }

    #[test]
    fn seq_key_ignores_case_and_trailing_separators() {
        assert_eq!(seq_key(r"D:\사진\", "상세"), seq_key(r"d:/사진", "상세"));
        assert_ne!(seq_key(r"D:\사진", "상세"), seq_key(r"D:\사진", "썸네일"), "이름이 다르면 번호도 따로");
        assert_ne!(seq_key(r"D:\사진", "상세"), seq_key(r"D:\보관", "상세"), "폴더가 다르면 번호도 따로");
    }

    #[test]
    fn peek_is_one_when_nothing_is_remembered() {
        assert_eq!(save_seq_peek("".into(), "상세".into()), 1);
        assert_eq!(save_seq_peek(r"D:\사진".into(), "".into()), 1);
    }

    /// 진짜 파일까지 한 바퀴 — **사용자 설정 파일 대신 임시 파일**로.
    #[test]
    fn seq_round_trips_through_the_file() {
        let tmp = std::env::temp_dir().join(format!("kom-prefs-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        use_test_path(Some(tmp.clone()));

        assert_eq!(seq_peek(r"D:\시험", "상세"), 1, "처음엔 1");
        seq_commit(r"D:\시험", "상세", 7).unwrap();
        forget_cache(); // 파일에서 다시 읽게 — 앱을 껐다 켠 것과 같다
        assert_eq!(seq_peek(r"D:\시험", "상세"), 8, "저장한 번호 다음");
        assert_eq!(seq_peek(r"D:\시험", "썸네일"), 1, "다른 이름은 영향 없음");

        // 화면 설정도 같은 파일에 함께 살아남는다
        prefs_set(serde_json::json!({ "folder": "D:/사진", "format": "jpg" })).unwrap();
        forget_cache();
        assert_eq!(prefs_get()["folder"], "D:/사진");
        assert_eq!(seq_peek(r"D:\시험", "상세"), 8, "설정을 저장해도 번호는 안 흔들린다");

        let _ = std::fs::remove_file(&tmp);
        use_test_path(None);
    }
}
