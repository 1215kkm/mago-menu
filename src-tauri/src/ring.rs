//! **MAGO** — 컴뚝딱(D:\computer) `src-tauri/src/imagery/quick.rs` 를 떼어 와 혼자 도는 프로그램으로 만든 것 (2026-10-20).
//! 아래 머리말은 컴뚝딱 때 그대로다. 설정 화면 이름만 다르다(컴뚝딱 '파일 형식 바꾸기' → 여기 `ui/index.html`).
//!
//! **끌다가 바로 바꾸기** (10-19 CEO "파일 선택하고 이동하다가 시프트 누르면 나오는 메뉴 — 우리 컴뚝딱 설치한 사람도 그렇게 쓸 수 있도록").
//!
//! 탐색기에서 정한 키(기본 Shift)를 누른 채 파일을 끌면 커서 둘레에 형식 고리가 뜬다.
//! 10-19 CEO: 한때 Ctrl 로 바꿨다가("컨트롤 누르고 옮길 때") "컨트롤 드래그는 윈도우의 복사 기능이었네, 시프트로 해야겠다" 로 Shift 로 돌아왔다.
//! 키를 먼저 누르고 끌어도, 끄는 도중에 눌러도 된다. 한 번 끌 때 한 번만 뜬다(Esc 로 닫으면 그 끌기 동안은 다시 안 뜬다).
//! 고리의 칸 위에 놓으면 그 형식으로 바꿔 **원본 옆에** 새 파일로 저장한다. 가운데·바깥에 놓으면 아무 일도 없다.
//! 바꾸는 일은 확장자 바꾸기(`ext_convert`)를 그대로 쓴다 — 그림끼리 · wav ↔ mp3 · PDF → JPG·PNG.
//!
//! ## 창이 둘인 이유 — 원본이 지워지면 안 된다
//! 탐색기에서 Shift 를 누르고 끌면 **"옮기기"** 다 (키를 Shift 로 바꿀 수 있고, 같은 드라이브 안 끌기는 키 없이도 옮기기다). 놓은 쪽이 "옮겼다" 고 답하면 탐색기가 원본을 지운다.
//! Tauri(wry)의 놓기 처리기는 놓을 때 답(DROPEFFECT)을 정해 주지 않아 탐색기가 "옮기기" 로 받아들일 수 있다.
//! 그래서 놓기는 **우리가 만든 둥근 투명 창**(`DropTarget`)이 받고, 언제나 "복사" 로 답한다.
//! 보이는 고리(`qconv.html` 웹 창)는 마우스가 통과하는 그림일 뿐이다. 어느 칸 위인지도 여기서 정해 그림에 알려 준다
//! — 그림과 실제 놓은 칸이 어긋날 수 없게.
//!
//! ## 언제 뜨나
//! 왼쪽 버튼을 누른 채 8px 넘게 움직였고(끌기), 키가 눌려 있고, 파일 끌기 그림(`SysDragImage` 창)이 떠 있을 때.
//! 끌기 그림을 못 찾으면 탐색기·바탕화면에서 시작한 끌기 중 **영역 선택(올가미)이 아닌 것**만 — Shift·Ctrl + 올가미는 "선택 넓히기·더하기" 라
//! 고리가 뜨면 안 된다. 올가미 동안은 탐색기 항목 창(DirectUIHWND · SysListView32)이 마우스를 잡고 있어 그것으로 가른다.
//! 10-19 CEO "포토샵 도우미 기능이 아니었어 — 컴뚝딱이 켜져 있으면 윈도우 폴더에서 사용되는 기능" → **기본으로 켜 둔다.**
//! 설정은 왼쪽 메뉴 '파일 형식 바꾸기'(app/modules/file-converter). 끄면 감시 스레드는 0.3초마다 설정만 보고 아무것도 하지 않는다.

use crate::convert::ext_convert::{self, ExtRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const LABEL: &str = "qconv";
/// 고리 창 한 변 (논리 px). 고리 + 오른쪽 아래 쓰레기통 + 하트·반짝이 여유
pub const SIZE: f64 = 560.0;
/// 칸으로 치는 거리 (논리 px, 가운데에서). 이보다 안쪽은 "취소", 바깥도 "취소".
/// MAGO 에서 고리를 조금 줄였다(178 → 150) — 바깥에 쓰레기통이 산다
pub const R_IN: f64 = 54.0;
pub const R_OUT: f64 = 150.0;
/// 쓰레기통 자리 (고리 가운데에서 논리 px, 오른쪽 아래 45°). qconv.js 의 TRASH 와 같아야 한다
pub const TRASH_X: f64 = 162.0;
pub const TRASH_Y: f64 = 162.0;
/// 쓰레기통 가운데에서 이 안에 놓으면 먹는다
pub const EAT_R: f64 = 66.0;
const PREFS_KEY: &str = "quickConvert";

/// square = SNS 정사각 1080 · small = 1MB 이하 JPG (actions.rs)
pub const IMAGE_OPTS: &[&str] = &["png", "jpg", "webp", "bmp", "gif", "square", "small"];
pub const AUDIO_OPTS: &[&str] = &["mp3", "wav"];
pub const PDF_OPTS: &[&str] = &["jpg-pages", "jpg-one", "png-pages", "png-one"];
/// 고를 수 있는 캐릭터 (ui/qconv.js · settings.js 의 CHARS 와 같아야 한다)
pub const CHARACTERS: &[&str] = &["cat", "trash"];

// ── 설정 ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub enabled: bool,
    /// shift | ctrl | alt
    pub key: String,
    pub image: Vec<String>,
    pub audio: Vec<String>,
    pub pdf: Vec<String>,
    pub quality: u8,
    pub dpi: u32,
    /// 움직이는 GIF 한 장을 보여 주는 시간 (ms) — anim::SPEEDS 중 하나
    pub gif_ms: u32,
    /// 고리 옆 캐릭터 — CHARACTERS 중 하나 (그림은 ui/mago/<이름>/, 쓰레기통은 ui/mago/)
    pub character: String,
    /// 설정 모양 판. 0 = v0.3.45 (기본 Shift · 꺼짐) · 1 = v0.3.46 (기본 Ctrl) · 2 = v0.3.47 (Shift) · 3 = MAGO 0.1.0 (기본 켜짐) · 4 = 지금 (GIF 칸 기본).
    /// 칸이 없으면 0 으로 읽는다 — 구조체 전체 기본값(지금 판)을 쓰면 옛 설정을 알아볼 수 없다
    #[serde(default)]
    pub ver: u32,
}

/// 지금 설정 판
const SETTINGS_VER: u32 = 4;

impl Default for Settings {
    fn default() -> Self {
        Self {
            // 컴뚝딱이 켜져 있으면 늘 되는 기능 (머리말). 끄는 건 '파일 형식 바꾸기' 화면에서
            enabled: true,
            key: "shift".into(),
            // BMP 는 설정에서 켤 수 있다. GIF 는 움직이는 그림을 만드는 칸이라 기본으로 (10-20 CEO "GIF 로 변환도 가능하게")
            image: ["png", "jpg", "webp", "gif", "square", "small"].iter().map(|s| s.to_string()).collect(),
            audio: AUDIO_OPTS.iter().map(|s| s.to_string()).collect(),
            pdf: vec!["jpg-pages".into(), "jpg-one".into(), "png-pages".into()],
            quality: 92,
            dpi: 150,
            gif_ms: crate::anim::DEFAULT_MS,
            // 0.1.2 부터 검은 고양이가 기본 (10-20 CEO "고양이 케릭터 추가해서 기본으로"). 칸이 없는 옛 설정도 고양이로 읽힌다
            character: "cat".into(),
            ver: SETTINGS_VER,
        }
    }
}

