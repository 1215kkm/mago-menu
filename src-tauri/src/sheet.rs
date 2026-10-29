//! **JPG 한 장 · PDF 하나로 미리 보기** 창 (`ui/sheet.html`). PDF 하나로는 10-28 CEO "모두 진행해" 로 같은 창을 쓴다 — 칸마다 쪽 하나, 그림은 A4 쪽에.
//! 아래는 JPG 한 장 때의 머리말 (PDF 도 같다 — "파일 폭" 대신 "A4 여백 안") — 고리의 [JPG 한 장으로] 는 바로 저장하지 않고 다 만든 모습을 먼저 보여 준다
//! (10-25 CEO "화질 떨어지면 안돼 크기조절할 사람은 파일변형후 완성파일 미리 보여줘 그중에 이미지들은 현재가 원본크기인걸 표시해두고
//! … 원본보다 커지면 화질이 떨어질수 있다 알려주고 그대로 저장할건지, 조절부터 할건지 선택하게 해줘 원본이 너무 커도 파일폭은 넘어가면 안돼").
//!
//! - 그림은 처음에 **원본 크기**(파일 폭보다 넓은 것만 폭에 맞춰 줄임). 화면이 칸마다 "원본 크기 · 원본의 N%" 를 적는다.
//! - [그대로 저장] 은 그 모습 그대로, [크기 조절] 은 그림 가장자리를 끌어 폭을 바꾼다 — 파일 폭보다 크게는 안 된다 (`mix::Sheet::clamp`).
//! - 원본보다 큰 그림이 있으면 저장 전에 한 번 더 묻는다 (화면이 한다).
//! - 창을 ✕ 로 닫으면 **보이는 그대로 저장**한다 (만든 것을 잃지 않게). 버리려면 [저장 안 함].
//! - 그림은 PC 밖으로 나가지 않는다 — 미리 보기는 data: 주소로 창에 바로 넣는다.

use crate::mix::{Outcome, Part, Sheet};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "sheet";
/// 미리 보기 그림의 파일 폭 (px) — 창(최대 약 760 CSS px)에서 또렷하게
pub const THUMB_W: u32 = 900;

