//! 문서(Word · 엑셀 · 한글) → PDF (10-24 CEO "jpg, txt, pdf, word, 한글파일, xlsx 를 모두 잡고 … 한 장의 pdf, jpg 로").
//!
//! 이 PC 에 깔린 그 프로그램을 **화면에 띄우지 않고** 불러(COM 자동화) 'PDF 로 저장' 을 시킨다 — 그 프로그램에서 보던 모양 그대로 나온다.
//! 프로그램이 없거나 너무 옛날 판이라 못 열면(한글 2010 은 .hwpx 를 못 연다) docx · xlsx · hwpx 는 **글만 꺼내** PDF 로 만든다 (모양은 빠진다).
//! .doc · .xls · .hwp 는 묶음 파일이 아니라 꺼낼 수 없어 그 프로그램이 있어야 한다.
//!
//! 지키는 것
//! - 원본은 읽기 전용으로 열고 저장하지 않는다. 매크로는 끈 채로 연다(AutomationSecurity = 강제 끔).
//! - 암호 걸린 문서는 암호 창을 띄우지 않고 실패로 돌린다 (엉터리 암호를 넘긴다 — 암호 없는 문서는 무시한다).
//! - 사용자가 이미 켜 둔 Word · 엑셀이 불려 오면 그 창은 건드리지 않는다: 숨기지 않고, 바꾼 설정은 되돌리고, 다른 문서가 열려 있으면 끄지 않는다.
//! - 프로그램이 대답을 안 하면(뜻밖의 창 등) 3분 뒤 포기한다.

use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::time::Duration;

pub const WORD_EXT: &[&str] = &["doc", "docx"];
pub const EXCEL_EXT: &[&str] = &["xls", "xlsx"];
pub const HWP_EXT: &[&str] = &["hwp", "hwpx"];

/// 이 모듈이 PDF 로 바꿀 수 있는 문서인가
pub fn is_doc(ext: &str) -> bool {
    WORD_EXT.contains(&ext) || EXCEL_EXT.contains(&ext) || HWP_EXT.contains(&ext)
}

/// 그 프로그램이 이 PC 에 없다 (com::export 가 돌려주는 표시)
const NO_APP: &str = "\u{1}없음";

/// "Word 가" · "엑셀이" · "한글이" (안내 글)
fn app_subject(ext: &str) -> &'static str {
    if WORD_EXT.contains(&ext) {
        "Word 가"
    } else if EXCEL_EXT.contains(&ext) {
        "엑셀이"
    } else {
        "한글이"
    }
}

/// 프로그램 이름 (안내 글)
fn app_name(ext: &str) -> &'static str {
    if WORD_EXT.contains(&ext) {
        "Word"
    } else if EXCEL_EXT.contains(&ext) {
        "엑셀"
    } else {
        "한글"
    }
}

/// 어떻게 만들었나
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum How {
    /// 그 프로그램으로 — 모양 그대로
    App,
    /// 프로그램이 없어 글만 꺼내서
    TextOnly,
}

/// 문서 하나를 `dst`(PDF)로. 한 번에 하나씩 (프로그램을 여럿 띄우지 않게)
pub fn to_pdf(src: &Path, dst: &Path) -> Result<How, String> {
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !is_doc(&ext) {
        return Err(format!(".{ext} 는 문서로 바꿀 수 없어요"));
    }
    let _ = std::fs::remove_file(dst);
    let by_app = with_timeout(src, dst, &ext).map_err(|e| if e == NO_APP { format!("이 PC 에 {} 없어요", app_subject(&ext)) } else { e });
    match by_app {
        Ok(()) if std::fs::metadata(dst).map(|m| m.len() > 0).unwrap_or(false) => return Ok(How::App),
        Ok(()) => crate::log(&format!("[문서] {} 가 PDF 를 만들지 않음 — {}", app_name(&ext), src.display())),
        Err(ref e) => crate::log(&format!("[문서] {} 로 못 바꿈 — {e}", app_name(&ext))),
    }
    // 프로그램으로 못 했다 → 묶음 파일(docx · xlsx · hwpx)이면 글만 꺼내서
    let app_err = by_app.err().unwrap_or_else(|| format!("{} 가 PDF 를 만들지 않았어요", app_name(&ext)));
    let html = match ext.as_str() {
        "docx" => docx_text(src).map(|t| crate::text::pdf_html(&t, &stem(src))),
        "hwpx" => hwpx_text(src).map(|t| crate::text::pdf_html(&t, &stem(src))),
        "xlsx" => xlsx_sheets(src).map(|s| sheets_html(&s, &stem(src))),
        _ => return Err(format!("{app_err} (.{ext} 는 그 프로그램이 있어야 바꿀 수 있어요)")),
    }
    .map_err(|e| format!("{app_err} · 글도 꺼내지 못했어요: {e}"))?;
    let tmp = std::env::temp_dir().join(format!("mago-doc-{}-{}.html", std::process::id(), chrono::Local::now().timestamp_millis()));
    std::fs::write(&tmp, html).map_err(|e| format!("임시 파일을 만들지 못했어요: {e}"))?;
    let r = crate::pdfprint::print(&tmp, dst);
    let _ = std::fs::remove_file(&tmp);
    r?;
    Ok(How::TextOnly)
}

fn stem(src: &Path) -> String {
    src.file_stem().and_then(|s| s.to_str()).unwrap_or("문서").to_string()
}

/// 전용 스레드에서 프로그램을 부르고, 제때 끝나지 않으면 포기한다 (그 스레드는 버린다).
/// 한글은 늘 숨긴 새 한글을 띄우는데, 숨은 창에 뜬 물음 때문에 멈추면 아무도 끌 수 없다 —
/// 11-04 대표님 PC 에서 .hwp 를 [JPG 한 장] · [PDF 하나로] 에 놓았을 때 3분 동안 멈췄고, 숨은 한글이 남았다.
/// 그래서 한글은 1분만 기다리고, **이번에 새로 뜬 한글만** 끈다 (대표님이 쓰던 한글은 그대로).
fn with_timeout(src: &Path, dst: &Path, ext: &str) -> Result<(), String> {
    if HWP_EXT.contains(&ext) {
        let ext2 = ext.to_string();
        return in_temp(src, dst, move |s, d| guarded(true, move || com::export(&s, &d, &ext2)));
    }
    let (src, dst, ext2) = (src.to_path_buf(), dst.to_path_buf(), ext.to_string());
    guarded(false, move || com::export(&src, &dst, &ext2))
}

