//! **PDF → 그림** (10-16 CEO "파일형식 변환 기능 중에 pdf 를 jpg 로 — 각 장으로 저장, pdf 전체를 한 장으로 저장").
//!
//! Windows 10·11 에 들어 있는 PDF 그리기(`Windows.Data.Pdf`)를 쓴다. 엣지가 PDF 를 보여 줄 때 쓰는 것과 같은 줄기라
//! 글꼴·벡터가 깨끗하게 나오고, **설치 파일에 아무것도 더하지 않는다**.
//!
//! WinRT 는 MTA 스레드가 필요하다. 포토샵 COM 이 쓰는 스레드(STA)와 섞이지 않게 **늘 새 스레드**에서 그리고,
//! 그린 쪽은 한 장씩 통로(channel)로 넘긴다 — 100쪽짜리를 한꺼번에 메모리에 올리지 않는다.

use image::RgbaImage;
use std::path::Path;
use std::sync::mpsc::{sync_channel, SyncSender};

/// 쪽 크기 단위는 DIP(1/96 인치). 150dpi 로 그리려면 150/96 배.
pub const DIP_PER_INCH: f64 = 96.0;

pub enum Msg {
    /// 그리기 전에 한 번 — 쪽마다 그릴 크기(px)
    Plan(Vec<(u32, u32)>),
    Page(usize, RgbaImage),
}

/// `scale_for(쪽 크기들 DIP)` 가 DIP 당 픽셀 배율을 정하고, 그 배율로 쪽마다 그려 `on_msg` 에 넘긴다.
/// `on_msg` 가 Err 를 돌려주면 그 자리에서 멈춘다. 돌려주는 값: 쪽 수.
pub fn render(
    src: &Path,
    scale_for: &(dyn Fn(&[(f64, f64)]) -> f64 + Sync),
    on_msg: &mut dyn FnMut(Msg) -> Result<(), String>,
) -> Result<usize, String> {
    let (tx, rx) = sync_channel::<Result<Msg, String>>(1);
    std::thread::scope(|s| {
        s.spawn(move || {
            if let Err(e) = worker(src, scale_for, &tx) {
                let _ = tx.send(Err(e));
            }
        });
        let mut pages = 0;
        for m in rx {
            let m = m?;
            if let Msg::Plan(p) = &m {
                pages = p.len();
            }
            on_msg(m)?;
        }
        Ok(pages)
    })
}

#[cfg(windows)]
fn worker(src: &Path, scale_for: &(dyn Fn(&[(f64, f64)]) -> f64 + Sync), tx: &SyncSender<Result<Msg, String>>) -> Result<(), String> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
    let inited = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    let r = draw(src, scale_for, tx);
    if inited {
        unsafe { CoUninitialize() };
    }
    r
}

#[cfg(not(windows))]
fn worker(_: &Path, _: &(dyn Fn(&[(f64, f64)]) -> f64 + Sync), _: &SyncSender<Result<Msg, String>>) -> Result<(), String> {
    Err("PDF 바꾸기는 Windows 에서만 돼요.".into())
}

#[cfg(windows)]
fn why(e: windows::core::Error) -> String {
    match e.code().0 as u32 {
        0x8007_052B => "암호가 걸린 PDF 예요. 암호를 푼 뒤 다시 넣어 주세요.".into(),
        0x8007_0002 | 0x8007_0003 => "파일을 찾지 못했어요. 옮겨졌거나 지워졌을 수 있어요.".into(),
        _ => format!("PDF 를 열지 못했어요. 파일이 깨졌을 수 있어요. ({})", e.message()),
    }
}

#[cfg(windows)]
fn draw(src: &Path, scale_for: &(dyn Fn(&[(f64, f64)]) -> f64 + Sync), tx: &SyncSender<Result<Msg, String>>) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};

    let abs = std::path::absolute(src).map_err(|e| format!("파일 위치를 알 수 없어요: {e}"))?;
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(abs.as_os_str())).and_then(|op| op.get()).map_err(why)?;
    let doc = PdfDocument::LoadFromFileAsync(&file).and_then(|op| op.get()).map_err(why)?;
    let n = doc.PageCount().map_err(why)?;
    if n == 0 {
        return Err("쪽이 하나도 없는 PDF 예요.".into());
    }
    let mut dips = Vec::with_capacity(n as usize);
    for i in 0..n {
        let s = doc.GetPage(i).and_then(|p| p.Size()).map_err(why)?;
        dips.push((s.Width.max(1.0) as f64, s.Height.max(1.0) as f64));
    }
    let k = scale_for(&dips);
    let plan: Vec<(u32, u32)> = dips.iter().map(|&(w, h)| (((w * k).round() as u32).max(1), ((h * k).round() as u32).max(1))).collect();
    if tx.send(Ok(Msg::Plan(plan.clone()))).is_err() {
        return Ok(());
    }
    let fail = |i: u32, e: windows::core::Error| format!("{}쪽을 그리지 못했어요: {}", i + 1, e.message());
    for i in 0..n {
        let page = doc.GetPage(i).map_err(|e| fail(i, e))?;
        let opts = PdfPageRenderOptions::new().map_err(|e| fail(i, e))?;
        opts.SetDestinationWidth(plan[i as usize].0).map_err(|e| fail(i, e))?;
        opts.SetDestinationHeight(plan[i as usize].1).map_err(|e| fail(i, e))?;
        let stream = InMemoryRandomAccessStream::new().map_err(|e| fail(i, e))?;
        page.RenderWithOptionsToStreamAsync(&stream, &opts).and_then(|op| op.get()).map_err(|e| fail(i, e))?;
        let size = stream.Size().map_err(|e| fail(i, e))? as u32;
        let reader = stream.GetInputStreamAt(0).and_then(|s| DataReader::CreateDataReader(&s)).map_err(|e| fail(i, e))?;
        reader.LoadAsync(size).and_then(|op| op.get()).map_err(|e| fail(i, e))?;
        let mut png = vec![0u8; size as usize];
        reader.ReadBytes(&mut png).map_err(|e| fail(i, e))?;
        drop(page);
        // 그려 준 것은 PNG 다 (기본 인코더). 바탕은 흰색으로 칠해져 나온다
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|e| format!("{}쪽 그림을 읽지 못했어요: {e}", i + 1))?
            .to_rgba8();
        if tx.send(Ok(Msg::Page(i as usize, img))).is_err() {
            return Ok(()); // 받는 쪽이 멈췄다
        }
    }
    Ok(())
}
