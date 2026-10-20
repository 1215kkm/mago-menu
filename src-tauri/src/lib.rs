//! **MAGO** — 탐색기에서 Shift 를 누른 채 파일을 끌면 커서 둘레에 원형 메뉴가 뜨고, 칸에 놓으면 원본 옆에 바꾼 파일을 만든다.
//! 고리 오른쪽 아래에는 쓰레기통 캐릭터가 산다 — 파일을 먹이면 윈도우 휴지통으로 보내고, 많이 먹이면 숨은 메뉴가 열린다.
//! 컴뚝딱(D:\computer)의 '끌어서 바로 바꾸기'를 떼어 와 혼자 도는 프로그램으로 만든 것 (10-20 컴뚝딱에서는 뺐다).
//!
//! - `ring.rs`    : 끌기 감시 · 고리 · 놓기 창 · 쓰레기통 먹이기
//! - `actions.rs` : 정사각(SNS) · 1MB · 휴지통
//! - `content.rs` : 숨은 메뉴 내용(관리자가 GitHub 에서 고침) · 찾은 것 · 쪽지 방 창
//! - `updater.rs` : 프로그램 자동 업데이트
//!
//! 늘 트레이에서 기다린다. 설정 창(`settings`, ui/index.html)은 처음 켤 때 · 트레이를 누를 때 · 두 번째로 켤 때만 보인다.
//! 설정 창 X 는 끝내기가 아니라 숨기기다 (끝내기는 트레이 메뉴 [끝내기]).

mod actions;
mod anim;
mod content;
mod convert;
mod cutout;
mod outdir;
mod pdfprint;
mod prefs;
mod ring;
mod run_key;
mod single;
mod text;
mod updater;

use std::io::Write;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

/// `%APPDATA%\com.mago.app\logs\app.log` 에 한 줄
pub fn log(line: &str) {
    let Some(dir) = std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("com.mago.app").join("logs")) else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("app.log")) {
        let _ = writeln!(f, "{} [pid {}] {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), std::process::id());
    }
}

/// 트레이 그림 — 설정에서 고른 캐릭터 얼굴 (exe·바탕화면 아이콘은 기본 캐릭터인 고양이로 고정)
fn tray_icon(character: &str) -> tauri::image::Image<'static> {
    let png: &[u8] = if character == "trash" { include_bytes!("../icons/tray-trash.png") } else { include_bytes!("../icons/tray-cat.png") };
    let rgba = image::load_from_memory(png).expect("트레이 그림").to_rgba8();
    let (w, h) = rgba.dimensions();
    tauri::image::Image::new_owned(rgba.into_raw(), w, h)
}

/// 설정에서 캐릭터를 바꾸면 트레이 그림도 바로 바꾼다
pub fn set_tray_character(app: &tauri::AppHandle, character: &str) {
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_icon(Some(tray_icon(character)));
    }
}

fn show_settings(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

// ── 화면에서 부르는 것 (고리 설정은 ring.rs) ──────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Autostart {
    on: bool,
}

#[tauri::command]
fn autostart_get() -> Autostart {
    Autostart { on: run_key::get().is_some() }
}

#[tauri::command]
fn autostart_set(on: bool) -> Result<Autostart, String> {
    if on {
        run_key::enable()?;
    } else {
        run_key::disable()?;
    }
    // 사용자가 직접 고른 것 — 다음에 켤 때 기본값으로 덮지 않는다
    let _ = prefs::section_set(AUTOSTART_DEFAULTED, serde_json::Value::Bool(true));
    log(&format!("윈도우 켤 때 같이 켜기 = {on}"));
    Ok(Autostart { on: run_key::get().is_some() })
}

/// prefs 칸: 기본값(켬)을 이미 한 번 넣었거나 사용자가 직접 골랐다는 표시
const AUTOSTART_DEFAULTED: &str = "autostartDefaulted";

/// cargo 로 빌드한 시험용 exe 인가 (`...\target\release\mago.exe`) — 시험 실행이 대표님 PC 시작프로그램에 박히면 안 된다
/// 시험용 exe 만: `mago.exe --시험바꾸기 <칸> <파일>` — 끌기 없이 그 칸 일을 한 번 하고 로그에 남긴 뒤 끝난다.
/// 마우스를 움직이지 않고 PDF(숨은 화면 창)·누끼까지 실제 프로그램 안에서 확인하려고 둔다. 설치한 exe 는 무시한다.
const TRY_ARG: &str = "--시험바꾸기";