/// 한글은 **임시 폴더 밖(D: 드라이브 등)의 파일**을 자동화로 열거나 저장하면 숨은 한글에 확인 창(HNC_DIALOG "한글")을 띄우고 멈춘다
/// (11-09 확인: 같은 Word 파일이 임시 폴더에서는 3초, D:\ 에서는 확인 창에서 멈춤 — 대표님 PC 의 D:\19_test\… 가 그랬다).
/// 그래서 원본을 임시 폴더에 복사해 거기서 바꾸고, 다 된 결과만 `dst` 로 가져온다. 원본은 읽기만 한다
fn in_temp(src: &Path, dst: &Path, job: impl FnOnce(std::path::PathBuf, std::path::PathBuf) -> Result<(), String>) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("mago-hwp-{}-{}", std::process::id(), chrono::Local::now().timestamp_micros()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("임시 폴더를 만들지 못했어요: {e}"))?;
    let name = |p: &Path, alt: &str| p.file_name().map(|f| f.to_os_string()).unwrap_or_else(|| alt.into());
    let (ts, td) = (tmp.join(name(src, "원본")), tmp.join(name(dst, "결과")));
    let r = std::fs::copy(src, &ts).map_err(|e| format!("파일을 읽지 못했어요: {e}")).and_then(|_| job(ts, td.clone())).and_then(|_| {
        std::fs::copy(&td, dst).map(|_| ()).map_err(|e| format!("만든 파일을 옮기지 못했어요: {e}"))
    });
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

fn guarded(hangul: bool, job: impl FnOnce() -> Result<(), String> + Send + 'static) -> Result<(), String> {
    let before = if hangul { hwp_pids() } else { vec![] };
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("mago-office".into())
        .spawn(move || {
            let _ = tx.send(job());
        })
        .map_err(|e| format!("일할 스레드를 만들지 못했어요: {e}"))?;
    let wait = Duration::from_secs(if hangul { 60 } else { 180 });
    match rx.recv_timeout(wait) {
        Ok(r) => r,
        Err(_) if hangul => {
            let ours: Vec<u32> = hwp_pids().into_iter().filter(|p| !before.contains(p)).collect();
            // 숨은 한글에 무슨 창이 떠 있었나 — 다음에 고칠 수 있게 기록
            crate::log(&format!("[문서] 멈춘 한글의 창: {:?}", windows_of(&ours)));
            for p in &ours {
                let _ = std::process::Command::new("taskkill").args(["/F", "/PID", &p.to_string()]).output();
            }
            crate::log(&format!("[문서] 한글이 1분 넘게 대답하지 않아 이번에 띄운 숨은 한글 {ours:?} 를 끔"));
            Err("한글이 1분 넘게 대답하지 않아 그만뒀어요. 이 한글 판으로는 이 파일을 바꾸지 못하는 것 같아요.".to_string())
        }
        Err(_) => Err("프로그램이 3분 넘게 대답하지 않아요. 그 프로그램에 뜬 창이 있는지 봐 주세요.".to_string()),
    }
}

/// 시험: MAGO_DOCX_DIR 의 Word 파일들을 차례로 한글로 — 멈추면 숨은 한글에 뜬 창을 MAGO_DOCX_DIR\창.txt 에 적는다
#[cfg(all(test, windows))]
#[test]
#[ignore = "이 PC 의 한글을 띄운다 (보이지 않게) — 손으로"]
fn word_files_in_a_folder_to_hwp() {
    let Some(dir) = std::env::var_os("MAGO_DOCX_DIR").map(std::path::PathBuf::from) else { return };
    let report = std::sync::Arc::new(Mutex::new(String::new()));
    let before = hwp_pids();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (r2, d2, b2) = (report.clone(), done.clone(), before.clone());
    let watch = std::thread::spawn(move || {
        let mut k = 0;
        while !d2.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
            k += 1;
            if k % 20 == 0 {
                let ours: Vec<u32> = hwp_pids().into_iter().filter(|p| !b2.contains(p)).collect();
                if !ours.is_empty() {
                    r2.lock().unwrap().push_str(&format!("  {}초: {:#?}\n", k / 2, windows_of(&ours)));
                }
            }
        }
    });
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "docx" || e == "doc" || e == "hwp")).collect();
    files.sort();
    // MAGO_LOCK=1 — 첫 파일을 Word 로 열어 둔 채로 (사람이 Word 에서 보고 있을 때와 같게)
    let mut locked = None;
    if std::env::var_os("MAGO_LOCK").is_some() {
        let (tx, rx) = mpsc::channel::<()>();
        let f = files[0].clone();
        let (ok_tx, ok_rx) = mpsc::channel();
        let h = std::thread::spawn(move || com::hold_in_word(&f, ok_tx, rx));
        report.lock().unwrap().push_str(&format!("Word 로 열어 둠: {:?}\n", ok_rx.recv()));
        locked = Some((tx, h));
    }
    let out = dir.join("결과");
    std::fs::create_dir_all(&out).unwrap();
    for f in files {
        let t = std::time::Instant::now();
        report.lock().unwrap().push_str(&format!("▶ {}\n", f.file_name().unwrap().to_string_lossy()));
        // 한글 파일은 PDF 로 (11-04 대표님이 멈췄던 것)
        let r = if f.extension().is_some_and(|e| e == "hwp") {
            let pdf = out.join(format!("{}.pdf", f.file_stem().unwrap().to_string_lossy()));
            to_pdf(&f, &pdf).map(|h| format!("{h:?} {}B", std::fs::metadata(&pdf).map(|m| m.len()).unwrap_or(0)))
        } else {
            to_hwp(&f, &out).map(|o| format!("{}B", o.bytes))
        };
        report.lock().unwrap().push_str(&format!("  → {r:?} ({}초)\n", t.elapsed().as_secs()));
    }
    if let Some((tx, h)) = locked {
        let _ = tx.send(());
        let _ = h.join();
    }
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = watch.join();
    std::fs::write(dir.join("창.txt"), report.lock().unwrap().as_bytes()).unwrap();
}

/// 그 프로세스들의 창과 그 안의 글 ("종류 [제목] { 안의 글 · 단추 }") — 보이지 않는 창도 (읽기만 한다)
pub fn windows_of(pids: &[u32]) -> Vec<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{HWND, LPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible};
        fn text(h: HWND) -> (String, String) {
            let (mut c, mut t) = ([0u16; 128], [0u16; 512]);
            let (cn, tn) = unsafe { (GetClassNameW(h, c.as_mut_ptr(), 128), GetWindowTextW(h, t.as_mut_ptr(), 512)) };
            (String::from_utf16_lossy(&c[..cn.max(0) as usize]), String::from_utf16_lossy(&t[..tn.max(0) as usize]))
        }
        struct Ctx {
            pids: Vec<u32>,
            out: Vec<String>,
        }
        unsafe extern "system" fn child(h: HWND, l: LPARAM) -> i32 {
            let v = &mut *(l as *mut Vec<String>);
            let (c, t) = text(h);
            if !t.trim().is_empty() {
                v.push(format!("{c}:{}", t.trim()));
            }
            1
        }
        unsafe extern "system" fn top(h: HWND, l: LPARAM) -> i32 {
            let ctx = &mut *(l as *mut Ctx);
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, &mut pid);
            if ctx.pids.contains(&pid) {
                let (c, t) = text(h);
                let mut kids: Vec<String> = Vec::new();
                EnumChildWindows(h, Some(child), &mut kids as *mut _ as LPARAM);
                // 이름 없는 숨은 틀 창은 빼고, 보이거나 글이 있는 창만
                if IsWindowVisible(h) != 0 || !t.is_empty() || !kids.is_empty() {
                    ctx.out.push(format!("{c} [{t}] 보임={} {{ {} }}", IsWindowVisible(h) != 0, kids.iter().take(12).cloned().collect::<Vec<_>>().join(" · ")));
                }
            }
            1
        }
        let mut ctx = Ctx { pids: pids.to_vec(), out: Vec::new() };
        unsafe { EnumWindows(Some(top), &mut ctx as *mut _ as LPARAM) };
        ctx.out
    }
    #[cfg(not(windows))]
    {
        let _ = pids;
        vec![]
    }
}

