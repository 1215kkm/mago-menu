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

/// 전용 스레드에서 프로그램을 부르고, 3분 안에 끝나지 않으면 포기한다 (그 스레드는 버린다)
fn with_timeout(src: &Path, dst: &Path, ext: &str) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let (src, dst, ext) = (src.to_path_buf(), dst.to_path_buf(), ext.to_string());
    std::thread::Builder::new()
        .name("mago-office".into())
        .spawn(move || {
            let _ = tx.send(com::export(&src, &dst, &ext));
        })
        .map_err(|e| format!("일할 스레드를 만들지 못했어요: {e}"))?;
    rx.recv_timeout(Duration::from_secs(180)).map_err(|_| "프로그램이 3분 넘게 대답하지 않아요. 그 프로그램에 뜬 창이 있는지 봐 주세요.".to_string())?
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