impl Settings {
    /// 모르는 값은 버리고 순서는 정해 둔 순서로 — 화면이 무엇을 보내도 고리가 깨지지 않게
    fn clean(mut self) -> Self {
        let keep = |v: &[String], opts: &[&str]| opts.iter().filter(|o| v.iter().any(|x| x == *o)).map(|s| s.to_string()).collect::<Vec<_>>();
        // 0.1.0 까지는 GIF 칸이 기본으로 꺼져 있었다 — 그때 저장된 목록에 한 번 더한다 (지금 판에서 끈 사람은 그대로)
        if self.ver < 4 && !self.image.iter().any(|x| x == "gif") {
            self.image.push("gif".into());
        }
        self.image = keep(&self.image, IMAGE_OPTS);
        self.audio = keep(&self.audio, AUDIO_OPTS);
        self.pdf = keep(&self.pdf, PDF_OPTS);
        // 0.3.45(Shift)·0.3.46(Ctrl) 때 저장된 키는 그때의 기본값이다 — 한 번만 지금 기본(Shift)으로 맞춘다.
        // Ctrl 끌기는 탐색기의 "복사" 라 고리 키로 쓰면 복사하려던 사람 앞에 고리가 뜬다. Alt 를 고른 사람은 그대로.
        if self.ver < 2 && self.key == "ctrl" {
            self.key = "shift".into();
        }
        // 0.3.45~0.3.47 은 기본이 꺼짐이었다 — 그때 저장된 꺼짐은 고른 값이 아니라 기본값이라 한 번 켠다
        if self.ver < 3 {
            self.enabled = true;
        }
        self.ver = SETTINGS_VER;
        if !["shift", "ctrl", "alt"].contains(&self.key.as_str()) {
            self.key = "shift".into();
        }
        self.quality = self.quality.clamp(50, 100);
        self.dpi = if self.dpi >= 300 { 300 } else { 150 };
        if !crate::anim::SPEEDS.contains(&self.gif_ms) {
            self.gif_ms = crate::anim::DEFAULT_MS;
        }
        if !CHARACTERS.contains(&self.character.as_str()) {
            self.character = "cat".into();
        }
        self
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

pub fn settings() -> Settings {
    let mut g = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = g.as_ref() {
        return s.clone();
    }
    let s = crate::prefs::load()
        .get(PREFS_KEY)
        .cloned()
        .and_then(|v| serde_json::from_value::<Settings>(v).ok())
        .unwrap_or_default()
        .clean();
    *g = Some(s.clone());
    s
}

fn save(s: &Settings) -> Result<(), String> {
    crate::prefs::section_set(PREFS_KEY, serde_json::to_value(s).map_err(|e| e.to_string())?)?;
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.clone());
    Ok(())
}

// ── 고리에 무엇을 보일지 ──────────────────────────────────────────────────────

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Seg {
    pub id: String,
    pub label: String,
    pub sub: String,
    /// 마우스가 올라갔을 때 가운데에 쓸 말 (비면 "놓으면 바꿔요")
    pub hint: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// image | audio | pdf | mixed | none
    pub kind: String,
    /// 가운데에 크게 — "JPG 3개"
    pub title: String,
    /// 가운데 작은 글 — "가운데에 놓으면 취소" / 못 바꾸는 이유
    pub note: String,
    pub segs: Vec<Seg>,
}

fn ext(p: &str) -> String {
    let e = Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if e == "jpeg" { "jpg".into() } else { e }
}

fn kind_of(e: &str) -> &'static str {
    if crate::convert::convert::READABLE_EXT.contains(&e) {
        "image"
    } else if ext_convert::AUDIO_EXT.contains(&e) {
        "audio"
    } else if e == "pdf" {
        "pdf"
    } else {
        "none"
    }
}

fn seg_of(id: &str) -> Seg {
    let (label, sub) = match id {
        "png" => ("PNG", "투명 그대로"),
        "jpg" => ("JPG", "가장 흔함"),
        "webp" => ("WEBP", "작은 용량"),
        "bmp" => ("BMP", "압축 없음"),
        "gif" => ("GIF", "움직이는 그림"),
        "square" => ("정사각", "SNS 1080"),
        "small" => ("1MB", "카톡·메일"),
        "mp3" => ("MP3", "192kbps"),
        "wav" => ("WAV", "압축 없음"),
        "jpg-pages" => ("JPG", "쪽마다"),
        "jpg-one" => ("JPG", "한 장으로"),
        "png-pages" => ("PNG", "쪽마다"),
        "png-one" => ("PNG", "한 장으로"),
        _ => (id, ""),
    };
    Seg { id: id.into(), label: label.into(), sub: sub.into(), hint: String::new() }
}

/// GIF 칸 — 여러 장이면 이어 움직이고, 한 장이면 동작 칸이 나뉜 그림일 때 움직인다 (anim.rs)
fn gif_seg(n: usize) -> Seg {
    let mut g = seg_of("gif");
    if n > 1 {
        g.sub = format!("{n}장 이어서");
        g.hint = format!("{n}장을 이어 움직여요");
    } else {
        g.hint = "동작 칸이 있으면 움직여요".into();
    }
    g
}

pub fn plan(paths: &[String], s: &Settings) -> Plan {
    if paths.is_empty() {
        return Plan { kind: "none".into(), title: "파일 없음".into(), note: "파일을 끌어 주세요".into(), segs: vec![] };
    }
    let exts: Vec<String> = paths.iter().map(|p| ext(p)).collect();
    let kinds: Vec<&str> = exts.iter().map(|e| kind_of(e)).collect();
    let n = paths.len();
    let shown_ext = if exts.iter().all(|e| *e == exts[0]) { exts[0].to_uppercase() } else { "파일".into() };
    let title = format!("{shown_ext} {n}개");
    if kinds.iter().any(|k| *k == "none") {
        return Plan { kind: "none".into(), title, note: "그림·소리·PDF 만 바꿀 수 있어요".into(), segs: vec![] };
    }
    if kinds.iter().any(|k| *k != kinds[0]) {
        return Plan { kind: "mixed".into(), title, note: "같은 종류끼리 끌어 주세요".into(), segs: vec![] };
    }
    let kind = kinds[0];
    let ids: Vec<String> = match kind {
        // 이미 그 형식인 것만 있으면 그 칸은 뺀다 (png 만 끌었는데 PNG 칸이 있으면 헷갈린다)
        "image" => s.image.iter().filter(|t| !exts.iter().all(|e| e == *t)).cloned().collect(),
        "audio" => s.audio.iter().filter(|t| !exts.iter().all(|e| e == *t)).cloned().collect(),
        _ => s.pdf.clone(),
    };
    let note = if ids.is_empty() { "설정에서 보일 형식을 골라 주세요".into() } else { "가운데에 놓으면 취소".into() };
    Plan { kind: kind.into(), title, note, segs: ids.iter().map(|i| if i == "gif" { gif_seg(n) } else { seg_of(i) }).collect() }
}

/// 고리 가운데에서 (dx, dy) 논리 px 떨어진 곳이 몇 번째 칸인가. 칸은 12시 방향에서 시계 방향으로.
pub fn hit(dx: f64, dy: f64, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let r = (dx * dx + dy * dy).sqrt();
    if !(R_IN..=R_OUT).contains(&r) {
        return None;
    }
    let deg = dy.atan2(dx).to_degrees(); // 화면 좌표(아래가 +) — 시계 방향으로 커진다
    let t = (deg + 90.0).rem_euclid(360.0);
    Some(((t / (360.0 / n as f64)) as usize).min(n - 1))
}

/// 쓰레기통 위인가 (고리 가운데에서 (dx, dy) 논리 px)
pub fn on_trash(dx: f64, dy: f64) -> bool {
    (dx - TRASH_X).hypot(dy - TRASH_Y) <= EAT_R
}

fn request(target: &str, files: Vec<String>, out_dir: &Path, s: &Settings) -> ExtRequest {
    let mut r = ExtRequest {
        files,
        out_dir: out_dir.to_string_lossy().into(),
        image_format: None,
        audio_format: None,
        quality: Some(s.quality),
        bitrate: Some(192),
        background: None,
        pdf_format: None,
        pdf_mode: None,
        pdf_dpi: Some(s.dpi),
    };
    match target {
        "mp3" | "wav" => r.audio_format = Some(target.into()),
        t if t.contains('-') => {
            let (f, m) = t.split_once('-').unwrap();
            r.pdf_format = Some(f.into());
            r.pdf_mode = Some(if m == "one" { "one".into() } else { "pages".into() });
        }
        t => r.image_format = Some(t.into()),
    }
    r
}