/// 지금 떠 있는 한글(Hwp.exe)들의 프로세스 번호
fn hwp_pids() -> Vec<u32> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("tasklist").args(["/FI", "IMAGENAME eq Hwp.exe", "/FO", "CSV", "/NH"]).creation_flags(0x0800_0000).output();
        let Ok(out) = out else { return vec![] };
        String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split("\",\"").nth(1)?.trim_matches('"').parse().ok()).collect()
    }
    #[cfg(not(windows))]
    vec![]
}

/// 이 PC 에 한글이 깔려 있나 (자동화 이름이 등록돼 있나)
pub fn has_hangul() -> bool {
    #[cfg(windows)]
    {
        winreg::RegKey::predef(winreg::enums::HKEY_CLASSES_ROOT).open_subkey(r"HWPFrame.HwpObject\CLSID").is_ok()
    }
    #[cfg(not(windows))]
    false
}

/// Word 문서(doc · docx) → 한글(HWP) — 이 PC 의 한글이 직접 열어 저장한다 (표 · 칸 합치기가 그대로).
/// 10-29 CEO "시프트드래그해서 hwp로 변환하면 좋겠어 지금 워드내용을 복사해서 한글프로그램에 넣으면 표가 다깨지는 상태야"
pub fn to_hwp(src: &Path, out_dir: &Path) -> Result<crate::convert::OutputFile, String> {
    static ONE: Mutex<()> = Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if !WORD_EXT.contains(&ext.as_str()) {
        return Err(format!(".{ext} 는 한글로 바꿀 수 없어요 (Word 파일만)"));
    }
    if !has_hangul() {
        return Err("이 PC 에 한글이 없어요".into());
    }
    let dst = crate::convert::convert::unique_path(out_dir, &format!("{}.hwp", stem(src)));
    in_temp(src, &dst, |s, d| guarded(true, move || com::to_hwp_job(&s.to_string_lossy(), &d.to_string_lossy())))?;
    let bytes = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
    if bytes == 0 {
        let _ = std::fs::remove_file(&dst);
        return Err("한글이 HWP 를 만들지 않았어요".into());
    }
    crate::log(&format!("[문서] Word → 한글 {} ({}KB)", dst.display(), bytes / 1024));
    Ok(crate::convert::OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: 0, height: 0, bytes, label: "한글".into() })
}

// ── COM 자동화 ────────────────────────────────────────────────────────────────

#[cfg(windows)]
mod com {
    use std::mem::ManuallyDrop;
    use std::path::Path;
    use windows::core::{w, BSTR, GUID, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{SysAllocString, VARIANT_BOOL};
    use windows::Win32::System::Com::{
        CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, DISPATCH_FLAGS,
        DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, EXCEPINFO,
    };
    use windows::Win32::System::Ole::DISPID_PROPERTYPUT;
    use windows::Win32::System::Variant::{VariantClear, VARIANT, VT_BOOL, VT_BSTR, VT_DISPATCH, VT_ERROR, VT_I2, VT_I4};

    /// 사용자 기본 언어 — 프로그램이 오류 글을 그 언어로 준다
    const LOCALE_USER_DEFAULT: u32 = 0x0400;

    /// 암호 걸린 문서가 암호 창을 띄우지 않고 실패하게 넘기는 엉터리 암호.
    /// Word 2010 은 15자가 넘는 암호를 받으면 암호가 없는 문서도 "명령을 정상적으로 종료할 수 없습니다" 로 못 연다 (10-24 확인)
    const NO_PASSWORD: &str = "mago-nopw";

    /// VARIANT — 다 쓰면 풀어 준다
    pub struct Var(VARIANT);
    impl Drop for Var {
        fn drop(&mut self) {
            unsafe {
                let _ = VariantClear(&mut self.0);
            }
        }
    }
    impl Var {
        fn str(s: &str) -> Var {
            let mut v = VARIANT::default();
            unsafe {
                let d = &mut *v.Anonymous.Anonymous;
                d.vt = VT_BSTR;
                // 빈 글은 null 이 아닌 빈 BSTR 로 — 한글은 null 을 받으면 "서버에서 예외 오류" 로 PDF 저장에 실패한다 (10-24 확인)
                d.Anonymous.bstrVal = ManuallyDrop::new(if s.is_empty() { SysAllocString(w!("")) } else { BSTR::from(s) });
            }
            Var(v)
        }
        fn i4(i: i32) -> Var {
            let mut v = VARIANT::default();
            unsafe {
                let d = &mut *v.Anonymous.Anonymous;
                d.vt = VT_I4;
                d.Anonymous.lVal = i;
            }
            Var(v)
        }
        fn bool(b: bool) -> Var {
            let mut v = VARIANT::default();
            unsafe {
                let d = &mut *v.Anonymous.Anonymous;
                d.vt = VT_BOOL;
                d.Anonymous.boolVal = VARIANT_BOOL(if b { -1 } else { 0 });
            }
            Var(v)
        }
        /// 넘기지 않은 선택 인자
        fn missing() -> Var {
            let mut v = VARIANT::default();
            unsafe {
                let d = &mut *v.Anonymous.Anonymous;
                d.vt = VT_ERROR;
                d.Anonymous.scode = 0x8002_0004u32 as i32; // DISP_E_PARAMNOTFOUND
            }
            Var(v)
        }
        fn as_bool(&self) -> Option<bool> {
            unsafe {
                let d = &*self.0.Anonymous.Anonymous;
                match d.vt {
                    VT_BOOL => Some(d.Anonymous.boolVal.0 != 0),
                    VT_I4 => Some(d.Anonymous.lVal != 0),
                    VT_I2 => Some(d.Anonymous.iVal != 0),
                    _ => None,
                }
            }
        }
        fn as_i32(&self) -> Option<i32> {
            unsafe {
                let d = &*self.0.Anonymous.Anonymous;
                match d.vt {
                    VT_I4 => Some(d.Anonymous.lVal),
                    VT_I2 => Some(d.Anonymous.iVal as i32),
                    _ => None,
                }
            }
        }
        /// 다른 COM 개체를 인자로 (Word 의 Range · Cell)
        #[cfg(test)]
        fn from_obj(o: &Obj) -> Var {
            let mut v = VARIANT::default();
            unsafe {
                let d = &mut *v.Anonymous.Anonymous;
                d.vt = VT_DISPATCH;
                d.Anonymous.pdispVal = ManuallyDrop::new(Some(o.0.clone()));
            }
            Var(v)
        }
        #[cfg(test)]
        fn as_string(&self) -> String {
            unsafe {
                let d = &*self.0.Anonymous.Anonymous;
                if d.vt == VT_BSTR { d.Anonymous.bstrVal.to_string() } else { String::new() }
            }
        }
        fn as_disp(&self) -> Option<IDispatch> {
            unsafe {
                let d = &*self.0.Anonymous.Anonymous;
                if d.vt == VT_DISPATCH {
                    (*d.Anonymous.pdispVal).clone()
                } else {
                    None
                }
            }
        }
    }

    pub struct Obj(IDispatch);

    impl Obj {
        fn create(prog_id: &str) -> Result<Obj, String> {
            unsafe {
                let clsid: GUID = CLSIDFromProgID(&HSTRING::from(prog_id)).map_err(|_| super::NO_APP.to_string())?;
                let d: IDispatch = CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER).map_err(|e| format!("켜지 못했어요 ({})", e.message()))?;
                Ok(Obj(d))
            }
        }

        fn invoke(&self, name: &str, flags: DISPATCH_FLAGS, args: Vec<Var>) -> Result<Var, String> {
            unsafe {
                let w = HSTRING::from(name);
                let names = [PCWSTR(w.as_ptr())];
                let mut id = 0i32;
                self.0.GetIDsOfNames(&GUID::zeroed(), names.as_ptr(), 1, LOCALE_USER_DEFAULT, &mut id).map_err(|_| format!("{name} 을(를) 찾지 못했어요"))?;
                // 인자는 거꾸로 넣는다 (COM 약속)
                let mut raw: Vec<VARIANT> = args.iter().rev().map(|a| std::ptr::read(&a.0)).collect();
                let mut put = DISPID_PROPERTYPUT;
                let params = DISPPARAMS {
                    rgvarg: if raw.is_empty() { std::ptr::null_mut() } else { raw.as_mut_ptr() },
                    rgdispidNamedArgs: if flags == DISPATCH_PROPERTYPUT { &mut put } else { std::ptr::null_mut() },
                    cArgs: raw.len() as u32,
                    cNamedArgs: if flags == DISPATCH_PROPERTYPUT { 1 } else { 0 },
                };
                let mut out = Var(VARIANT::default());
                let mut ex = EXCEPINFO::default();
                let r = self.0.Invoke(id, &GUID::zeroed(), LOCALE_USER_DEFAULT, flags, &params, Some(&mut out.0), Some(&mut ex), None);
                // raw 는 args 를 그대로 베낀 것이라 풀기는 args(Var) 쪽에서 한 번만 한다
                std::mem::forget(raw);
                drop(args);
                r.map_err(|e| {
                    let d = ex.bstrDescription.to_string();
                    if d.trim().is_empty() { format!("{name}: {}", e.message()) } else { format!("{name}: {}", d.trim()) }
                })?;
                Ok(out)
            }
        }
        fn call(&self, name: &str, args: Vec<Var>) -> Result<Var, String> {
            self.invoke(name, DISPATCH_METHOD | DISPATCH_PROPERTYGET, args)
        }
        fn get(&self, name: &str) -> Result<Var, String> {
            self.invoke(name, DISPATCH_PROPERTYGET, vec![])
        }
        fn put(&self, name: &str, v: Var) -> Result<(), String> {
            self.invoke(name, DISPATCH_PROPERTYPUT, vec![v]).map(|_| ())
        }
        fn obj(&self, name: &str, args: Vec<Var>) -> Result<Obj, String> {
            self.call(name, args)?.as_disp().map(Obj).ok_or_else(|| format!("{name} 을(를) 받지 못했어요"))
        }
    }

