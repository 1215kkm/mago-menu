//! 글을 PDF 로 — 숨은 화면 창(WebView2)에 글을 띄우고 'PDF 로 인쇄' (ICoreWebView2_7::PrintToPdf).
//! 한글 · 한자 · 일본어 글꼴을 윈도우 글꼴에서 그대로 쓰고, 쓴 글자만 PDF 에 넣는다. 설치 파일에 더하는 것 없음.
//! 글은 임시 HTML 파일(text.rs pdf_html — 이스케이프, 스크립트 없음)로 넘긴다. 한 번에 하나씩.

use std::path::Path;
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Environment6, ICoreWebView2_7};
use webview2_com::PrintToPdfCompletedHandler;
use windows_core_62::{Interface, HSTRING};

const LABEL: &str = "pdfprint";
static ONE: Mutex<()> = Mutex::new(());

pub fn print(html: &Path, dst: &Path) -> Result<(), String> {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let app = crate::ring::app().ok_or("프로그램이 아직 준비되지 않았어요")?;
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
    let url = tauri::Url::from_file_path(html).map_err(|_| "임시 파일 주소를 만들지 못했어요".to_string())?;
    let (ltx, lrx) = mpsc::channel::<()>();
    let ltx = Mutex::new(ltx);
    let w = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title("MAGO PDF")
        .visible(false)
        .skip_taskbar(true)
        .focused(false)
        .on_page_load(move |_, p| {
            if p.event() == PageLoadEvent::Finished {
                let _ = ltx.lock().map(|t| t.send(()));
            }
        })
        .build()
        .map_err(|e| format!("PDF 창을 열지 못했어요: {e}"))?;
    let r = (|| {
        lrx.recv_timeout(Duration::from_secs(30)).map_err(|_| "글을 펼치는 데 너무 오래 걸려요".to_string())?;
        let (ptx, prx) = mpsc::channel::<Result<(), String>>();
        let dst_s = dst.to_string_lossy().to_string();
        w.with_webview(move |pw| {
            let fail = ptx.clone();
            let r = (|| -> windows_core_62::Result<()> {
                unsafe {
                    let core: ICoreWebView2_7 = pw.controller().CoreWebView2()?.cast()?;
                    let env: ICoreWebView2Environment6 = pw.environment().cast()?;
                    let st = env.CreatePrintSettings()?;
                    // A4 (인치). 여백은 위아래 18mm · 양옆 16mm
                    st.SetPageWidth(8.27)?;
                    st.SetPageHeight(11.69)?;
                    st.SetMarginTop(0.71)?;
                    st.SetMarginBottom(0.71)?;
                    st.SetMarginLeft(0.63)?;
                    st.SetMarginRight(0.63)?;
                    st.SetShouldPrintHeaderAndFooter(false.into())?;
                    st.SetShouldPrintBackgrounds(true.into())?;
                    let h = PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
                        let _ = ptx.send(if hr.is_ok() && ok { Ok(()) } else { Err(format!("PDF 로 인쇄하지 못했어요 ({hr:?})")) });
                        Ok(())
                    }));
                    core.PrintToPdf(&HSTRING::from(dst_s), &st, &h)?;
                }
                Ok(())
            })();
            if let Err(e) = r {
                let _ = fail.send(Err(format!("PDF 로 인쇄하지 못했어요: {e}")));
            }
        })
        .map_err(|e| format!("PDF 창에 닿지 못했어요: {e}"))?;
        prx.recv_timeout(Duration::from_secs(120)).map_err(|_| "PDF 를 만드는 데 너무 오래 걸려요".to_string())?
    })();
    let _ = w.destroy();
    r
}