// ── 고리 상태 ────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Ring {
    shown: bool,
    busy: bool,
    preview: bool,
    /// 고리 가운데 (물리 px) · 배율
    cx: i32,
    cy: i32,
    scale: f64,
    paths: Vec<String>,
    plan: Plan,
    hover: Option<usize>,
    /// 쓰레기통 위에 있나
    trash: bool,
    hide_at: Option<Instant>,
    /// 못 찾은 숨은 메뉴 수 · 전체 (고리 가운데 작은 글)
    left: u32,
    total: u32,
    /// 이번에 찾은 숨은 메뉴 — 고리가 닫히면 쪽지 방을 연다
    secret: Option<String>,
}

static RING: Mutex<Option<Ring>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();
/// 놓기를 받는 둥근 창 (HWND). 0 이면 아직 없음
static DROP_HWND: AtomicIsize = AtomicIsize::new(0);

fn ring<R>(f: impl FnOnce(&mut Ring) -> R) -> R {
    let mut g = RING.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Ring::default))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct View {
    shown: bool,
    busy: bool,
    preview: bool,
    plan: Plan,
    hover: Option<usize>,
    trash: bool,
    left: u32,
    total: u32,
    character: String,
}

fn view() -> View {
    let character = settings().character;
    ring(|r| View { shown: r.shown, busy: r.busy, preview: r.preview, plan: r.plan.clone(), hover: r.hover, trash: r.trash, left: r.left, total: r.total, character })
}

/// 고리가 떠 있거나 일하는 중인가 (업데이트 설치를 미룬다) — 쪽지 방이 열려 있어도 쓰는 중
pub fn in_use() -> bool {
    let busy = ring(|r| r.shown || r.busy || r.secret.is_some());
    busy || APP.get().and_then(|a| a.get_webview_window("secret")).is_some()
}

/// 숨은 메뉴 내용이 바뀌었다 (관리자가 새로 올림) — 가운데 숫자를 다시
pub fn refresh_counts() {
    let (left, total) = crate::content::left();
    ring(|r| {
        r.left = left;
        r.total = total;
    });
    push_view();
}

/// 고리 창에 알린다. 앱이 없으면(테스트) 조용히 넘어간다
fn emit<P: Serialize + Clone>(event: &str, payload: P) {
    if let Some(app) = APP.get() {
        let _ = app.emit_to(LABEL, event, payload);
    }
}

fn push_view() {
    emit("qconv:view", view());
}

/// 고리 웹 창 — 켜 둔 사람에게만 만든다 (WebView2 한 벌만큼 메모리를 쓴다)
fn ensure_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(LABEL) {
        return Some(w);
    }
    let url = tauri::WebviewUrl::App("qconv.html".into());
    match tauri::WebviewWindowBuilder::new(app, LABEL, url)
        .title("MAGO 고리")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .visible(false)
        // 놓기는 우리 둥근 창이 받는다 — 이 창은 그림만 (머리말 "창이 둘인 이유")
        .disable_drag_drop_handler()
        .inner_size(SIZE, SIZE)
        .build()
    {
        Ok(w) => {
            let _ = w.set_ignore_cursor_events(true);
            Some(w)
        }
        Err(e) => {
            crate::log(&format!("[MAGO] 고리 창을 못 만듦: {e}"));
            None
        }
    }
}

/// 고리는 **앞으로 나오지 않게**(활성화 없이) 띄운다 — 끌던 탐색기가 포커스를 잃지 않게.
/// Tauri 의 show() 는 창을 활성화할 수 있어서 Win32 로 직접 한다 (숨기기도 같은 길로 — 상태가 어긋나지 않게).
fn show_quiet(w: &tauri::WebviewWindow, on: bool) {
    #[cfg(windows)]
    if let Ok(h) = w.hwnd() {
        // Tauri 가 쓰는 windows 판과 우리 판이 달라도 되게 손잡이 값만 옮긴다
        let h = windows::Win32::Foundation::HWND(h.0 as _);
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE};
        unsafe {
            if on {
                let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
                let _ = SetWindowPos(h, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE);
            } else {
                let _ = ShowWindow(h, SW_HIDE);
            }
        }
        return;
    }
    let _ = if on { w.show() } else { w.hide() };
}

/// (cx, cy) 물리 px 가운데로 고리를 띄운다
fn show_at(app: &AppHandle, cx: i32, cy: i32, preview: bool) {
    let Some(w) = ensure_window(app) else { return };
    let scale = w
        .monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let px = (SIZE * scale).round() as i32;
    let (x, y) = (cx - px / 2, cy - px / 2);
    let (left, total) = crate::content::left();
    // 시각으로 여는 숨은 메뉴 (새벽 손님 등) — 찾았으면 고리가 닫힐 때 쪽지 방을 연다
    use chrono::Timelike;
    let found = if preview { None } else { crate::content::on_open(chrono::Local::now().hour()) };
    let (left, total) = if found.is_some() { crate::content::left() } else { (left, total) };
    ring(|r| {
        r.shown = true;
        r.busy = false;
        r.preview = preview;
        r.cx = cx;
        r.cy = cy;
        r.scale = scale;
        r.paths.clear();
        r.hover = None;
        r.trash = false;
        r.hide_at = None;
        r.left = left;
        r.total = total;
        if let Some(h) = &found {
            r.secret = Some(h.id.clone());
        }
        r.plan = if preview {
            let mut p = plan(&["보기.jpg".to_string()], &settings());
            p.title = "미리 보기".into();
            p.note = "끌 때 이렇게 떠요".into();
            p
        } else {
            Plan { kind: "wait".into(), title: "MAGO".into(), note: "조금 움직여 주세요".into(), segs: vec![] }
        };
    });
    let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = w.set_size(tauri::PhysicalSize::new(px as u32, px as u32));
    push_view();
    if let Some(h) = found {
        emit("qconv:found", h.name);
    }
    show_quiet(&w, true);
    // 미리 보기에는 놓기 창을 깔지 않는다 — 보이지 않는 창이 그 자리 클릭을 3초 동안 먹는다
    #[cfg(windows)]
    if !preview {
        drop_win::place(x, y, px, scale);
    }
}

fn hide() {
    let secret = ring(|r| {
        r.shown = false;
        r.busy = false;
        r.preview = false;
        r.paths.clear();
        r.hover = None;
        r.trash = false;
        r.hide_at = None;
        r.secret.take()
    });
    #[cfg(windows)]
    drop_win::hide();
    if let Some(w) = APP.get().and_then(|a| a.get_webview_window(LABEL)) {
        show_quiet(&w, false);
    }
    push_view();
    if let (Some(id), Some(app)) = (secret, APP.get()) {
        crate::content::show_secret(app, &id);
    }
}

// ── 놓기 받은 뒤 ──────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Prog {
    done: usize,
    total: usize,
    /// 0~1 (PDF 는 쪽까지 셈)
    frac: f64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Done {
    made: usize,
    failed: usize,
    first_error: String,
    /// 원본 폴더에 못 써서 다른 곳에 둔 경우 그 폴더
    moved_to: String,
    /// 결과 한 줄 — "12장 움직이는 GIF"
    note: String,
}