    pub fn export(src: &Path, dst: &Path, ext: &str) -> Result<(), String> {
        let inited = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        let src = std::path::absolute(src).map_err(|e| format!("파일 위치를 알 수 없어요: {e}"))?;
        let dst = std::path::absolute(dst).map_err(|e| format!("저장할 곳을 알 수 없어요: {e}"))?;
        let (s, d) = (src.to_string_lossy().to_string(), dst.to_string_lossy().to_string());
        let r = if super::WORD_EXT.contains(&ext) {
            word(&s, &d)
        } else if super::EXCEL_EXT.contains(&ext) {
            excel(&s, &d)
        } else {
            hwp(&s, &d, ext == "hwpx")
        };
        if inited {
            unsafe { CoUninitialize() };
        }
        r
    }

    fn word(src: &str, dst: &str) -> Result<(), String> {
        let app = Obj::create("Word.Application").map_err(|e| if e == super::NO_APP { e } else { format!("Word 를 {e}") })?;
        // 이미 켜져 있던 Word 일 수 있다 — 바꾼 설정은 되돌린다
        let alerts = app.get("DisplayAlerts").ok();
        let security = app.get("AutomationSecurity").ok();
        let _ = app.put("DisplayAlerts", Var::i4(0));
        let _ = app.put("AutomationSecurity", Var::i4(3));
        let r = (|| {
            let docs = app.obj("Documents", vec![])?;
            // Open(FileName, ConfirmConversions, ReadOnly, AddToRecentFiles, PasswordDocument)
            let doc = docs.obj("Open", vec![Var::str(src), Var::bool(false), Var::bool(true), Var::bool(false), Var::str(NO_PASSWORD)])?;
            // 17 = wdExportFormatPDF
            let r = doc.call("ExportAsFixedFormat", vec![Var::str(dst), Var::i4(17), Var::bool(false)]);
            let _ = doc.call("Close", vec![Var::i4(0)]);
            r.map(|_| ())
        })();
        if let Some(v) = alerts {
            let _ = app.put("DisplayAlerts", v);
        }
        if let Some(v) = security {
            let _ = app.put("AutomationSecurity", v);
        }
        // 사용자가 쓰던 문서가 열려 있으면 끄지 않는다
        let open = app.obj("Documents", vec![]).and_then(|d| d.get("Count")).ok().and_then(|c| c.as_i32()).unwrap_or(1);
        if open == 0 {
            let _ = app.call("Quit", vec![Var::i4(0)]);
        }
        r
    }