struct Pending {
    sheet: Sheet,
    thumbs: Vec<String>,
    /// 화면이 마지막으로 알려 준 폭 — ✕ 로 닫으면 이대로 저장
    widths: Vec<u32>,
    /// 준비하다 못 넣은 파일 수 · 첫 이유
    failed: usize,
    first_error: String,
    /// 저장한 파일 — 다시 저장하면 이 파일을 고친다
    saved: Option<PathBuf>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// 창을 닫아 뒤에서 저장하는 중인 수 (시험 실행이 끝나기 전에 기다린다)
static SAVING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 시험 실행(lib.rs --시험바꾸기 mix-jpg-view) — 창을 화면 밖에 앞으로 나오지 않게 띄운다 (대표님이 쓰는 화면을 가리지 않게. 안은 원격 디버깅으로 본다)
pub static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 기다리는 것도 뒤에서 저장하는 것도 없다
pub fn idle() -> bool {
    pending(|p| p.is_none()) && SAVING.load(std::sync::atomic::Ordering::SeqCst) == 0
}

fn pending<R>(f: impl FnOnce(&mut Option<Pending>) -> R) -> R {
    f(&mut PENDING.lock().unwrap_or_else(|e| e.into_inner()))
}

/// 미리 보기 창이 저장을 기다리는 중인가 (업데이트 설치를 미룬다)
pub fn waiting() -> bool {
    pending(|p| p.as_ref().is_some_and(|p| p.saved.is_none()))
}

/// 고리에서 [JPG 한 장으로] 를 다 준비했다 → 창을 연다. 앞의 것을 아직 저장하지 않았으면 보이던 그대로 먼저 저장한다
pub fn open(app: &AppHandle, sheet: Sheet, thumbs: Vec<String>, prep: &Outcome) {
    let widths = sheet.default_widths();
    let old = pending(|p| p.replace(Pending { sheet, thumbs, widths, failed: prep.failed, first_error: prep.first_error.clone(), saved: None }));
    if let Some(old) = old {
        save_in_background(old);
    }
    let title = if pending(|p| p.as_ref().is_some_and(|p| p.sheet.pdf)) { "MAGO — PDF 하나로 미리 보기" } else { "MAGO — JPG 한 장 미리 보기" };
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_title(title);
        let _ = app.emit_to(LABEL, "sheet:new", ());
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let quiet = QUIET.load(std::sync::atomic::Ordering::SeqCst);
    let b = tauri::WebviewWindowBuilder::new(app, LABEL, tauri::WebviewUrl::App("sheet.html".into()))
        .title(title)
        .inner_size(1000.0, 820.0)
        .min_inner_size(520.0, 560.0)
        .focused(!quiet)
        // 놓기는 받지 않는다 (탐색기가 원본을 옮기지 않게 — ring.rs 머리말)
        .disable_drag_drop_handler();
    let b = if quiet { b.position(-6000.0, -6000.0).skip_taskbar(true) } else { b.center() };
    // 시험 실행만: 창 안을 원격 디버깅(CDP)으로 들여다본다 — 다른 설정이라 WebView2 자료 폴더도 따로
    let b = match std::env::var("MAGO_CDP_PORT").ok().filter(|_| quiet) {
        Some(port) => b
            .additional_browser_args(&format!("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port}"))
            .data_directory(std::env::temp_dir().join(format!("mago-cdp-{port}"))),
        None => b,
    };
    match b.build() {
        Ok(w) => {
            if !quiet {
                let _ = w.set_focus();
            }
        }
        Err(e) => {
            crate::log(&format!("[미리 보기] 창을 못 만듦: {e} — 원래 크기로 바로 저장"));
            if let Some(p) = pending(|p| p.take()) {
                save_in_background(p);
            }
        }
    }
}

/// 저장할 파일 — 처음이면 1번 파일 옆(못 쓰는 폴더면 ring.rs 가 정한 다른 곳) 새 이름, 다시 저장이면 그 파일
fn target(p: &Pending) -> PathBuf {
    p.saved.clone().unwrap_or_else(|| crate::convert::convert::unique_path(&p.sheet.dir, &format!("{}.{}", p.sheet.name, if p.sheet.pdf { "pdf" } else { "jpg" })))
}

fn save_in_background(p: Pending) {
    if p.saved.is_some() {
        return;
    }
    SAVING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
        let dst = target(&p);
        let mut out = Outcome::default();
        let r = p.sheet.draw(&p.widths, &dst, &mut |_| {}, &mut out);
        crate::log(&format!("[미리 보기] 보이던 그대로 저장 → {} {:?} 폭 {:?}", dst.display(), r, p.widths));
        drop(p);
        SAVING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewPart {
    #[serde(flatten)]
    part: Part,
    /// 처음 폭 (px)
    w: u32,
    thumb: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// 저장될 이름 (확장자 포함)
    name: String,
    /// 파일 폭 (px) — 그림은 이보다 넓어지지 않는다
    width: u32,
    parts: Vec<ViewPart>,
    failed: usize,
    first_error: String,
    /// 문서 몇 개는 프로그램이 없어 글만 옮겼다
    text_only: usize,
    /// 저장할 폴더
    dir: String,
    /// 한 장의 상한 — 넘으면 전체를 같은 비율로 줄인다 (화면이 다 그린 크기를 미리 셈한다)
    max_height: u64,
    max_pixels: u64,
    saved: Option<String>,
    /// PDF 하나로 (칸마다 쪽 하나) — 아니면 JPG 한 장
    pdf: bool,
}

#[tauri::command]
pub fn sheet_get() -> Result<View, String> {
    pending(|p| {
        let p = p.as_ref().ok_or("미리 볼 것이 없어요 — 창을 닫고 다시 끌어 주세요")?;
        let name = target(p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Ok(View {
            name,
            width: p.sheet.width,
            parts: p.sheet.parts.iter().zip(&p.widths).zip(&p.thumbs).map(|((part, &w), t)| ViewPart { part: part.clone(), w, thumb: t.clone() }).collect(),
            failed: p.failed,
            first_error: p.first_error.clone(),
            text_only: p.sheet.text_only,
            dir: p.sheet.dir.to_string_lossy().into(),
            max_height: crate::convert::ext_convert::ONE_MAX_HEIGHT,
            max_pixels: crate::convert::ext_convert::ONE_MAX_PIXELS,
            saved: p.saved.as_ref().map(|s| s.to_string_lossy().to_string()),
            pdf: p.sheet.pdf,
        })
    })
}

/// 화면에서 폭을 바꿀 때마다 — ✕ 로 닫으면 이대로 저장한다
#[tauri::command]
pub fn sheet_set(widths: Vec<u32>) {
    pending(|p| {
        if let Some(p) = p.as_mut() {
            p.widths = p.sheet.clamp(&widths);
        }
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    file: String,
    note: String,
    /// 다 그린 크기 (px)
    w: u32,
    h: u64,
    bytes: u64,
    /// 쪽 수 (PDF 하나로)
    pages: usize,
}

/// [그대로 저장] · [이 크기로 저장] — 오래 걸릴 수 있어 따로 돌린다. 진행은 sheet:progress (0~1)
#[tauri::command]
pub async fn sheet_save(app: AppHandle, widths: Vec<u32>) -> Result<Saved, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut g = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        let p = g.as_mut().ok_or("미리 볼 것이 없어요")?;
        p.widths = p.sheet.clamp(&widths);
        let dst = target(p);
        let mut out = Outcome::default();
        let note = p.sheet.draw(&p.widths, &dst, &mut |f| {
            let _ = app.emit_to(LABEL, "sheet:progress", f);
        }, &mut out)?;
        let (sizes, cw, _) = p.sheet.layout(&p.widths);
        p.saved = Some(dst.clone());
        let bytes = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
        crate::log(&format!("[미리 보기] 저장 {} · {note} · 폭 {:?}", dst.display(), p.widths));
        Ok(Saved { file: dst.to_string_lossy().into(), note, w: cw, h: sizes.iter().map(|s| s.1 as u64).sum(), bytes, pages: p.sheet.parts.len() })
    })
    .await
    .map_err(|e| format!("저장하다 멈췄어요: {e}"))?
}

/// [저장 안 함] — 만든 것을 버리고 창을 닫는다 (원본은 그대로)
#[tauri::command]
pub fn sheet_cancel(app: AppHandle) {
    let p = pending(|p| p.take());
    crate::log(&format!("[미리 보기] 저장 안 함 (저장했던 것 {:?})", p.as_ref().and_then(|p| p.saved.clone())));
    drop(p);
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

/// [폴더에서 보기] — 저장한 파일을 탐색기에서 골라 둔 채로
#[tauri::command]
pub fn sheet_reveal() -> Result<(), String> {
    let f = pending(|p| p.as_ref().and_then(|p| p.saved.clone())).ok_or("아직 저장하지 않았어요")?;
    crate::web::reveal(&f)
}

/// [닫기]
#[tauri::command]
pub fn sheet_close(app: AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

/// 창이 닫힌다 (✕ · [닫기]) — 아직 저장하지 않았으면 보이던 그대로 저장
pub fn on_close(label: &str) {
    if label != LABEL {
        return;
    }
    // 저장하는 중이면 그것이 끝날 때까지 기다려야 한다 — 창 일(메인 스레드)을 막지 않게 따로
    SAVING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(|| {
        if let Some(p) = pending(|p| p.take()) {
            save_in_background(p);
        }
        SAVING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    });
}