fn run(paths: Vec<String>, target: String) {
    let s = settings();
    let total = paths.len();
    crate::log(&format!("[MAGO] {total}개 → {target}"));
    // 원본 폴더마다 묶어 그 폴더에 저장. 못 쓰는 폴더(읽기 전용 USB 등)면 기본 저장 폴더\확장자변경
    let mut groups: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for p in paths {
        let dir = Path::new(&p).parent().map(Path::to_path_buf).unwrap_or_default();
        match groups.iter_mut().find(|g| g.0 == dir) {
            Some(g) => g.1.push(p),
            None => groups.push((dir, vec![p])),
        }
    }
    let mut out = Done::default();
    let mut base = 0usize;
    for (dir, files) in groups {
        let n = files.len();
        let dir = match crate::outdir::ensure(&dir.to_string_lossy()) {
            Ok(d) => PathBuf::from(d),
            Err(_) => {
                let f = crate::outdir::default_base().join("확장자변경");
                out.moved_to = f.to_string_lossy().into();
                f
            }
        };
        // 정사각(SNS) · 1MB 는 형식 바꾸기가 아니라 actions.rs 가 한 장씩
        if target == "square" || target == "small" {
            for (i, f) in files.iter().enumerate() {
                emit("qconv:progress", Prog { done: base + i, total, frac: (base + i) as f64 / total.max(1) as f64 });
                let r = if target == "square" { crate::actions::square(Path::new(f), &dir) } else { crate::actions::small(Path::new(f), &dir) };
                match r {
                    Ok(_) => out.made += 1,
                    Err(e) => {
                        out.failed += 1;
                        if out.first_error.is_empty() {
                            out.first_error = e;
                        }
                    }
                }
            }
            base += n;
            continue;
        }
        // 움직이는 GIF — 여러 장이면 이어서 하나, 한 장이면 동작 칸을 나눠서. 나눌 칸이 없는 한 장만 아래 보통 GIF 로
        if target == "gif" {
            let made = if n > 1 {
                crate::anim::from_files(&files, &dir, s.gif_ms, &mut |i, _| {
                    emit("qconv:progress", Prog { done: base, total, frac: (base as f64 + i as f64 / n as f64) / total.max(1) as f64 });
                })
                .map(Some)
            } else {
                emit("qconv:progress", Prog { done: base, total, frac: base as f64 / total.max(1) as f64 });
                crate::anim::from_sheet(Path::new(&files[0]), &dir, s.gif_ms)
            };
            match made {
                Ok(Some(m)) => {
                    crate::log(&format!("[MAGO] 움직이는 GIF {}장 · {}KB → {}", m.frames, m.out.bytes / 1024, m.out.file));
                    out.made += 1;
                    out.note = format!("{}장 움직이는 GIF", m.frames);
                    base += n;
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    out.failed += n;
                    if out.first_error.is_empty() {
                        out.first_error = e;
                    }
                    base += n;
                    continue;
                }
            }
        }
        let req = request(&target, files, &dir, &s);
        let mut tick = move |i: usize, _t: usize, cur: &str| {
            // PDF 는 "파일 · 3/40쪽" 으로 온다
            let page = cur.rsplit_once(" · ").and_then(|(_, p)| p.strip_suffix("쪽")).and_then(|p| p.split_once('/')).and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)));
            let f = page.map(|(a, b)| a / b.max(1.0)).unwrap_or(0.0);
            let frac = ((base + i) as f64 + f) / total.max(1) as f64;
            emit("qconv:progress", Prog { done: base + i, total, frac: frac.min(1.0) });
        };
        match ext_convert::run(&req, &mut tick) {
            Ok(r) => {
                out.made += r.outputs.len();
                out.failed += r.errors.len();
                if out.first_error.is_empty() {
                    if let Some(e) = r.errors.first() {
                        out.first_error = e.error.clone();
                    }
                }
            }
            Err(e) => {
                out.failed += n;
                if out.first_error.is_empty() {
                    out.first_error = e;
                }
            }
        }
        base += n;
    }
    crate::log(&format!("[MAGO] 끝 — 만듦 {} · 실패 {} {}", out.made, out.failed, out.first_error));
    emit("qconv:done", out.clone());
    // 결과를 잠깐 보여 주고 닫는다 (실패면 조금 더 길게 — 이유를 읽을 시간)
    let wait = if out.failed > 0 { 3200 } else { 1400 };
    ring(|r| {
        r.busy = false;
        r.hide_at = Some(Instant::now() + Duration::from_millis(wait));
    });
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Ate {
    /// 휴지통으로 간 수 · 그 용량(바이트)
    count: usize,
    bytes: u64,
    /// 지금까지 먹인 횟수 · 파일 수 · 용량
    feeds: u32,
    files_total: u64,
    bytes_total: u64,
    /// 이번에 찾은 숨은 메뉴 이름
    found: Option<String>,
    error: String,
    left: u32,
    total: u32,
}

/// 쓰레기통에 먹였다 → 윈도우 휴지통으로 (되살릴 수 있다) · 먹인 횟수 · 숨은 메뉴
fn feed(paths: Vec<String>) {
    crate::log(&format!("[MAGO] 쓰레기통 {}개", paths.len()));
    let mut a = Ate::default();
    // 크기는 보내기 전에 잰다 — 휴지통으로 간 뒤에는 그 자리에 없다
    let sizes: Vec<(String, u64)> = paths.iter().filter_map(|p| Some((p.clone(), std::fs::metadata(p).ok()?.len()))).collect();
    #[cfg(windows)]
    let gone = crate::actions::recycle(&paths);
    #[cfg(not(windows))]
    let gone: Result<usize, String> = Err("윈도우에서만 돼요".into());
    match gone {
        Ok(k) => {
            // 실제로 사라진 것만 센다 (휴지통이 일부를 못 받았을 수 있다)
            let bytes = sizes.iter().filter(|(p, _)| !Path::new(p).exists()).map(|(_, b)| b).sum();
            let (p, found) = crate::content::on_feed(k, bytes);
            a.count = k;
            a.bytes = bytes;
            a.feeds = p.feeds;
            a.files_total = p.files;
            a.bytes_total = p.bytes;
            if let Some(h) = found {
                a.found = Some(h.name.clone());
                ring(|r| r.secret = Some(h.id));
            }
        }
        Err(e) => a.error = e,
    }
    (a.left, a.total) = crate::content::left();
    crate::log(&format!("[MAGO] 먹음 {}개 {}B · 누적 {}번 {}개 {}B · 찾음 {:?} {}", a.count, a.bytes, a.feeds, a.files_total, a.bytes_total, a.found, a.error));
    // 씹는 모습(약 2초)을 보여 주고 닫는다. 숨은 메뉴를 찾았으면 배부름 → 응가 → 메뉴까지 (약 5초)
    let wait = if a.found.is_some() { 5600 } else if a.error.is_empty() { 2600 } else { 3200 };
    ring(|r| {
        r.left = a.left;
        r.total = a.total;
        r.busy = false;
        r.hide_at = Some(Instant::now() + Duration::from_millis(wait));
    });
    emit("qconv:ate", a);
}

/// 둥근 창이 받은 끌기 소식
fn on_enter(paths: Vec<String>) {
    let p = plan(&paths, &settings());
    ring(|r| {
        r.paths = paths;
        r.plan = p;
        r.preview = false;
    });
    push_view();
}

fn on_over(sx: i32, sy: i32) {
    let changed = ring(|r| {
        let s = if r.scale > 0.0 { r.scale } else { 1.0 };
        let (dx, dy) = ((sx - r.cx) as f64 / s, (sy - r.cy) as f64 / s);
        let h = hit(dx, dy, r.plan.segs.len());
        let t = on_trash(dx, dy);
        let c = h != r.hover || t != r.trash;
        r.hover = h;
        r.trash = t;
        c
    });
    if changed {
        push_view();
    }
}

fn on_leave() {
    ring(|r| {
        r.hover = None;
        r.trash = false;
    });
    push_view();
}