    /// 그림을 Word 에 넣는다 — 켜져 있는 Word 의 지금 문서 커서 자리에, 문서가 없으면 새 문서에. Word 를 보이게 하고 앞으로 (web.rs)
    pub fn word_picture(pic: &Path) -> Result<(), String> {
        let inited = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        let r = (|| {
            let pic = std::path::absolute(pic).map_err(|e| format!("파일 위치를 알 수 없어요: {e}"))?;
            let app = running("Word.Application").map_or_else(|| Obj::create("Word.Application"), Ok).map_err(|e| if e == super::NO_APP { "이 PC 에 Word 가 없어요".to_string() } else { format!("Word 를 {e}") })?;
            let docs = app.obj("Documents", vec![])?;
            if docs.get("Count")?.as_i32().unwrap_or(0) == 0 {
                docs.obj("Add", vec![])?;
            }
            let _ = app.put("Visible", Var::bool(true));
            // AddPicture(FileName, LinkToFile, SaveWithDocument) — 파일과 이어 두지 않고 문서 안에 넣는다. 쪽보다 크면 Word 가 쪽 폭에 맞춘다
            app.obj("Selection", vec![])?.obj("InlineShapes", vec![])?.call("AddPicture", vec![Var::str(&pic.to_string_lossy()), Var::bool(false), Var::bool(true)])?;
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(windows::Win32::UI::WindowsAndMessaging::ASFW_ANY);
            }
            let _ = app.call("Activate", vec![]);
            Ok(())
        })();
        if inited {
            unsafe { CoUninitialize() };
        }
        r
    }

    /// 시험: 보이지 않는 새 Word 로 word_picture 와 같은 순서(새 문서 → 커서 자리에 AddPicture)를 밟고 docx 로 저장해 본다.
    /// Word 가 켜져 있으면 그 Word 에 붙을 수 있어 하지 않는다
    #[cfg(test)]
    #[test]
    #[ignore = "이 PC 의 Word 를 띄운다 (보이지 않게) — Word 가 꺼져 있을 때 손으로"]
    fn word_puts_the_picture_in_the_document() {
        assert!(running("Word.Application").is_none(), "Word 가 켜져 있으면 하지 않는다");
        let dir = std::env::temp_dir().join(format!("mago-word-pic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pic = dir.join("그림.png");
        image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 30, 60, 255])).save(&pic).unwrap();
        let out = dir.join("결과.docx");
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let app = Obj::create("Word.Application").unwrap();
        let _ = app.put("DisplayAlerts", Var::i4(0));
        let doc = app.obj("Documents", vec![]).unwrap().obj("Add", vec![]).unwrap();
        app.obj("Selection", vec![]).unwrap().obj("InlineShapes", vec![]).unwrap().call("AddPicture", vec![Var::str(&pic.to_string_lossy()), Var::bool(false), Var::bool(true)]).unwrap();
        let n = doc.obj("InlineShapes", vec![]).unwrap().get("Count").unwrap().as_i32();
        // 16 = wdFormatDocumentDefault (docx)
        doc.call("SaveAs2", vec![Var::str(&out.to_string_lossy()), Var::i4(16)]).or_else(|_| doc.call("SaveAs", vec![Var::str(&out.to_string_lossy()), Var::i4(16)])).unwrap();
        let _ = doc.call("Close", vec![Var::i4(0)]);
        let _ = app.call("Quit", vec![Var::i4(0)]);
        assert_eq!(n, Some(1), "그림 하나가 문서에 들어가야 한다");
        let z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        assert!(z.file_names().any(|f| f.starts_with("word/media/")), "docx 안에 그림이 들어 있어야 한다 (파일과 이어 둔 것이 아니라)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 시험: 표 있는 docx 를 Word 로 만들고(보이지 않게) → 한글이 열어 HWP 로 저장 → 다시 열어 표 수를 센다
    #[cfg(test)]
    #[test]
    #[ignore = "이 PC 의 Word · 한글을 띄운다 (보이지 않게) — Word 가 꺼져 있을 때 손으로"]
    fn hangul_turns_a_word_table_into_hwp() {
        assert!(running("Word.Application").is_none(), "Word 가 켜져 있으면 하지 않는다");
        let dir = std::env::temp_dir().join(format!("mago-word-hwp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let docx = dir.join("표 시험.docx");
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let app = Obj::create("Word.Application").unwrap();
        let doc = app.obj("Documents", vec![]).unwrap().obj("Add", vec![]).unwrap();
        let sel = app.obj("Selection", vec![]).unwrap();
        sel.call("TypeText", vec![Var::str("평가 표")]).unwrap();
        sel.call("TypeParagraph", vec![]).unwrap();
        let range = sel.obj("Range", vec![]).unwrap();
        let tbl = doc.obj("Tables", vec![]).unwrap().obj("Add", vec![Var::from_obj(&range), Var::i4(3), Var::i4(4)]).unwrap();
        let _ = tbl.obj("Borders", vec![]).and_then(|b| b.put("Enable", Var::bool(true)));
        for r in 1..=3 {
            for c in 1..=4 {
                tbl.obj("Cell", vec![Var::i4(r), Var::i4(c)]).unwrap().obj("Range", vec![]).unwrap().put("Text", Var::str(&format!("칸{r}-{c}"))).unwrap();
            }
        }
        // 첫 줄 두 칸 합치기 — 깨지기 쉬운 것
        tbl.obj("Cell", vec![Var::i4(1), Var::i4(1)]).unwrap().call("Merge", vec![Var::from_obj(&tbl.obj("Cell", vec![Var::i4(1), Var::i4(2)]).unwrap())]).unwrap();
        doc.call("SaveAs2", vec![Var::str(&docx.to_string_lossy()), Var::i4(16)]).or_else(|_| doc.call("SaveAs", vec![Var::str(&docx.to_string_lossy()), Var::i4(16)])).unwrap();
        let _ = doc.call("Close", vec![Var::i4(0)]);
        let _ = app.call("Quit", vec![Var::i4(0)]);

        // MAGO 가 실제로 부르는 길 (1분 지킴이 · 원본 옆 · 같은 이름이면 (2))
        let made = crate::office::to_hwp(&docx, &dir).unwrap();
        eprintln!("한글 저장: {} · {}B", made.file, made.bytes);
        let hwp_out = std::path::PathBuf::from(&made.file);
        assert_eq!(hwp_out.file_name().unwrap(), "표 시험.hwp");
        let again = crate::office::to_hwp(&docx, &dir).unwrap();
        assert!(again.file.ends_with("표 시험 (2).hwp"), "같은 이름이 있으면 (2): {}", again.file);
        assert!(crate::office::to_hwp(&dir.join("없음.txt"), &dir).is_err(), "Word 파일만");
        let n = hwp_tables(&hwp_out.to_string_lossy()).unwrap();
        eprintln!("표 {n}개");
        assert_eq!(n, 1, "표 하나가 표로 남아야 한다");
        // MAGO_KEEP=<폴더> 면 눈으로 볼 수 있게 남긴다
        if let Some(keep) = std::env::var_os("MAGO_KEEP") {
            std::fs::copy(&hwp_out, std::path::Path::new(&keep).join("표 시험.hwp")).unwrap();
            std::fs::copy(&docx, std::path::Path::new(&keep).join("표 시험.docx")).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 시험: 보이지 않는 새 Word 로 그 파일을 열어 두었다가 `stop` 이 오면 닫는다
    #[cfg(test)]
    pub fn hold_in_word(f: &Path, ok: std::sync::mpsc::Sender<Result<(), String>>, stop: std::sync::mpsc::Receiver<()>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let app = match Obj::create("Word.Application") {
            Ok(a) => a,
            Err(e) => {
                let _ = ok.send(Err(e));
                return;
            }
        };
        let doc = app.obj("Documents", vec![]).and_then(|d| d.obj("Open", vec![Var::str(&f.to_string_lossy())]));
        let _ = ok.send(doc.as_ref().map(|_| ()).map_err(|e| e.clone()));
        let _ = stop.recv();
        if let Ok(d) = doc {
            let _ = d.call("Close", vec![Var::i4(0)]);
        }
        let _ = app.call("Quit", vec![Var::i4(0)]);
    }

    /// 한글 문서 안의 표 수 (시험용)
    #[cfg(test)]
    fn hwp_tables(path: &str) -> Result<usize, String> {
        let app = Obj::create("HWPFrame.HwpObject")?;
        let _ = app.call("RegisterModule", vec![Var::str("FilePathCheckDLL"), Var::str("FilePathCheckerModule")]);
        if let Ok(w) = app.obj("XHwpWindows", vec![]).and_then(|w| w.obj("Item", vec![Var::i4(0)])) {
            let _ = w.put("Visible", Var::bool(false));
        }
        let r = (|| {
            app.call("Open", vec![Var::str(path), Var::str("HWP"), Var::str("forceopen:true")])?;
            let mut n = 0;
            let mut c = app.get("HeadCtrl")?.as_disp().map(Obj);
            while let Some(ctrl) = c {
                if ctrl.get("CtrlID").map(|v| v.as_string() == "tbl").unwrap_or(false) {
                    n += 1;
                }
                c = ctrl.get("Next").ok().and_then(|v| v.as_disp()).map(Obj);
            }
            Ok(n)
        })();
        let _ = app.call("Clear", vec![Var::i4(1)]);
        let _ = app.call("Quit", vec![]);
        r
    }

    /// 이미 켜져 있는 그 프로그램 (없으면 None)
    fn running(prog_id: &str) -> Option<Obj> {
        use windows::core::{IUnknown, Interface};
        unsafe {
            let clsid: GUID = CLSIDFromProgID(&HSTRING::from(prog_id)).ok()?;
            let mut unk: Option<IUnknown> = None;
            windows::Win32::System::Ole::GetActiveObject(&clsid, None, &mut unk).ok()?;
            unk?.cast::<IDispatch>().ok().map(Obj)
        }
    }

    fn excel(src: &str, dst: &str) -> Result<(), String> {
        let app = Obj::create("Excel.Application").map_err(|e| if e == super::NO_APP { e } else { format!("엑셀을 {e}") })?;
        let alerts = app.get("DisplayAlerts").ok();
        let security = app.get("AutomationSecurity").ok();
        let _ = app.put("DisplayAlerts", Var::bool(false));
        let _ = app.put("AutomationSecurity", Var::i4(3));
        let r = (|| {
            let books = app.obj("Workbooks", vec![])?;
            // Open(Filename, UpdateLinks, ReadOnly, Format, Password)
            let book = books.obj("Open", vec![Var::str(src), Var::i4(0), Var::bool(true), Var::missing(), Var::str(NO_PASSWORD)])?;
            // 0 = xlTypePDF — 통합 문서의 모든 시트
            let r = book.call("ExportAsFixedFormat", vec![Var::i4(0), Var::str(dst)]);
            let _ = book.call("Close", vec![Var::bool(false)]);
            r.map(|_| ())
        })();
        if let Some(v) = alerts {
            let _ = app.put("DisplayAlerts", v);
        }
        if let Some(v) = security {
            let _ = app.put("AutomationSecurity", v);
        }
        let open = app.obj("Workbooks", vec![]).and_then(|d| d.get("Count")).ok().and_then(|c| c.as_i32()).unwrap_or(1);
        if open == 0 {
            let _ = app.call("Quit", vec![]);
        }
        r
    }

    /// to_hwp_com 을 COM 을 연 스레드에서 (office::to_hwp 의 guarded 가 부른다)
    pub fn to_hwp_job(src: &str, dst: &str) -> Result<(), String> {
        let inited = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        let r = to_hwp_com(src, dst);
        if inited {
            unsafe { CoUninitialize() };
        }
        r
    }

    /// Word 문서(doc · docx)를 **한글이 직접 열어** HWP 로 저장한다 — 표 · 칸 합치기 · 테두리를 한글의 변환기가 옮긴다
    /// (Word 에서 복사해 붙이면 클립보드의 HTML 만 건너가 표가 깨진다 — 10-29 CEO). 한글은 숨긴 새 창으로, 다 하면 끈다
    pub fn to_hwp_com(src: &str, dst: &str) -> Result<(), String> {
        let app = Obj::create("HWPFrame.HwpObject").map_err(|e| if e == super::NO_APP { e } else { format!("한글을 {e}") })?;
        let _ = app.call("RegisterModule", vec![Var::str("FilePathCheckDLL"), Var::str("FilePathCheckerModule")]);
        if let Ok(w) = app.obj("XHwpWindows", vec![]).and_then(|w| w.obj("Item", vec![Var::i4(0)])) {
            let _ = w.put("Visible", Var::bool(false));
        }
        let r = (|| {
            // 형식 이름은 판마다 다르다 — 비우면 한글이 알아본다. 안 되면 Word 형식 이름으로
            let mut why = String::new();
            for fmt in ["", "MSWORD", "DOCX", "OOXML"] {
                match app.call("Open", vec![Var::str(src), Var::str(fmt), Var::str("forceopen:true;suspendpassword:true;versionwarning:false")]) {
                    Ok(v) if v.as_bool().unwrap_or(true) => {
                        why.clear();
                        break;
                    }
                    Ok(_) => why = "한글이 이 Word 파일을 열지 못했어요".into(),
                    Err(e) => why = e,
                }
            }
            if !why.is_empty() {
                return Err(why);
            }
            let saved = app.call("SaveAs", vec![Var::str(dst), Var::str("HWP"), Var::str("")])?.as_bool().unwrap_or(true);
            if saved { Ok(()) } else { Err("한글이 HWP 로 저장하지 못했어요".to_string()) }
        })();
        let _ = app.call("Clear", vec![Var::i4(1)]);
        let _ = app.call("Quit", vec![]);
        r
    }

    fn hwp(src: &str, dst: &str, hwpx: bool) -> Result<(), String> {
        let app = Obj::create("HWPFrame.HwpObject").map_err(|e| if e == super::NO_APP { e } else { format!("한글을 {e}") })?;
        // 한글 자동화는 늘 새 한글을 띄운다 — 창을 숨기고, 다 하면 끈다
        let _ = app.call("RegisterModule", vec![Var::str("FilePathCheckDLL"), Var::str("FilePathCheckerModule")]);
        if let Ok(w) = app.obj("XHwpWindows", vec![]).and_then(|w| w.obj("Item", vec![Var::i4(0)])) {
            let _ = w.put("Visible", Var::bool(false));
        }
        let r = (|| {
            let opened = app
                .call("Open", vec![Var::str(src), Var::str(if hwpx { "HWPX" } else { "HWP" }), Var::str("forceopen:true;suspendpassword:true;versionwarning:false")])?
                .as_bool()
                .unwrap_or(false);
            if !opened {
                return Err(if hwpx { "이 한글은 .hwpx 를 열지 못해요 (한글 2014 이후 판이 필요해요)".to_string() } else { "한글이 이 파일을 열지 못했어요".to_string() });
            }
            let saved = app.call("SaveAs", vec![Var::str(dst), Var::str("PDF"), Var::str("")])?.as_bool().unwrap_or(true);
            if saved { Ok(()) } else { Err("한글이 PDF 로 저장하지 못했어요".to_string()) }
        })();
        let _ = app.call("Clear", vec![Var::i4(1)]);
        let _ = app.call("Quit", vec![]);
        r
    }
}

/// 그림을 Word 문서에 넣고 Word 를 앞으로 (켜져 있으면 지금 문서의 커서 자리, 아니면 새 문서)
pub fn word_picture(pic: &Path) -> Result<(), String> {
    com::word_picture(pic)
}

#[cfg(not(windows))]
mod com {
    pub fn to_hwp_job(_: &str, _: &str) -> Result<(), String> {
        Err("Windows 에서만 돼요".into())
    }
    pub fn word_picture(_: &std::path::Path) -> Result<(), String> {
        Err("Windows 에서만 돼요".into())
    }
    pub fn export(_: &std::path::Path, _: &std::path::Path, _: &str) -> Result<(), String> {
        Err("Windows 에서만 돼요".into())
    }
}

// ── 프로그램이 없을 때: 글만 꺼내기 ─────────────────────────────────────────────

use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;

/// 너무 큰 문서 조각은 읽지 않는다 (압축 폭탄)
const MAX_PART: u64 = 64 * 1024 * 1024;

fn zip_part(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<String, String> {
    let f = z.by_name(name).map_err(|_| format!("{name} 이(가) 없어요"))?;
    let mut s = String::new();
    f.take(MAX_PART).read_to_string(&mut s).map_err(|e| format!("{name} 을(를) 읽지 못했어요: {e}"))?;
    Ok(s)
}

fn open_zip(src: &Path) -> Result<zip::ZipArchive<std::fs::File>, String> {
    let f = std::fs::File::open(src).map_err(|e| format!("파일을 열지 못했어요: {e}"))?;
    zip::ZipArchive::new(f).map_err(|_| "문서 파일이 깨졌거나 다른 형식이에요".to_string())
}

fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// 문단마다 줄을 바꾸며 글자만 모은다 — `para` 가 끝나면 줄바꿈, `text` 안의 글자, `tab` · `br` 은 탭 · 줄바꿈
fn flow_text(xml: &str, para: &str, text: &str, tab: &[&str], br: &[&str]) -> Result<String, String> {
    let mut r = Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = 0usize;
    loop {
        match r.read_event().map_err(|e| format!("문서 안 XML 을 읽지 못했어요: {e}"))? {
            Event::Start(e) => {
                if local(e.name().as_ref()) == text {
                    in_text += 1;
                }
            }
            Event::Empty(e) => {
                let qn = e.name();
                let n = local(qn.as_ref());
                if tab.contains(&n) {
                    out.push('\t');
                } else if br.contains(&n) {
                    out.push('\n');
                } else if n == para {
                    out.push('\n');
                }
            }
            Event::End(e) => {
                let qn = e.name();
                let n = local(qn.as_ref());
                if n == text {
                    in_text = in_text.saturating_sub(1);
                } else if n == para {
                    out.push('\n');
                }
            }
            Event::Text(t) if in_text > 0 => out.push_str(&t.xml10_content()),
            Event::GeneralRef(g) if in_text > 0 => {
                if let Ok(Some(c)) = g.resolve_char_ref() {
                    out.push(c);
                } else if let Some(s) = quick_xml::escape::resolve_predefined_entity(&g.xml10_content()) {
                    out.push_str(s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

pub fn docx_text(src: &Path) -> Result<String, String> {
    let mut z = open_zip(src)?;
    let xml = zip_part(&mut z, "word/document.xml")?;
    flow_text(&xml, "p", "t", &["tab"], &["br", "cr"])
}

pub fn hwpx_text(src: &Path) -> Result<String, String> {
    let mut z = open_zip(src)?;
    // Contents/section0.xml, section1.xml … 번호 순서로
    let mut secs: Vec<(u32, String)> = z
        .file_names()
        .filter_map(|n| Some((n.strip_prefix("Contents/section")?.strip_suffix(".xml")?.parse().ok()?, n.to_string())))
        .collect();
    secs.sort();
    if secs.is_empty() {
        return Err("한글 문서 본문이 없어요".into());
    }
    let mut out = String::new();
    for (_, n) in secs {
        let xml = zip_part(&mut z, &n)?;
        out.push_str(&flow_text(&xml, "p", "t", &["tab"], &["lineBreak"])?);
    }
    Ok(out)
}

/// 엑셀 시트 (이름, 줄마다 칸들)
pub type Sheet = (String, Vec<Vec<String>>);

/// "BC12" → 54번째 칸(0부터)
fn col_index(r: &str) -> Option<usize> {
    let letters: String = r.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if letters.is_empty() {
        return None;
    }
    Some(letters.bytes().fold(0usize, |a, b| a * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize) - 1)
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find(|a| local(a.key.as_ref()) == key).and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.to_string()))
}

pub fn xlsx_sheets(src: &Path) -> Result<Vec<Sheet>, String> {
    let mut z = open_zip(src)?;
    // 공유 글자들 (si 하나에 t 여럿 — 서식이 섞인 글)
    let mut shared: Vec<String> = Vec::new();
    if let Ok(xml) = zip_part(&mut z, "xl/sharedStrings.xml") {
        let mut r = Reader::from_str(&xml);
        let (mut cur, mut in_t, mut in_rph) = (String::new(), false, false);
        loop {
            match r.read_event().map_err(|e| format!("엑셀 글자표를 읽지 못했어요: {e}"))? {
                Event::Start(e) => match local(e.name().as_ref()) {
                    "si" => cur.clear(),
                    "t" => in_t = true,
                    "rPh" => in_rph = true, // 일본어 후리가나는 빼고
                    _ => {}
                },
                Event::Empty(e) if local(e.name().as_ref()) == "si" => shared.push(String::new()),
                Event::End(e) => match local(e.name().as_ref()) {
                    "si" => shared.push(std::mem::take(&mut cur)),
                    "t" => in_t = false,
                    "rPh" => in_rph = false,
                    _ => {}
                },
                Event::Text(t) if in_t && !in_rph => cur.push_str(&t.xml10_content()),
                Event::GeneralRef(g) if in_t && !in_rph => {
                    if let Ok(Some(c)) = g.resolve_char_ref() {
                        cur.push(c);
                    } else if let Some(s) = quick_xml::escape::resolve_predefined_entity(&g.xml10_content()) {
                        cur.push_str(s);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    // 시트 순서와 이름은 workbook.xml, 파일 자리는 workbook.xml.rels
    let rels = zip_part(&mut z, "xl/_rels/workbook.xml.rels").unwrap_or_default();
    let mut targets: Vec<(String, String)> = Vec::new();
    let mut r = Reader::from_str(&rels);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if local(e.name().as_ref()) == "Relationship" => {
                if let (Some(id), Some(t)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    let t = t.trim_start_matches('/').to_string();
                    targets.push((id, if t.starts_with("xl/") { t } else { format!("xl/{t}") }));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let book = zip_part(&mut z, "xl/workbook.xml")?;
    let mut order: Vec<(String, String)> = Vec::new();
    let mut r = Reader::from_str(&book);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if local(e.name().as_ref()) == "sheet" => {
                let name = attr(&e, "name").unwrap_or_default();
                if let Some(path) = attr(&e, "id").and_then(|id| targets.iter().find(|t| t.0 == id).map(|t| t.1.clone())) {
                    order.push((name, path));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut sheets = Vec::new();
    for (name, path) in order {
        let Ok(xml) = zip_part(&mut z, &path) else { continue };
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut r = Reader::from_str(&xml);
        let (mut col, mut kind, mut val, mut in_v) = (0usize, String::new(), String::new(), false);
        let mut row: Vec<String> = Vec::new();
        loop {
            match r.read_event().map_err(|e| format!("시트 {name} 을(를) 읽지 못했어요: {e}"))? {
                Event::Start(e) => match local(e.name().as_ref()) {
                    "row" => row.clear(),
                    "c" => {
                        col = attr(&e, "r").and_then(|r| col_index(&r)).unwrap_or(row.len());
                        kind = attr(&e, "t").unwrap_or_default();
                        val.clear();
                    }
                    "v" | "t" => in_v = true,
                    _ => {}
                },
                Event::End(e) => match local(e.name().as_ref()) {
                    "v" | "t" => in_v = false,
                    "c" => {
                        let shown = match kind.as_str() {
                            "s" => val.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default(),
                            "b" => if val.trim() == "1" { "TRUE".into() } else { "FALSE".into() },
                            _ => std::mem::take(&mut val),
                        };
                        if col < 16_384 {
                            if row.len() <= col {
                                row.resize(col + 1, String::new());
                            }
                            row[col] = shown;
                        }
                    }
                    "row" => rows.push(std::mem::take(&mut row)),
                    _ => {}
                },
                Event::Text(t) if in_v => val.push_str(&t.xml10_content()),
                Event::GeneralRef(g) if in_v => {
                    if let Ok(Some(c)) = g.resolve_char_ref() {
                        val.push(c);
                    } else if let Some(s) = quick_xml::escape::resolve_predefined_entity(&g.xml10_content()) {
                        val.push_str(s);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        // 끝의 빈 줄은 뺀다
        while rows.last().is_some_and(|r| r.iter().all(|c| c.is_empty())) {
            rows.pop();
        }
        sheets.push((name, rows));
    }
    if sheets.is_empty() {
        return Err("시트가 하나도 없어요".into());
    }
    Ok(sheets)
}

/// 시트들을 인쇄용 표로 — 글은 모두 이스케이프하고 스크립트는 넣지 않는다
fn sheets_html(sheets: &[Sheet], title: &str) -> String {
    let esc = crate::text::esc;
    let mut body = String::new();
    for (i, (name, rows)) in sheets.iter().enumerate() {
        if sheets.len() > 1 {
            body.push_str(&format!("<h2{}>{}</h2>", if i > 0 { " class=\"next\"" } else { "" }, esc(name)));
        }
        body.push_str("<table>");
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        for r in rows {
            body.push_str("<tr>");
            for c in 0..cols {
                body.push_str(&format!("<td>{}</td>", esc(r.get(c).map(|s| s.as_str()).unwrap_or(""))));
            }
            body.push_str("</tr>");
        }
        body.push_str("</table>");
    }
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title><style>
html, body {{ margin: 0; background: #fff; color: #111; font: 9.5pt/1.45 "Malgun Gothic", "맑은 고딕", "Yu Gothic", "Microsoft YaHei", "Segoe UI", sans-serif; }}
h2 {{ font-size: 11pt; margin: 0 0 6px; }}
h2.next {{ break-before: page; }}
table {{ border-collapse: collapse; }}
td {{ border: 1px solid #bbb; padding: 2px 6px; white-space: pre-wrap; overflow-wrap: anywhere; vertical-align: top; }}
</style></head><body>{body}</body></html>"#,
        esc(title)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(dst: &Path, parts: &[(&str, &str)]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(dst).unwrap());
        for (n, s) in parts {
            z.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(s.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("mago-office-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Word 가 없을 때 — 문단 · 탭 · 줄바꿈 · &amp; 가 글로 남는다 (우리가 만드는 docx 로 확인)
    #[test]
    fn word_text_keeps_paragraphs_tabs_and_entities() {
        let d = tmp("docx");
        let src = d.join("a.txt");
        std::fs::write(&src, "첫 줄 & <태그>\n이름\t나이\n").unwrap();
        let made = crate::text::docx(&src, &d).unwrap();
        let t = docx_text(Path::new(&made.file)).unwrap();
        assert_eq!(t, "첫 줄 & <태그>\n이름\t나이\n");
    }

    #[test]
    fn hangul_text_reads_every_section_in_order() {
        let d = tmp("hwpx");
        let src = d.join("a.hwpx");
        let sec = |s: &str| format!(r#"<?xml version="1.0"?><hs:sec xmlns:hs="h" xmlns:hp="p"><hp:p><hp:run><hp:t>{s}</hp:t><hp:tab/><hp:t>끝</hp:t></hp:run></hp:p></hs:sec>"#);
        zip_with(&src, &[("Contents/section10.xml", &sec("열")), ("Contents/section2.xml", &sec("둘")), ("Contents/section0.xml", &sec("영"))]);
        assert_eq!(hwpx_text(&src).unwrap(), "영\t끝\n둘\t끝\n열\t끝\n");
        // 우리가 만드는 hwpx 도 읽힌다
        let txt = d.join("b.txt");
        std::fs::write(&txt, "가나\t다\n").unwrap();
        let made = crate::text::hwpx(&txt, &d).unwrap();
        assert!(hwpx_text(Path::new(&made.file)).unwrap().contains("가나"));
    }

    /// 엑셀이 없을 때 — 시트 순서 · 이름, 공유 글자 · 숫자 · 빈 칸 자리
    #[test]
    fn excel_sheets_come_out_in_book_order_with_cells_in_place() {
        let d = tmp("xlsx");
        let txt = d.join("표.txt");
        std::fs::write(&txt, "이름\t나이\t메모\n김철수\t31\n\t\tA&B\n").unwrap();
        let made = crate::text::xlsx(&txt, &d).unwrap();
        let s = xlsx_sheets(Path::new(&made.file)).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, vec![vec!["이름", "나이", "메모"], vec!["김철수", "31"], vec!["", "", "A&B"]]);
        let h = sheets_html(&s, "<표>");
        assert!(h.contains("A&amp;B") && h.contains("&lt;표&gt;") && !h.contains("<script"));
        assert_eq!(col_index("BC12"), Some(54));
        assert_eq!(col_index("A1"), Some(0));
    }

    #[test]
    fn only_office_and_hangul_files_count_as_documents() {
        for e in ["doc", "docx", "xls", "xlsx", "hwp", "hwpx"] {
            assert!(is_doc(e), "{e}");
        }
        for e in ["txt", "pdf", "pptx", "jpg"] {
            assert!(!is_doc(e), "{e}");
        }
    }

    /// 손으로 돌리는 확인 — 이 PC 의 Word · 엑셀 · 한글로 PDF 를 만든다 (프로그램이 깔린 PC 에서만)
    /// `MAGO_OFFICE_DIR` 에 시험 문서.docx · .xlsx · .hwpx · 한글 시험.hwp 를 둔다
    #[test]
    #[ignore]
    fn the_installed_programs_make_pdfs() {
        let d = std::path::PathBuf::from(std::env::var("MAGO_OFFICE_DIR").expect("MAGO_OFFICE_DIR"));
        for f in ["시험 문서.docx", "시험 문서.xlsx", "한글 시험.hwp", "시험 문서.hwpx"] {
            let src = d.join(f);
            let dst = d.join(format!("{}-러스트.pdf", f.replace('.', "-")));
            let t = std::time::Instant::now();
            let how = to_pdf(&src, &dst);
            println!("{f} → {how:?} {}ms {}B", t.elapsed().as_millis(), std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0));
        }
    }
}
