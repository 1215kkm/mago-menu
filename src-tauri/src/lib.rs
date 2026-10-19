//! 시프트 메뉴 — 탐색기에서 Shift 를 누른 채 파일을 끌면 커서 둘레에 원형 메뉴가 뜨고, 칸에 놓으면 원본 옆에 바꾼 파일을 만든다.
//! 컴뚝딱(D:\computer)의 '끌어서 바로 바꾸기'를 떼어 와 혼자 도는 프로그램으로 만든 것. 컴뚝딱 쪽 기능은 그대로 있다.
//!
//! 늘 트레이에서 기다린다. 설정 창(`settings`, ui/index.html)은 처음 켤 때 · 트레이를 누를 때 · 두 번째로 켤 때만 보인다.
//! 설정 창 X 는 끝내기가 아니라 숨기기다 (끝내기는 트레이 메뉴 [끝내기]).

mod convert;
mod outdir;
mod prefs;
mod ring;
mod run_key;
mod single;

use std::io::Write;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

/// `%APPDATA%\com.shiftmenu.app\logs\app.log` 에 한 줄
pub fn log(line: &str) {
    let Some(dir) = std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("com.shiftmenu.app").join("logs")) else { return };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("app.log")) {
        let _ = writeln!(f, "{} [pid {}] {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), std::process::id());
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
    log(&format!("윈도우 켤 때 같이 켜기 = {on}"));
    Ok(Autostart { on: run_key::get().is_some() })
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

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            ring::qconv_settings_get,
            ring::qconv_settings_set,
            ring::qconv_view,
            ring::qconv_preview,
            autostart_get,
            autostart_set,
            app_quit,
        ])
        .setup(move |app| {
            let open = MenuItem::with_id(app, "open", "설정 열기", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "끝내기", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("아이콘"))
                .tooltip("시프트 메뉴 — 탐색기에서 Shift 를 누른 채 파일을 끌어 보세요")
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
            if first_run || !tray_only {
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
        .expect("시프트 메뉴 실행 실패");
}