/// 놓았다. 쓰레기통 위면 먹이고, 칸 위면 바꾸기 시작, 아니면 닫기.
fn on_drop(paths: Vec<String>, sx: i32, sy: i32) {
    let (pick, trash) = ring(|r| {
        let s = if r.scale > 0.0 { r.scale } else { 1.0 };
        if !paths.is_empty() {
            r.plan = plan(&paths, &settings());
        }
        let (dx, dy) = ((sx - r.cx) as f64 / s, (sy - r.cy) as f64 / s);
        let h = hit(dx, dy, r.plan.segs.len());
        r.hover = h;
        r.trash = on_trash(dx, dy);
        (h.map(|i| r.plan.segs[i].id.clone()), r.trash)
    });
    if trash && !paths.is_empty() {
        ring(|r| r.busy = true);
        push_view();
        emit("qconv:eat", paths.len());
        std::thread::Builder::new().name("mago-feed".into()).spawn(move || feed(paths)).ok();
        return;
    }
    match pick {
        Some(target) => {
            ring(|r| r.busy = true);
            push_view();
            std::thread::Builder::new()
                .name("qconv-run".into())
                .spawn(move || run(paths, target))
                .ok();
        }
        None => hide(),
    }
}

// ── 놓기를 받는 둥근 창 (Win32 · OLE) ──────────────────────────────────────────