fn try_one(app: tauri::AppHandle, target: String, file: String) {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if !is_dev_build(&exe) {
        log("[시험] 설치한 exe 라 --시험바꾸기 를 무시함");
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        let src = std::path::PathBuf::from(&file);
        let dir = src.parent().map(|d| d.to_path_buf()).unwrap_or_default();
        let t = std::time::Instant::now();
        match ring::one(&target, &src, &dir, &mut |_| {}) {
            Ok(o) => log(&format!("[시험] {target} 됨 {}ms → {} ({}B)", t.elapsed().as_millis(), o.file, o.bytes)),
            Err(e) => log(&format!("[시험] {target} 실패 — {e}")),
        }
        app.exit(0);
    });
}

fn is_dev_build(exe: &str) -> bool {
    let e = exe.to_lowercase().replace('/', "\\");
    e.contains("\\target\\release\\") || e.contains("\\target\\debug\\")
}

/// 윈도우 켤 때 같이 켜기는 **기본 켬** (10-20 대표님) — 늘 켜져 있어야 쓸 수 있는 프로그램이라서.
/// 한 번만 넣는다: 그 뒤 사용자가 끄면 끈 채로 둔다. 0.1.0 을 깐 사람도 0.1.1 로 바뀌며 한 번 켜진다.
fn apply_autostart_default() {
    if prefs::load().get(AUTOSTART_DEFAULTED).is_some() {
        return;
    }
    let Some(exe) = run_key::current_exe() else { return };
    if is_dev_build(&exe) {
        log("시험용 exe 라 윈도우 켤 때 같이 켜기 기본값을 넣지 않음");
        return;
    }
    match run_key::enable() {
        Ok(_) => {
            let _ = prefs::section_set(AUTOSTART_DEFAULTED, serde_json::Value::Bool(true));
            log("윈도우 켤 때 같이 켜기 = true (기본값)");
        }
        Err(e) => log(&format!("윈도우 켤 때 같이 켜기 기본값 못 넣음: {e}")),
    }
}

#[tauri::command]
fn app_quit(app: tauri::AppHandle) {
    app.exit(0);
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let tray_only = args.iter().any(|a| a == run_key::TRAY_ARG);
    log(&format!("켜짐 v{} 인자={:?}", env!("CARGO_PKG_VERSION"), &args[1..]));
    if !single::acquire() {
        log("이미 켜져 있어 그 설정 창을 앞으로 부르고 이 실행은 끝냄");
        single::focus_existing();
        return;
    }
    // 처음 켜는 사람에게는 설정 창을 보여 준다 (트레이에만 숨어 있으면 켜졌는지 모른다)
    let first_run = prefs::path().map(|p| !p.exists()).unwrap_or(true);
    apply_autostart_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            ring::qconv_settings_get,
            ring::qconv_settings_set,
            ring::qconv_view,
            ring::qconv_preview,
            content::mago_state,
            content::content_set,
            content::mago_reward,
            content::secret_show,
            content::secret_close,
            content::open_link,
            autostart_get,
            autostart_set,
            app_quit,
        ])
        .setup(move |app| {
            let open = MenuItem::with_id(app, "open", "설정 열기", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "끝내기", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(tray_icon(&ring::settings().character))
                .tooltip("MAGO — 탐색기에서 Shift 를 누른 채 파일을 끌어 보세요")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => show_settings(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_settings(tray.app_handle());
                    }
                })
                .build(app)?;
            ring::start(app.handle().clone());
            updater::spawn(app.handle().clone());
            if let Some(i) = args.iter().position(|a| a == TRY_ARG) {
                try_one(app.handle().clone(), args.get(i + 1).cloned().unwrap_or_default(), args.get(i + 2).cloned().unwrap_or_default());
            } else if first_run || !tray_only {
                show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "settings" {
                    api.prevent_close();
                    let _ = window.hide();
                    log("설정 창 X → 트레이로 숨김");
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("MAGO 실행 실패");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10-20 CEO "케릭터를 고양이로 설정했는데 … 트레이아이콘이 쓰레기통" — 캐릭터마다 트레이 그림이 다르고 둘 다 읽힌다
    #[test]
    fn the_tray_icon_follows_the_character() {
        let cat = tray_icon("cat");
        let trash = tray_icon("trash");
        assert_eq!((cat.width(), cat.height()), (64, 64));
        assert_eq!((trash.width(), trash.height()), (64, 64));
        assert_ne!(cat.rgba(), trash.rgba());
        assert_eq!(tray_icon("모름").rgba(), cat.rgba(), "모르는 값은 기본(고양이)");
    }

    /// 시험용 exe 는 대표님 PC 시작프로그램에 박히면 안 된다 — 설치된 exe 만 기본 켬
    #[test]
    fn only_installed_exes_get_the_autostart_default() {
        assert!(is_dev_build(r"D:\shift-menu\src-tauri\target\release\mago.exe"));
        assert!(is_dev_build("D:/shift-menu/src-tauri/Target/Debug/mago.exe"));
        assert!(!is_dev_build(r"C:\Users\강경모\AppData\Local\MAGO\mago.exe"));
    }
}