#[cfg(windows)]
mod drop_win {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::{implement, w, Ref};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINTL, WPARAM};
    use windows::Win32::Graphics::Gdi::{CombineRgn, CreateEllipticRgn, DeleteObject, SetWindowRgn, RGN_OR};
    use windows::Win32::System::Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Ole::{IDropTarget, IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium, CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE};
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
    use windows::Win32::UI::WindowsAndMessaging::*;

    fn hwnd() -> Option<HWND> {
        let v = DROP_HWND.load(Ordering::SeqCst);
        (v != 0).then(|| HWND(v as *mut _))
    }

    pub fn place(x: i32, y: i32, px: i32, scale: f64) {
        let Some(h) = hwnd() else { return };
        unsafe {
            // 둥글게 — 고리 바깥 모서리는 아래 창으로 그대로 통과한다. 고리 원 + 쓰레기통 원
            let d = ((super::R_OUT + 6.0) * 2.0 * scale).round() as i32;
            let off = (px - d) / 2;
            let rgn = CreateEllipticRgn(off, off, off + d, off + d);
            let tr = (super::EAT_R * scale).round() as i32;
            let (tx, ty) = (px / 2 + (super::TRASH_X * scale).round() as i32, px / 2 + (super::TRASH_Y * scale).round() as i32);
            let t = CreateEllipticRgn(tx - tr, ty - tr, tx + tr, ty + tr);
            let _ = CombineRgn(Some(rgn), Some(rgn), Some(t), RGN_OR);
            let _ = DeleteObject(t.into());
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), x, y, px, px, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            let _ = SetWindowRgn(h, Some(rgn), true);
        }
    }

    pub fn hide() {
        if let Some(h) = hwnd() {
            unsafe {
                let _ = ShowWindow(h, SW_HIDE);
            }
        }
    }

    unsafe fn paths_of(obj: Ref<'_, IDataObject>) -> Vec<String> {
        let mut out = Vec::new();
        let Some(obj) = obj.as_ref() else { return out };
        let fmt = FORMATETC { cfFormat: CF_HDROP.0, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 };
        if let Ok(mut medium) = obj.GetData(&fmt) {
            let hdrop = HDROP(medium.u.hGlobal.0 as _);
            let n = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
            for i in 0..n {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                let mut buf = vec![0u16; len + 1];
                DragQueryFileW(hdrop, i, Some(&mut buf));
                out.push(OsString::from_wide(&buf[..len]).to_string_lossy().to_string());
            }
            ReleaseStgMedium(&mut medium);
        }
        out
    }

    #[implement(IDropTarget)]
    struct Target;

    #[allow(non_snake_case)]
    impl IDropTarget_Impl for Target_Impl {
        fn DragEnter(&self, obj: Ref<'_, IDataObject>, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> windows::core::Result<()> {
            let paths = unsafe { paths_of(obj) };
            let ok = !paths.is_empty();
            super::on_enter(paths);
            super::on_over(pt.x, pt.y);
            unsafe { *eff = if ok { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
            Ok(())
        }
        fn DragOver(&self, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> windows::core::Result<()> {
            super::on_over(pt.x, pt.y);
            let ok = ring(|r| !r.paths.is_empty());
            unsafe { *eff = if ok { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
            Ok(())
        }
        fn DragLeave(&self) -> windows::core::Result<()> {
            super::on_leave();
            Ok(())
        }
        fn Drop(&self, obj: Ref<'_, IDataObject>, _k: MODIFIERKEYS_FLAGS, pt: &POINTL, eff: *mut DROPEFFECT) -> windows::core::Result<()> {
            let paths = unsafe { paths_of(obj) };
            // **언제나 복사** — "옮김" 으로 답하면 탐색기가 원본을 지울 수 있다 (머리말)
            unsafe { *eff = if paths.is_empty() { DROPEFFECT_NONE } else { DROPEFFECT_COPY } };
            super::on_drop(paths, pt.x, pt.y);
            Ok(())
        }
    }

    extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe {
            match m {
                // 눌러도 앞으로 나오지 않는다 (끌던 탐색기 창이 뒤로 가지 않게)
                WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
                _ => DefWindowProcW(h, m, w, l),
            }
        }
    }

    /// **탐색기가 놓을 때와 같은 셸 데이터**(SHCreateDataObject · CF_HDROP)를 우리 놓기 처리기에 그대로 건넨다.
    /// 마우스를 움직이지 않고 확인하는 것: 칸 위에 놓으면 "복사" 로 답하고 원본 옆에 새 파일이 생기며 원본은 남는다.
    /// 가운데에 놓으면 아무것도 만들지 않는다. (끌기 감지 — 키·끌기 그림 창 — 는 사람 손으로만 확인된다)
    #[cfg(test)]
    mod tests {
        use super::*;
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
        use windows::Win32::UI::Shell::Common::ITEMIDLIST;
        use windows::Win32::UI::Shell::{ILFindLastID, ILFree, SHCreateDataObject, SHParseDisplayName};

        unsafe fn pidl(p: &Path) -> *mut ITEMIDLIST {
            let w: Vec<u16> = p.as_os_str().to_string_lossy().encode_utf16().chain([0]).collect();
            let mut out = std::ptr::null_mut();
            SHParseDisplayName(windows::core::PCWSTR(w.as_ptr()), None, &mut out, 0, None).expect("경로를 셸 항목으로");
            out
        }

        /// 10-20 CEO "쓰레기통에 4개를 넣어도 1개 먹었다고 나와" — 탐색기에서 네 개를 같이 끌면 네 개 다 받는지
        #[test]
        fn a_shell_drag_of_four_files_hands_over_all_four() {
            unsafe {
                let _ = OleInitialize(None);
                *SETTINGS.lock().unwrap() = Some(Settings::default());
                let d = std::env::temp_dir().join(format!("mago-q4-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&d);
                std::fs::create_dir_all(&d).unwrap();
                let mut items = Vec::new();
                for i in 1..=4 {
                    let f = d.join(format!("동작{i}.png"));
                    crate::convert::convert::save(&image::RgbaImage::from_pixel(6, 6, image::Rgba([9, 9, 9, 255])), &f, crate::convert::convert::OutFormat::Png, 90, [255; 4]).unwrap();
                    items.push(pidl(&f));
                }
                let folder = pidl(&d);
                let kids: Vec<*const ITEMIDLIST> = items.iter().map(|p| ILFindLastID(*p) as *const ITEMIDLIST).collect();
                let data: IDataObject = SHCreateDataObject(Some(folder as *const _), Some(&kids), None::<&IDataObject>).expect("셸 끌기 데이터");
                let some = Some(data);
                assert_eq!(paths_of(Ref::from(&some)).len(), 4, "놓기 창이 네 개를 다 받아야 한다");
                for p in items {
                    ILFree(Some(p));
                }
                ILFree(Some(folder));
            }
        }

        #[test]
        fn a_shell_drop_on_a_segment_is_a_copy_and_converts_next_to_the_original() {
            unsafe {
                OleInitialize(None).unwrap();
                *SETTINGS.lock().unwrap() = Some(Settings::default());
                let d = std::env::temp_dir().join(format!("kom-qdrop-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&d);
                std::fs::create_dir_all(&d).unwrap();
                let src = d.join("상품 사진.png");
                crate::convert::convert::save(&image::RgbaImage::from_pixel(12, 12, image::Rgba([200, 30, 30, 255])), &src, crate::convert::convert::OutFormat::Png, 90, [255; 4]).unwrap();

                let folder = pidl(&d);
                let item = pidl(&src);
                let child = ILFindLastID(item) as *const ITEMIDLIST;
                let data: IDataObject = SHCreateDataObject(Some(folder as *const _), Some(&[child]), None::<&IDataObject>).expect("셸 끌기 데이터");

                // 고리를 (1000, 1000) 에 띄운 것처럼
                ring(|r| { r.shown = true; r.cx = 1000; r.cy = 1000; r.scale = 1.0; });
                let target: IDropTarget = Target.into();
                let mut eff = DROPEFFECT(7); // 탐색기는 복사|옮기기|바로가기를 다 허락한 채로 온다
                target.DragEnter(&data, MODIFIERKEYS_FLAGS(0), POINTL { x: 1100, y: 1010 }, &mut eff).unwrap();
                assert_eq!(eff, DROPEFFECT_COPY, "끄는 동안에도 복사로 답해야 한다");
                let segs: Vec<String> = ring(|r| r.plan.segs.iter().map(|g| g.id.clone()).collect());
                assert_eq!(segs, ["jpg", "webp", "gif", "square", "small"], "PNG 를 끌면 PNG 칸은 없다");
                assert_eq!(ring(|r| r.hover), Some(1), "오른쪽 조금 아래는 두 번째 칸(WEBP)");

                // 가운데에 놓으면 아무 일도 없다
                let mut eff = DROPEFFECT(7);
                target.Drop(&data, MODIFIERKEYS_FLAGS(0), POINTL { x: 1003, y: 998 }, &mut eff).unwrap();
                assert_eq!(eff, DROPEFFECT_COPY, "취소여도 '옮김' 으로 답하면 안 된다");
                std::thread::sleep(Duration::from_millis(300));
                assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1, "가운데에 놓으면 새 파일이 없어야 한다");

                // WEBP 칸에 놓기
                ring(|r| { r.shown = true; r.cx = 1000; r.cy = 1000; r.scale = 1.0; });
                target.DragEnter(&data, MODIFIERKEYS_FLAGS(0), POINTL { x: 1100, y: 1010 }, &mut eff).unwrap();
                let mut eff = DROPEFFECT(7);
                target.Drop(&data, MODIFIERKEYS_FLAGS(0), POINTL { x: 1100, y: 1010 }, &mut eff).unwrap();
                assert_eq!(eff, DROPEFFECT_COPY, "놓을 때 '복사' 로 답해야 탐색기가 원본을 지우지 않는다");
                let out = d.join("상품 사진.webp");
                let t = Instant::now();
                while !(out.exists() && !ring(|r| r.busy)) && t.elapsed() < Duration::from_secs(10) {
                    std::thread::sleep(Duration::from_millis(50));
                }
                assert!(out.exists(), "원본 옆에 WEBP 가 생겨야 한다");
                assert_eq!(&std::fs::read(&out).unwrap()[8..12], b"WEBP");
                assert!(src.exists(), "원본은 그대로");
                assert!(ring(|r| r.hide_at.is_some()), "끝나면 잠깐 뒤 닫힐 예정이어야 한다");

                // 쓰레기통 자리에 놓으면 바꾸지 않고 먹이러 간다 (없는 파일이라 휴지통에는 아무것도 안 간다 — 진짜 휴지통은 actions 의 손 확인)
                ring(|r| { r.shown = true; r.busy = false; r.hide_at = None; r.cx = 1000; r.cy = 1000; r.scale = 1.0; });
                let ghost = d.join("이미 없는 파일.png").to_string_lossy().to_string();
                super::super::on_drop(vec![ghost], 1000 + TRASH_X as i32, 1000 + TRASH_Y as i32);
                assert!(ring(|r| r.trash), "쓰레기통 위로 알아봐야 한다");
                let t = Instant::now();
                while ring(|r| r.busy || r.hide_at.is_none()) && t.elapsed() < Duration::from_secs(5) {
                    std::thread::sleep(Duration::from_millis(20));
                }
                assert!(ring(|r| r.hide_at.is_some()), "먹고 나면 잠깐 뒤 닫힐 예정");
                assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2, "쓰레기통은 새 파일을 만들지 않는다");

                ILFree(Some(item));
                ILFree(Some(folder));
                drop(data);
                OleUninitialize();
            }
        }
    }

    /// 전용 스레드 — OLE 놓기는 메시지를 돌리는 STA 스레드에서 받아야 한다
    pub fn start() {
        std::thread::Builder::new()
            .name("qconv-drop".into())
            .spawn(|| unsafe {
                if OleInitialize(None).is_err() {
                    crate::log("[MAGO] OLE 초기화 실패");
                    return;
                }
                let inst = GetModuleHandleW(None).unwrap_or_default();
                let class = w!("MagoDrop");
                let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: inst.into(), lpszClassName: class, ..Default::default() };
                RegisterClassW(&wc);
                let Ok(h) = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
                    class,
                    w!("MAGO 놓기"),
                    WS_POPUP,
                    0, 0, 10, 10,
                    None, None, Some(inst.into()), None,
                ) else {
                    crate::log("[MAGO] 놓기 창을 못 만듦");
                    return;
                };
                // 눈에는 안 보이지만(알파 1/255) 끌기는 받는다. 0 이면 놓기 대상에서 빠질 수 있다
                let _ = SetLayeredWindowAttributes(h, COLORREF(0), 1, LWA_ALPHA);
                let target: IDropTarget = Target.into();
                if let Err(e) = RegisterDragDrop(h, &target) {
                    crate::log(&format!("[MAGO] 놓기 등록 실패: {e}"));
                    return;
                }
                DROP_HWND.store(h.0 as isize, Ordering::SeqCst);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            })
            .ok();
    }
}

// ── 감시 (끌기 + 키) ─────────────────────────────────────────────────────────

#[cfg(windows)]
mod watch {
    use super::*;
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON};
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetAncestor, GetClassNameW, GetCursorPos, GetGUIThreadInfo, GetSystemMetrics, GetWindowThreadProcessId, IsWindowVisible, WindowFromPoint, GA_ROOT, GUITHREADINFO, SM_SWAPBUTTON};

    fn down(vk: u16) -> bool {
        unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
    }

    fn cursor() -> (i32, i32) {
        let mut p = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut p) };
        (p.x, p.y)
    }

    fn class_of(h: HWND) -> String {
        let mut buf = [0u16; 128];
        let n = unsafe { GetClassNameW(h, buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// 끌기를 탐색기·바탕화면·파일 고르기 창에서 시작했나. 그랬으면 그 창의 스레드 번호
    fn from_shell(x: i32, y: i32) -> Option<u32> {
        unsafe {
            let h = WindowFromPoint(POINT { x, y });
            if h.is_null() {
                return None;
            }
            let root = GetAncestor(h, GA_ROOT);
            if !matches!(class_of(root).as_str(), "CabinetWClass" | "ExploreWClass" | "Progman" | "WorkerW" | "#32770") {
                return None;
            }
            Some(GetWindowThreadProcessId(h, std::ptr::null_mut()))
        }
    }

    /// 그 스레드에서 마우스를 잡고 있는 창의 종류 (없으면 빈 글자)
    fn capture_class(thread: u32) -> String {
        unsafe {
            let mut gi: GUITHREADINFO = std::mem::zeroed();
            gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(thread, &mut gi) == 0 || gi.hwndCapture.is_null() {
                return String::new();
            }
            class_of(gi.hwndCapture)
        }
    }

    /// 올가미(영역 선택) 중인가 — 탐색기 항목 창이 마우스를 잡고 있다
    fn lasso(capture: &str) -> bool {
        matches!(capture, "DirectUIHWND" | "SysListView32")
    }

    /// 셸이 파일을 끌 때 띄우는 끌기 그림 창
    fn drag_image() -> bool {
        let class: Vec<u16> = "SysDragImage\0".encode_utf16().collect();
        unsafe {
            let h = FindWindowW(class.as_ptr(), std::ptr::null());
            !h.is_null() && IsWindowVisible(h) != 0
        }
    }

    fn key_vk(k: &str) -> u16 {
        match k {
            "ctrl" => 0x11,
            "alt" => 0x12,
            _ => 0x10,
        }
    }

    pub fn start(app: AppHandle) {
        std::thread::Builder::new()
            .name("qconv-watch".into())
            .spawn(move || {
                let mut was_down = false;
                let mut press = (0, 0);
                let mut moved = false;
                let mut shell: Option<u32> = None;
                // 이번에 키를 누른 동안 이미 띄웠나 (Esc 로 닫은 뒤 키를 쥐고 있어도 다시 안 뜨게 — 키를 떼면 풀린다)
                let mut fired = false;
                // 10-20 CEO "시프트 누르고 있는 동안만 원형 메뉴가 나오도록" — 키를 뗀 때
                let mut key_up_since: Option<Instant> = None;
                let mut up_since: Option<Instant> = None;
                let mut made_window = false;
                let mut last_cur = (i32::MIN, i32::MIN);
                loop {
                    let s = settings();
                    let (shown, busy, hide_at, preview) = ring(|r| (r.shown, r.busy, r.hide_at, r.preview));
                    if let Some(t) = hide_at {
                        if Instant::now() >= t && !busy {
                            hide();
                        }
                    }
                    if !s.enabled {
                        if shown && !preview && !busy {
                            hide();
                        }
                        std::thread::sleep(Duration::from_millis(300));
                        continue;
                    }
                    if !made_window {
                        // 처음 뜰 때 웹 창을 만드느라 늦지 않게 미리
                        made_window = ensure_window(&app).is_some();
                    }
                    let btn = if unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0 { VK_RBUTTON } else { VK_LBUTTON };
                    let is_down = down(btn);
                    let key = down(key_vk(&s.key));
                    let (x, y) = cursor();
                    if is_down && !was_down {
                        press = (x, y);
                        moved = false;
                        fired = false;
                        shell = from_shell(x, y);
                    }
                    if !key {
                        fired = false;
                    }
                    if shown && !busy && !preview && is_down && !key {
                        // 놓을 때 Shift 를 마우스보다 살짝 먼저 떼는 사람이 많다 — 0.25초는 기다린다 (그 사이 놓으면 그대로 바꾼다)
                        let t = *key_up_since.get_or_insert_with(Instant::now);
                        if t.elapsed() > Duration::from_millis(250) {
                            hide();
                            key_up_since = None;
                        }
                    } else {
                        key_up_since = None;
                    }
                    if is_down {
                        up_since = None;
                        if (x - press.0).abs() > 8 || (y - press.1).abs() > 8 {
                            moved = true;
                        }
                        if moved && key && !fired && !shown && !busy {
                            let img = drag_image();
                            let cap = shell.map(capture_class).unwrap_or_default();
                            if img || (shell.is_some() && !lasso(&cap)) {
                                fired = true;
                                crate::log(&format!("[MAGO] 고리 띄움 (끌기 그림={img} 탐색기에서 시작={} 마우스 잡은 창={cap})", shell.is_some()));
                                show_at(&app, x, y, false);
                            }
                        }
                    } else if shown && !busy && !preview && hide_at.is_none() {
                        // 버튼을 뗐는데 놓기 소식이 없으면(고리 밖에 놓음) 잠깐 뒤 닫는다
                        let t = *up_since.get_or_insert_with(Instant::now);
                        if t.elapsed() > Duration::from_millis(700) {
                            hide();
                            up_since = None;
                        }
                    }
                    // 쓰레기통이 커서를 바라보고, 가까워지면 입을 벌린다 (qconv.js) — 고리 밖에서도 알아야 해서 놓기 창이 아니라 여기서
                    if shown && !preview && (x, y) != last_cur {
                        last_cur = (x, y);
                        let (cx, cy, sc) = ring(|r| (r.cx, r.cy, if r.scale > 0.0 { r.scale } else { 1.0 }));
                        emit("qconv:cur", [((x - cx) as f64 / sc).round(), ((y - cy) as f64 / sc).round()]);
                    }
                    if shown && !busy && down(VK_ESCAPE) {
                        hide();
                    }
                    was_down = is_down;
                    std::thread::sleep(Duration::from_millis(if shown || is_down { 15 } else { 40 }));
                }
            })
            .ok();
    }
}

/// 앱이 켜질 때 한 번
pub fn start(app: AppHandle) {
    if APP.set(app.clone()).is_err() {
        return;
    }
    #[cfg(windows)]
    {
        drop_win::start();
        watch::start(app);
    }
}

// ── command ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn qconv_settings_get() -> Settings {
    settings()
}

/// 화면이 바뀐 칸만 보낸다
#[tauri::command]
pub fn qconv_settings_set(patch: Value) -> Result<Settings, String> {
    let mut v = serde_json::to_value(settings()).map_err(|e| e.to_string())?;
    if let (Some(dst), Some(src)) = (v.as_object_mut(), patch.as_object()) {
        for (k, x) in src {
            dst.insert(k.clone(), x.clone());
        }
    }
    let s = serde_json::from_value::<Settings>(v).map_err(|e| format!("설정을 알아보지 못했어요: {e}"))?.clean();
    save(&s)?;
    crate::log(&format!("[MAGO] 설정 — 켬={} 키={}", s.enabled, s.key));
    Ok(s)
}

/// 고리 창이 처음 뜰 때 지금 상태를 받아 간다
#[tauri::command]
pub fn qconv_view() -> Value {
    serde_json::to_value(view()).unwrap_or(Value::Null)
}

/// 설정 화면의 [미리 보기] — 지금 창 가운데에 3.5초
#[tauri::command]
pub async fn qconv_preview(app: AppHandle) -> Result<(), String> {
    let (cx, cy) = app
        .get_webview_window("settings")
        .and_then(|w| Some((w.outer_position().ok()?, w.outer_size().ok()?)))
        .map(|(p, s)| (p.x + s.width as i32 / 2, p.y + s.height as i32 / 2))
        .unwrap_or((800, 500));
    show_at(&app, cx, cy, true);
    ring(|r| r.hide_at = Some(Instant::now() + Duration::from_millis(3500)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_start_at_twelve_and_go_clockwise() {
        // 4칸: 위(0) · 오른쪽(1) · 아래(2) · 왼쪽(3) 을 조금씩 시계 방향으로 비켜서
        assert_eq!(hit(10.0, -100.0, 4), Some(0));
        assert_eq!(hit(100.0, 10.0, 4), Some(1));
        assert_eq!(hit(-10.0, 100.0, 4), Some(2));
        assert_eq!(hit(-100.0, -10.0, 4), Some(3));
        assert_eq!(hit(0.0, 0.0, 4), None, "가운데는 취소");
        assert_eq!(hit(30.0, 30.0, 4), None, "안쪽 원도 취소");
        assert_eq!(hit(300.0, 0.0, 4), None, "고리 바깥도 취소");
        assert_eq!(hit(100.0, 0.0, 0), None, "칸이 없으면 늘 취소");
    }

    #[test]
    fn the_trash_sits_outside_the_ring() {
        assert!(on_trash(TRASH_X, TRASH_Y));
        assert!(on_trash(TRASH_X - 40.0, TRASH_Y + 40.0));
        assert!(!on_trash(0.0, 0.0));
        // 고리 칸과 쓰레기통이 겹치지 않는다 — 고리 바깥 끝(45° 방향)은 쓰레기통 밖
        let edge = R_OUT / std::f64::consts::SQRT_2;
        assert!(!on_trash(edge, edge));
        // 창 안에 다 들어간다
        assert!(TRASH_X + EAT_R < SIZE / 2.0);
    }

    #[test]
    fn plan_matches_the_dragged_kind() {
        let s = Settings::default();
        let p = plan(&["C:\\a\\사진.jpg".into(), "C:\\a\\b.JPEG".into()], &s);
        assert_eq!(p.kind, "image");
        assert_eq!(p.title, "JPG 2개");
        assert!(!p.segs.iter().any(|g| g.id == "jpg"), "전부 JPG 면 JPG 칸은 없다");
        assert_eq!(p.segs.len(), 5);
        let gif = p.segs.iter().find(|g| g.id == "gif").expect("GIF 칸은 기본으로 있다");
        assert_eq!((gif.sub.as_str(), gif.hint.as_str()), ("2장 이어서", "2장을 이어 움직여요"), "여러 장이면 이어 움직인다고 알려 준다");
        let one = plan(&["걷기.png".into()], &s);
        assert_eq!(one.segs.iter().find(|g| g.id == "gif").unwrap().sub, "움직이는 그림");

        let p = plan(&["x.pdf".into()], &s);
        assert_eq!(p.segs.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["jpg-pages", "jpg-one", "png-pages"]);

        let p = plan(&["x.wav".into()], &s);
        assert_eq!(p.segs.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["mp3"]);

        let p = plan(&["a.png".into(), "b.wav".into()], &s);
        assert_eq!(p.kind, "mixed");
        assert!(p.segs.is_empty());

        let p = plan(&["보고서.docx".into()], &s);
        assert_eq!(p.kind, "none");
        assert!(p.segs.is_empty());
    }

    #[test]
    fn settings_drop_unknown_values() {
        let s = Settings { key: "win".into(), image: vec!["gif".into(), "tiff".into(), "png".into()], pdf: vec!["x".into()], quality: 3, dpi: 999, ..Default::default() }.clean();
        assert_eq!(s.key, "shift");
        assert_eq!(s.image, ["png", "gif"], "정해 둔 순서로, 모르는 형식은 빼고");
        assert!(s.pdf.is_empty());
        assert_eq!(s.quality, 50);
        assert_eq!(s.dpi, 300);
    }

    /// 10-19 CEO "컨트롤 드래그는 윈도우의 복사 기능이었네, 시프트로 해야겠다"
    /// — 0.3.45 의 Shift · 0.3.46 의 Ctrl(둘 다 그때 기본값)은 Shift 로, 지금 판에서 직접 고른 Ctrl 은 그대로
    #[test]
    fn saved_defaults_from_older_versions_end_up_on_shift() {
        for (saved, want) in [
            (serde_json::json!({ "enabled": true, "key": "shift" }), "shift"),            // 0.3.45
            (serde_json::json!({ "enabled": true, "key": "ctrl", "ver": 1 }), "shift"),   // 0.3.46
            (serde_json::json!({ "enabled": true, "key": "alt", "ver": 1 }), "alt"),      // 직접 고른 Alt
            (serde_json::json!({ "enabled": true, "key": "ctrl", "ver": 2 }), "ctrl"),    // 0.3.47 에서 고른 Ctrl
        ] {
            let s: Settings = serde_json::from_value(saved.clone()).unwrap();
            let s = s.clean();
            assert_eq!((s.key.as_str(), s.ver, s.enabled), (want, SETTINGS_VER, true), "{saved}");
        }
        assert_eq!(Settings::default().key, "shift");
    }

    /// 10-20 CEO "GIF 로 변환도 가능하게" — 0.1.0 때 저장된 목록에는 GIF 칸을 한 번 더하고, 그 뒤에 끈 사람은 그대로
    #[test]
    fn the_gif_cell_is_added_once_for_old_settings() {
        let old: Settings = serde_json::from_value(serde_json::json!({ "image": ["png", "jpg", "webp", "square", "small"], "ver": 3 })).unwrap();
        assert_eq!(old.clean().image, ["png", "jpg", "webp", "gif", "square", "small"]);
        let off: Settings = serde_json::from_value(serde_json::json!({ "image": ["png", "jpg"], "ver": 4 })).unwrap();
        assert_eq!(off.clean().image, ["png", "jpg"], "지금 판에서 끈 GIF 는 다시 켜지 않는다");
        let odd: Settings = serde_json::from_value(serde_json::json!({ "gifMs": 7, "ver": 4 })).unwrap();
        assert_eq!(odd.clean().gif_ms, crate::anim::DEFAULT_MS, "모르는 빠르기는 보통으로");
    }

    /// 10-20 CEO "고양이 케릭터 추가해서 기본으로 해주고 설정에서 선택할수 있게"
    #[test]
    fn the_cat_is_the_default_character_and_the_trash_can_still_be_picked() {
        assert_eq!(Settings::default().character, "cat");
        let old: Settings = serde_json::from_value(serde_json::json!({ "key": "alt", "ver": 4 })).unwrap();
        assert_eq!(old.clean().character, "cat", "0.1.1 까지 저장된 설정(캐릭터 칸 없음)도 고양이로");
        let trash: Settings = serde_json::from_value(serde_json::json!({ "character": "trash", "ver": 4 })).unwrap();
        assert_eq!(trash.clean().character, "trash", "쓰레기통을 고른 사람은 그대로");
        let odd: Settings = serde_json::from_value(serde_json::json!({ "character": "../x", "ver": 4 })).unwrap();
        assert_eq!(odd.clean().character, "cat", "모르는 이름은 그림 주소에 넣지 않는다");
    }

    /// 10-19 CEO "컴뚝딱이 켜져 있으면 윈도우 폴더에서 사용되는 기능" — 기본 켜짐. 옛 판의 꺼짐(기본값)은 한 번 켜고, 지금 판에서 끈 것은 그대로
    #[test]
    fn on_by_default_and_old_off_defaults_turn_on_once() {
        assert!(Settings::default().enabled);
        assert!(serde_json::from_value::<Settings>(serde_json::json!({ "enabled": false, "ver": 2 })).unwrap().clean().enabled);
        assert!(!serde_json::from_value::<Settings>(serde_json::json!({ "enabled": false, "ver": 3 })).unwrap().clean().enabled, "지금 판에서 끈 사람은 꺼진 채로");
    }

    #[test]
    fn requests_point_at_the_right_converter() {
        let s = Settings::default();
        let d = Path::new("C:\\out");
        let r = request("webp", vec!["a.png".into()], d, &s);
        assert_eq!(r.image_format.as_deref(), Some("webp"));
        let r = request("mp3", vec!["a.wav".into()], d, &s);
        assert_eq!(r.audio_format.as_deref(), Some("mp3"));
        let r = request("png-one", vec!["a.pdf".into()], d, &s);
        assert_eq!((r.pdf_format.as_deref(), r.pdf_mode.as_deref()), (Some("png"), Some("one")));
        let r = request("jpg-pages", vec!["a.pdf".into()], d, &s);
        assert_eq!((r.pdf_format.as_deref(), r.pdf_mode.as_deref()), (Some("jpg"), Some("pages")));
    }

    /// 손으로 돌리는 확인 — 진짜 둥근 놓기 창에 **우리 프로세스 안에서** 끌어다 놓아 본다 대신,
    /// 놓기 처리(on_drop → run)가 원본 옆에 파일을 만들고 원본은 그대로 두는지 본다.
    #[test]
    fn a_drop_converts_next_to_the_original_and_keeps_it() {
        let d = std::env::temp_dir().join(format!("kom-qconv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("상품.png");
        crate::convert::convert::save(&image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255])), &src, crate::convert::convert::OutFormat::Png, 90, [255; 4]).unwrap();
        let req = request("jpg", vec![src.to_string_lossy().into()], &d, &Settings::default());
        let r = ext_convert::run(&req, &mut |_, _, _| {}).unwrap();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(Path::new(&r.outputs[0].file), d.join("상품.jpg"));
        assert!(src.exists(), "원본은 그대로");
    }
}
