//! **여러 종류를 한꺼번에** (10-24 CEO "jpg, txt, pdf, word, 한글파일, xlsx 를 모두 잡고 시프트드래그하면
//! 모두 한장의 pdf, jpg 로 저장할건지, 쪽마다 저장할건지 … 순서는 선택한 파일순서대로").
//!
//! 고리 칸 (ring.rs MIX_OPTS):
//! - `mix-pdf`  : 순서대로 이은 PDF 한 파일 — 1번 파일 옆 `1번이름 외 N개.pdf`
//! - `mix-pdfs` : 파일마다 PDF — 각 원본 옆 `이름.pdf` (이미 PDF 인 것은 그대로 둔다)
//! - `mix-jpg`  : 순서대로 세로로 이은 JPG 한 장 — 1번 파일 옆 `1번이름 외 N개.jpg`
//! - `mix-jpgs` : 쪽마다 JPG — 1번 파일 옆 새 폴더 `1번이름 외 N개` 에 `01_이름.jpg` · `02_보고서_1쪽.jpg` …
//!
//! 순서는 탐색기가 넘겨 준 순서 그대로다 (고리 위 목록의 번호와 같다).
//! 문서(TXT · Word · 엑셀 · 한글)는 먼저 PDF 로 만들고(text.rs · office.rs), 그림은 그대로 쓴다.
//! 한 파일이 실패해도 나머지로 만든다 — 실패한 수와 첫 이유를 돌려준다. 원본은 읽기만 한다.

use crate::convert::convert::{self, unique_path, OutFormat};
use crate::convert::ext_convert::{ONE_MAX_HEIGHT, ONE_MAX_PIXELS};
use crate::convert::pdf_render::{self, Msg, DIP_PER_INCH};
use image::{DynamicImage, ImageDecoder, RgbaImage};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::path::{Path, PathBuf};

/// 고리 칸 (ring.rs 설정 · settings.js 와 같아야 한다)
pub const OPTS: &[&str] = &["mix-pdf", "mix-pdfs", "mix-jpg", "mix-jpgs"];

/// A4 (pt)
const A4: (f64, f64) = (595.28, 841.89);
/// 그림 쪽 둘레 여백 (pt, 약 1cm)
const MARGIN: f64 = 28.0;
/// PDF 에 넣는 그림의 긴 변 상한 (px) — A4 를 250dpi 안팎으로 채우는 크기. 폰 사진(4000px)을 그대로 넣으면 PDF 가 너무 커진다
const PDF_IMAGE_SIDE: u32 = 2400;
/// 한 장으로 이을 때 그림만 있으면 폭 상한 (px)
const ONE_IMAGE_WIDTH: u32 = 2000;
const WHITE: [u8; 4] = [255, 255, 255, 255];

#[derive(Default, Debug)]
pub struct Outcome {
    pub made: usize,
    pub failed: usize,
    pub first_error: String,
    /// 결과 한 줄 — "6개를 PDF 12쪽으로"
    pub note: String,
    /// 만든 파일 · 폴더 (로그)
    pub out: Vec<String>,
}

impl Outcome {
    /// 한 파일로 다 만든 결과를 적는다 — 실패면 모두 실패로
    pub fn settle(&mut self, r: Result<String, String>, text_only: usize, n: usize) {
        match r {
            Ok(note) => {
                self.made = 1;
                self.note = if text_only > 0 { format!("{note} · 문서 {text_only}개는 글만") } else { note };
            }
            Err(e) => {
                self.failed = n;
                self.first_error = e;
            }
        }
    }

    fn fail(&mut self, i: usize, e: String) {
        self.failed += 1;
        crate::log(&format!("[묶음] {}번 실패 — {e}", i + 1));
        if self.first_error.is_empty() {
            self.first_error = format!("{}번: {e}", i + 1);
        }
    }
}

/// 이 파일은 그대로 쓸 수 있는가 / 먼저 PDF 로 만들어야 하나
#[derive(Clone, Debug)]
enum Piece {
    Image(PathBuf),
    Pdf(PathBuf),
}

fn ext(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn stem(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("파일").to_string()
}

fn is_image(p: &Path) -> bool {
    let e = ext(p);
    convert::READABLE_EXT.contains(&e.as_str())
}

/// 묶음 이름 — "보고서 외 5개" (하나면 그 이름)
fn bundle_name(files: &[PathBuf]) -> String {
    let first = files.first().map(|f| stem(f)).unwrap_or_else(|| "묶음".into());
    if files.len() > 1 { format!("{first} 외 {}개", files.len() - 1) } else { first }
}

/// 사진의 회전 정보(EXIF)를 따라 세운 그림
fn load_image(p: &Path) -> Result<RgbaImage, String> {
    let mut r = image::ImageReader::open(p).map_err(|e| format!("그림을 열지 못했어요: {e}"))?.with_guessed_format().map_err(|e| format!("그림 형식을 모르겠어요: {e}"))?;
    let mut l = image::Limits::default();
    l.max_image_width = Some(convert::MAX_SIDE);
    l.max_image_height = Some(convert::MAX_SIDE);
    l.max_alloc = Some(convert::MAX_ALLOC);
    r.limits(l);
    let mut dec = r.into_decoder().map_err(|e| format!("그림을 읽지 못했어요: {e}"))?;
    let o = dec.orientation().unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut img = DynamicImage::from_decoder(dec).map_err(|e| format!("그림을 읽지 못했어요(깨졌거나 너무 커요): {e}"))?;
    img.apply_orientation(o);
    Ok(img.to_rgba8())
}

/// 세운 뒤의 크기 (그림을 다 읽지 않고)
fn image_size(p: &Path) -> Result<(u32, u32), String> {
    use image::metadata::Orientation::*;
    let r = image::ImageReader::open(p).map_err(|e| format!("그림을 열지 못했어요: {e}"))?.with_guessed_format().map_err(|e| format!("그림 형식을 모르겠어요: {e}"))?;
    let mut dec = r.into_decoder().map_err(|e| format!("그림을 읽지 못했어요: {e}"))?;
    let (w, h) = dec.dimensions();
    Ok(match dec.orientation().unwrap_or(NoTransforms) {
        Rotate90 | Rotate270 | Rotate90FlipH | Rotate270FlipH => (h, w),
        _ => (w, h),
    })
}

/// PDF 쪽 크기들 (DIP) — 그리지 않고 크기만 (pdf_render 가 크기를 알려 주는 첫 소식에서 멈춘다)
fn pdf_sizes(p: &Path) -> Result<Vec<(f64, f64)>, String> {
    const STOP: &str = "\u{1}크기만";
    let got = std::sync::Mutex::new(Vec::new());
    let r = pdf_render::render(p, &|d: &[(f64, f64)]| {
        *got.lock().unwrap() = d.to_vec();
        1.0
    }, &mut |_| Err(STOP.into()));
    match r {
        Err(e) if e != STOP => Err(e),
        _ => Ok(got.into_inner().unwrap()),
    }
}

/// 1단계 — 파일마다 그림이나 PDF 로. 문서는 임시 폴더에 PDF 를 만든다
fn prepare(files: &[PathBuf], tmp: &Path, out: &mut Outcome, text_only: &mut usize, progress: &mut dyn FnMut(usize, f64)) -> Vec<(usize, Piece)> {
    let mut pieces = Vec::new();
    for (i, f) in files.iter().enumerate() {
        progress(i, i as f64 / files.len() as f64);
        let e = ext(f);
        let r = if is_image(f) {
            Ok(Piece::Image(f.clone()))
        } else if e == "pdf" {
            Ok(Piece::Pdf(f.clone()))
        } else if crate::text::TEXT_EXT.contains(&e.as_str()) {
            crate::text::pdf(f, tmp).map(|o| Piece::Pdf(PathBuf::from(o.file)))
        } else if crate::office::is_doc(&e) {
            let dst = tmp.join(format!("{:03}.pdf", i + 1));
            crate::office::to_pdf(f, &dst).map(|how| {
                if how == crate::office::How::TextOnly {
                    *text_only += 1;
                }
                Piece::Pdf(dst)
            })
        } else {
            Err(format!(".{e} 는 넣을 수 없어요"))
        };
        match r {
            Ok(p) => pieces.push((i, p)),
            Err(e) => out.fail(i, e),
        }
    }
    pieces
}

/// 임시 폴더를 만들고 파일마다 그림이나 PDF 로 (진행의 앞 60%). 하나도 못 쓰면 None — 임시 폴더도 지운다
fn gather(files: &[PathBuf], out: &mut Outcome, progress: &mut dyn FnMut(usize, f64)) -> Option<(PathBuf, Vec<(usize, Piece)>, usize)> {
    let tmp = std::env::temp_dir().join(format!("mago-mix-{}-{}", std::process::id(), chrono::Local::now().timestamp_nanos_opt().unwrap_or_default()));
    if let Err(e) = std::fs::create_dir_all(&tmp) {
        out.first_error = format!("임시 폴더를 만들지 못했어요: {e}");
        return None;
    }
    let mut text_only = 0usize;
    // 문서를 PDF 로 만드는 데 시간이 가장 많이 든다 — 진행의 앞 60%
    let pieces = prepare(files, &tmp, out, &mut text_only, &mut |i, f| progress(i, f * 0.6));
    if pieces.is_empty() {
        let _ = std::fs::remove_dir_all(&tmp);
        return None;
    }
    Some((tmp, pieces, text_only))
}

/// 묶음 만들기. `dirs[i]` = i 번 파일 결과를 둘 폴더 (원본 폴더, 못 쓰면 다른 곳 — ring.rs 가 정한다).
/// `progress(지금 파일 번호, 전체 0~1)`. JPG 한 장은 여기서는 기본 크기로 바로 저장한다 (고리에서는 sheet.rs 미리 보기를 거친다)
pub fn run(target: &str, files: &[PathBuf], dirs: &[PathBuf], quality: u8, dpi: u32, progress: &mut dyn FnMut(usize, f64)) -> Outcome {
    let mut out = Outcome::default();
    if files.is_empty() || dirs.len() != files.len() {
        out.first_error = "파일이 없어요".into();
        return out;
    }
    crate::log(&format!("[묶음] {target} · {}개 · 순서 {:?}", files.len(), files.iter().map(|f| f.file_name().unwrap_or_default().to_string_lossy().to_string()).collect::<Vec<_>>()));
    if target == "mix-pdfs" {
        each_pdf(files, dirs, &mut out, progress);
        return out;
    }
    let n = files.len();
    if target == "mix-jpg" {
        let (sheet, mut out) = sheet(files, dirs, quality, dpi, false, progress);
        if let Some(s) = sheet {
            let dst = unique_path(&s.dir, &format!("{}.jpg", s.name));
            let r = s.draw(&s.default_widths(), &dst, &mut |f| progress(n, 0.6 + 0.4 * f), &mut out);
            out.settle(r, s.text_only, n);
        }
        return out;
    }
    let Some((tmp, pieces, text_only)) = gather(files, &mut out, progress) else { return out };
    // 잇는 동안은 모든 파일을 마친 것으로 (목록의 ✓ 가 되돌아가지 않게)
    let mut later = |_: usize, f: f64| progress(n, 0.6 + 0.4 * f);
    let name = bundle_name(files);
    let dir = &dirs[0];
    let r = match target {
        "mix-pdf" => merge_pdf(&pieces, &unique_path(dir, &format!("{name}.pdf")), dpi, quality, n, &|_| None, &mut later, &mut out),
        "mix-jpgs" => page_jpgs(&pieces, files, &unique_path(dir, &name), dpi, quality, n, &mut later, &mut out),
        _ => Err(format!("모르는 칸이에요: {target}")),
    };
    out.settle(r, text_only, n);
    let _ = std::fs::remove_dir_all(&tmp);
    out
}

// ── PDF 하나로 ────────────────────────────────────────────────────────────────

/// 그림이 들어갈 A4 쪽 (pt) — 가로로 긴 그림은 가로 쪽
fn a4_for(w: f64, h: f64) -> (f64, f64) {
    if w > h { (A4.1, A4.0) } else { A4 }
}

/// A4 쪽 여백 안에 들어가는 그림 폭의 상한 (pt) — 그림 (w, h) px
fn a4_max_width(w: f64, h: f64) -> f64 {
    let (pw, ph) = a4_for(w, h);
    (pw - 2.0 * MARGIN).min((ph - 2.0 * MARGIN) * w / h.max(1.0))
}

/// 그림 한 장을 쪽 하나로. `page` 가 없으면 A4(그림 모양 따라 세로 · 가로)에 여백을 두고 가운데, 있으면 그 크기(pt) 쪽에 꽉 차게.
/// `width` 는 A4 쪽에 그릴 그림 폭(pt) — 미리 보기 창에서 고른 것 (없으면 1px = 1pt, 여백 안에 들어가게)
fn add_image_page(doc: &mut Document, pages_id: ObjectId, img: &RgbaImage, page: Option<(f64, f64)>, width: Option<f64>, quality: u8) -> Result<ObjectId, String> {
    let (ow, oh) = (img.width() as f64, img.height() as f64);
    let mut img = std::borrow::Cow::Borrowed(img);
    let long = img.width().max(img.height());
    if page.is_none() && long > PDF_IMAGE_SIDE {
        let k = PDF_IMAGE_SIDE as f64 / long as f64;
        img = std::borrow::Cow::Owned(image::imageops::resize(img.as_ref(), ((img.width() as f64 * k).round() as u32).max(1), ((img.height() as f64 * k).round() as u32).max(1), image::imageops::FilterType::Lanczos3));
    }
    let rgb = convert::flatten_on(&img, WHITE);
    let mut jpg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg, quality.clamp(50, 100)).encode_image(&rgb).map_err(|e| format!("그림을 PDF 에 넣지 못했어요: {e}"))?;
    let (w, h) = (rgb.width() as f64, rgb.height() as f64);
    let (pw, ph, dw, dh) = match page {
        Some((pw, ph)) => (pw, ph, pw, ph),
        None => {
            let (pw, ph) = a4_for(w, h);
            // 작은 그림을 억지로 키우지 않는다 (1px = 1pt 까지). 고른 폭이 있으면 그대로 — 여백 안까지만
            let dw = width.unwrap_or(ow).clamp(1.0, a4_max_width(ow, oh));
            (pw, ph, dw, dw * h / w)
        }
    };
    let (x, y) = ((pw - dw) / 2.0, (ph - dh) / 2.0);
    let im = doc.add_object(Stream::new(
        dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => rgb.width() as i64, "Height" => rgb.height() as i64,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "Filter" => "DCTDecode" },
        jpg,
    ));
    let content = doc.add_object(Stream::new(dictionary! {}, format!("q {dw:.2} 0 0 {dh:.2} {x:.2} {y:.2} cm /Im0 Do Q").into_bytes()));
    Ok(doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), Object::Real(pw as f32), Object::Real(ph as f32)],
        "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => im } },
        "Contents" => content,
    }))
}

/// 쪽 나무에서 물려받는 값 — 쪽을 새 나무에 옮기면 끊기므로 쪽에 직접 적는다
const INHERITED: [&[u8]; 4] = [b"MediaBox", b"CropBox", b"Resources", b"Rotate"];

/// PDF 한 개의 쪽들을 `doc` 끝에 잇는다
fn append_pdf(doc: &mut Document, pages_id: ObjectId, kids: &mut Vec<Object>, mut src: Document) -> Result<usize, String> {
    src.renumber_objects_with(doc.max_id + 1);
    let pages: Vec<ObjectId> = src.get_pages().into_values().collect();
    if pages.is_empty() {
        return Err("쪽이 하나도 없는 PDF 예요".into());
    }
    for &pid in &pages {
        let mut add: Vec<(&[u8], Object)> = Vec::new();
        for key in INHERITED {
            if src.get_dictionary(pid).map(|d| d.has(key)).unwrap_or(true) {
                continue;
            }
            let mut cur = src.get_dictionary(pid).ok().and_then(|d| d.get(b"Parent").ok()).and_then(|p| p.as_reference().ok());
            for _ in 0..64 {
                let Some(id) = cur else { break };
                let Ok(d) = src.get_dictionary(id) else { break };
                if let Ok(v) = d.get(key) {
                    add.push((key, v.clone()));
                    break;
                }
                cur = d.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
            }
        }
        let d = src.get_dictionary_mut(pid).map_err(|e| format!("PDF 쪽을 읽지 못했어요: {e}"))?;
        for (k, v) in add {
            d.set(k.to_vec(), v);
        }
        d.set("Parent", pages_id);
        kids.push(Object::Reference(pid));
    }
    // 목차(Catalog)와 쪽 나무(Pages) 말고 다 옮긴다 — 안 쓰이게 된 것(책갈피 등)은 저장 전에 치운다
    let mut top = doc.max_id;
    for (id, obj) in std::mem::take(&mut src.objects) {
        let t = match &obj {
            Object::Dictionary(d) => d.get(b"Type").ok().and_then(|t| t.as_name().ok()).map(|n| n.to_vec()),
            _ => None,
        };
        if matches!(t.as_deref(), Some(b"Catalog") | Some(b"Pages")) {
            continue;
        }
        top = top.max(id.0);
        doc.objects.insert(id, obj);
    }
    doc.max_id = top;
    Ok(pages.len())
}

/// PDF 를 쪽 그림으로 그려 넣는다 — 잇기용으로 못 읽는 PDF (손상 · 특이한 암호)
fn append_pdf_as_images(doc: &mut Document, pages_id: ObjectId, kids: &mut Vec<Object>, p: &Path, dpi: u32, quality: u8) -> Result<usize, String> {
    let k = dpi as f64 / DIP_PER_INCH;
    let mut n = 0;
    let mut err = None;
    pdf_render::render(p, &move |_| k, &mut |m| {
        if let Msg::Page(_, img) = m {
            // 그린 크기(px) → 원래 쪽 크기(pt): px / k = DIP, DIP × 0.75 = pt
            let size = (img.width() as f64 / k * 0.75, img.height() as f64 / k * 0.75);
            match add_image_page(doc, pages_id, &img, Some(size), None, quality) {
                Ok(id) => {
                    kids.push(Object::Reference(id));
                    n += 1;
                }
                Err(e) => {
                    err = Some(e.clone());
                    return Err(e);
                }
            }
        }
        Ok(())
    })
    .map_err(|e| err.unwrap_or(e))?;
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
/// `width(j)` = j 번째 조각이 그림일 때 A4 쪽에 그릴 폭(pt) — 미리 보기 창에서 고른 것 (없으면 기본)
#[allow(clippy::too_many_arguments)]
fn merge_pdf(pieces: &[(usize, Piece)], dst: &Path, dpi: u32, quality: u8, n: usize, width: &dyn Fn(usize) -> Option<f64>, progress: &mut dyn FnMut(usize, f64), out: &mut Outcome) -> Result<String, String> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids: Vec<Object> = Vec::new();
    for (j, (i, p)) in pieces.iter().enumerate() {
        progress(*i, *i as f64 / n as f64);
        let r = match p {
            Piece::Image(f) => load_image(f).and_then(|img| add_image_page(&mut doc, pages_id, &img, None, width(j), quality)).map(|id| {
                kids.push(Object::Reference(id));
                1
            }),
            Piece::Pdf(f) => match Document::load(f) {
                Ok(src) if !(src.is_encrypted() && src.authenticate_password("").is_err()) => append_pdf(&mut doc, pages_id, &mut kids, src),
                other => {
                    crate::log(&format!("[묶음] PDF 를 그대로 잇지 못해 쪽 그림으로 넣음 — {} {:?}", f.display(), other.err()));
                    append_pdf_as_images(&mut doc, pages_id, &mut kids, f, dpi, quality)
                }
            },
        };
        if let Err(e) = r {
            out.fail(*i, e);
        }
    }
    if kids.is_empty() {
        return Err(if out.first_error.is_empty() { "넣을 쪽이 없어요".into() } else { out.first_error.clone() });
    }
    let count = kids.len();
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count as i64 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.prune_objects();
    doc.compress();
    doc.save(dst).map_err(|e| {
        let _ = std::fs::remove_file(dst);
        format!("PDF 를 저장하지 못했어요: {e}")
    })?;
    out.out.push(dst.to_string_lossy().into());
    progress(n.saturating_sub(1), 1.0);
    Ok(format!("PDF {count}쪽"))
}

// ── JPG ──────────────────────────────────────────────────────────────────────

/// 쪽 크기 재기 — 그림은 원래 크기, PDF 쪽은 dpi 로 그렸을 때 크기 (px)
fn measure(pieces: &[(usize, Piece)], dpi: u32, out: &mut Outcome) -> Vec<(usize, Vec<(f64, f64)>)> {
    let k = dpi as f64 / DIP_PER_INCH;
    let mut sizes = Vec::new();
    for (j, (i, p)) in pieces.iter().enumerate() {
        let r = match p {
            Piece::Image(f) => image_size(f).map(|(w, h)| vec![(w as f64, h as f64)]),
            Piece::Pdf(f) => pdf_sizes(f).map(|v| v.iter().map(|&(w, h)| (w * k, h * k)).collect()),
        };
        match r {
            Ok(v) if !v.is_empty() => sizes.push((j, v)),
            Ok(_) => out.fail(*i, "쪽이 하나도 없어요".into()),
            Err(e) => out.fail(*i, e),
        }
    }
    sizes
}

/// 한 장에 들어가는 칸 하나 — 그림 한 장 또는 PDF 쪽 하나 (미리 보기 창 sheet.rs 가 그대로 화면에 넘긴다)
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// 몇 번 파일인가 (0부터 — 고리 위 목록 번호보다 하나 작다)
    pub file: usize,
    pub name: String,
    pub image: bool,
    /// PDF 의 몇 쪽째 (1부터) / 모두 몇 쪽. 그림은 1 / 1
    pub page: usize,
    pub pages: usize,
    /// 원래 크기 (px) — 그림은 원본, PDF 쪽은 설정한 dpi 로 그린 크기
    pub ow: u32,
    pub oh: u32,
    /// 이 칸이 놓이는 바탕의 폭 · 높이 — JPG 한 장은 파일 폭(px), PDF 하나로는 그 쪽(pt, A4 · 문서 쪽 그대로)
    pub pw: f64,
    pub ph: f64,
    /// 그림 폭의 상한 — JPG 는 파일 폭, PDF 는 A4 여백 안
    pub max: u32,
    #[serde(skip)]
    piece: usize,
}

/// **JPG 한 장으로** 만들 준비가 된 것 — 크기를 재 두고, 고른 폭으로 그린다 (10-25 CEO "화질 떨어지면 안돼 …
/// 완성파일 미리 보여줘 … 원본보다 커지면 화질이 떨어질수 있다 알려주고 … 원본이 너무 커도 파일폭은 넘어가면 안돼").
/// - 파일 폭: PDF 쪽이 있으면 가장 넓은 PDF 쪽, 그림만 있으면 가장 넓은 그림(2000px 까지).
/// - 그림은 기본이 **원본 크기**다. 키우지 않는다. 파일 폭보다 넓은 그림만 폭에 맞춰 줄인다.
/// - 사람이 크기를 바꾸면(미리 보기 창) 파일 폭까지는 키울 수 있다 — 원본보다 크면 흐려질 수 있다고 알린다.
/// 문서를 바꾼 임시 PDF 는 이것이 사라질 때 지운다.
pub struct Sheet {
    pieces: Vec<(usize, Piece)>,
    pub parts: Vec<Part>,
    /// 파일 폭 (px)
    pub width: u32,
    dpi: u32,
    quality: u8,
    tmp: PathBuf,
    /// 저장할 폴더 · 이름(확장자 없이) — 1번 파일 옆 "1번이름 외 N개"
    pub dir: PathBuf,
    pub name: String,
    /// 프로그램이 없어 글만 옮긴 문서 수
    pub text_only: usize,
    /// **PDF 하나로** 미리 보기 (10-28) — 칸마다 쪽 하나, 그림은 A4 쪽에 고른 폭(pt)으로. 아니면 JPG 한 장
    pub pdf: bool,
}

impl Drop for Sheet {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

/// 그림을 (w, h) 로 — 같으면 그대로
fn fit(img: RgbaImage, w: u32, h: u32) -> RgbaImage {
    if (img.width(), img.height()) == (w, h) { img } else { image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3) }
}

impl Sheet {
    /// 처음 보여 줄 폭 — 그림은 원본 크기(파일 폭보다 넓으면 폭에 맞춰), PDF 쪽은 제 크기
    pub fn default_widths(&self) -> Vec<u32> {
        self.parts.iter().map(|p| if p.image { p.ow.min(p.max) } else if self.pdf { p.pw.round() as u32 } else { p.ow }).collect()
    }

    /// 고른 폭을 지킬 선 안으로 — 그림은 1px ~ 파일 폭, PDF 쪽은 바꾸지 않는다. 모자라는 값은 기본으로
    pub fn clamp(&self, widths: &[u32]) -> Vec<u32> {
        let d = self.default_widths();
        self.parts.iter().enumerate().map(|(k, p)| if p.image { widths.get(k).copied().unwrap_or(d[k]).clamp(1, p.max) } else { d[k] }).collect()
    }

    /// 칸마다 그릴 크기 · 파일 폭 · 전체를 줄인 비율. 너무 길거나 크면 전체를 같은 비율로 줄인다 (ext_convert 의 한 장과 같은 상한)
    pub fn layout(&self, widths: &[u32]) -> (Vec<(u32, u32)>, u32, f64) {
        let ws = self.clamp(widths);
        let raw: Vec<(f64, f64)> = self.parts.iter().zip(&ws).map(|(p, &w)| (w as f64, w as f64 * p.oh as f64 / p.ow.max(1) as f64)).collect();
        let total_h: f64 = raw.iter().map(|s| s.1).sum();
        let width = self.width as f64;
        let g = 1f64.min(ONE_MAX_HEIGHT as f64 / total_h.max(1.0)).min((ONE_MAX_PIXELS as f64 / (width * total_h).max(1.0)).sqrt());
        let sizes = raw.iter().map(|&(w, h)| (((w * g).round() as u32).max(1), ((h * g).round() as u32).max(1))).collect();
        (sizes, ((width * g).round() as u32).max(1), g)
    }

    /// 고른 폭으로 그려 `dst` 에 저장. 돌려주는 값은 결과 한 줄 — "7쪽을 JPG 한 장으로" · "PDF 7쪽"
    pub fn draw(&self, widths: &[u32], dst: &Path, progress: &mut dyn FnMut(f64), out: &mut Outcome) -> Result<String, String> {
        if self.pdf {
            let ws = self.clamp(widths);
            let by_piece = |j: usize| self.parts.iter().zip(&ws).find(|(p, _)| p.piece == j && p.image).map(|(_, &w)| w as f64);
            let n = self.pieces.len().max(1);
            return merge_pdf(&self.pieces, dst, self.dpi, self.quality, n, &by_piece, &mut |i, _| progress(i as f64 / n as f64), out);
        }
        let (sizes, cw, g) = self.layout(widths);
        let ch: u64 = sizes.iter().map(|s| s.1 as u64).sum();
        let mut canvas = RgbaImage::from_pixel(cw, ch.max(1) as u32, image::Rgba(WHITE));
        let k = self.dpi as f64 / DIP_PER_INCH * g;
        let n = self.pieces.len().max(1);
        let (mut y, mut at, mut drawn) = (0i64, 0usize, 0usize);
        for (j, (i, p)) in self.pieces.iter().enumerate() {
            progress(j as f64 / n as f64 * 0.95);
            let mine: Vec<(u32, u32)> = self.parts.iter().zip(&sizes).skip(at).take_while(|(pt, _)| pt.piece == j).map(|(_, s)| *s).collect();
            if mine.is_empty() {
                continue; // 크기를 못 재 빠진 파일
            }
            at += mine.len();
            let r: Result<(), String> = match p {
                Piece::Image(f) => load_image(f).map(|img| {
                    let (w, h) = mine[0];
                    image::imageops::overlay(&mut canvas, &fit(img, w, h), (cw as i64 - w as i64) / 2, y);
                }),
                Piece::Pdf(f) => {
                    let mut pages: Vec<(u32, u32)> = Vec::new();
                    let top = y;
                    pdf_render::render(f, &move |_| k, &mut |m| {
                        match m {
                            Msg::Plan(pl) => pages = pl,
                            Msg::Page(pi, img) => {
                                let py = top + pages.iter().take(pi).map(|s| s.1 as i64).sum::<i64>();
                                image::imageops::overlay(&mut canvas, &img, (cw as i64 - img.width() as i64) / 2, py);
                            }
                        }
                        Ok(())
                    })
                    .map(|_| ())
                }
            };
            match r {
                Ok(()) => drawn += 1,
                Err(e) => out.fail(*i, e),
            }
            y += mine.iter().map(|s| s.1 as i64).sum::<i64>();
        }
        if drawn == 0 {
            return Err(out.first_error.clone());
        }
        progress(0.95);
        convert::save(&canvas, dst, OutFormat::Jpg, self.quality, WHITE)?;
        out.out.push(dst.to_string_lossy().into());
        progress(1.0);
        let mut note = format!("{}쪽을 JPG 한 장으로", self.parts.len());
        if g < 0.99 {
            note.push_str(" · 길어서 줄임");
        }
        Ok(note)
    }

    /// 미리 보기 그림 — 칸마다 JPG(data: 주소). 파일 폭을 `tw` px 로 줄여 본 크기 (원본보다 크게는 그리지 않는다).
    /// 못 그린 칸은 빈 글 — 화면이 회색 칸으로 보인다
    pub fn thumbs(&self, tw: u32, progress: &mut dyn FnMut(f64)) -> Vec<String> {
        let s = (tw as f64 / self.width.max(1) as f64).min(1.0);
        let k = self.dpi as f64 / DIP_PER_INCH * s;
        let mut v = vec![String::new(); self.parts.len()];
        let n = self.pieces.len().max(1);
        let mut at = 0usize;
        for (j, (_, p)) in self.pieces.iter().enumerate() {
            progress(j as f64 / n as f64);
            let start = at;
            let count = self.parts.iter().skip(at).take_while(|pt| pt.piece == j).count();
            if count == 0 {
                continue;
            }
            at += count;
            match p {
                Piece::Image(f) => {
                    let pt = &self.parts[start];
                    // PDF 하나로는 쪽마다 따로 보이니 그림마다 tw 까지 (원본보다 크게는 안 그린다)
                    let s = if self.pdf { (tw as f64 / pt.ow.max(1) as f64).min(1.0) } else { s };
                    let (w, h) = (((pt.ow as f64 * s).round() as u32).max(1), ((pt.oh as f64 * s).round() as u32).max(1));
                    if let Ok(img) = load_image(f) {
                        v[start] = data_jpg(&fit(img, w, h));
                    }
                }
                Piece::Pdf(f) => {
                    let _ = pdf_render::render(f, &move |_| k, &mut |m| {
                        if let Msg::Page(pi, img) = m {
                            if pi < count {
                                v[start + pi] = data_jpg(&img);
                            }
                        }
                        Ok(())
                    });
                }
            }
        }
        progress(1.0);
        v
    }
}

/// 그림 → `data:image/jpeg;base64,…` (미리 보기 창에 바로 넣는다 — 창은 PC 의 파일을 읽지 못한다)
fn data_jpg(img: &RgbaImage) -> String {
    let rgb = convert::flatten_on(img, WHITE);
    let mut jpg = Vec::new();
    if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpg, 82).encode_image(&rgb).is_err() {
        return String::new();
    }
    format!("data:image/jpeg;base64,{}", base64(&jpg))
}

fn base64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for (k, sh) in [18, 12, 6, 0].into_iter().enumerate() {
            s.push(if k <= c.len() { A[(n >> sh & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// JPG 한 장 · PDF 하나로(`pdf`) 의 준비 — 문서를 PDF 로 바꾸고(진행의 앞 60%) 크기를 잰다. 다 실패하면 None
pub fn sheet(files: &[PathBuf], dirs: &[PathBuf], quality: u8, dpi: u32, pdf: bool, progress: &mut dyn FnMut(usize, f64)) -> (Option<Sheet>, Outcome) {
    let mut out = Outcome::default();
    if files.is_empty() || dirs.len() != files.len() {
        out.first_error = "파일이 없어요".into();
        return (None, out);
    }
    let Some((tmp, pieces, text_only)) = gather(files, &mut out, progress) else { return (None, out) };
    let sizes = measure(&pieces, dpi, &mut out);
    let mut parts = Vec::new();
    for (j, v) in &sizes {
        let (i, p) = &pieces[*j];
        for (pi, &(w, h)) in v.iter().enumerate() {
            parts.push(Part {
                file: *i,
                name: files[*i].file_name().unwrap_or_default().to_string_lossy().into(),
                image: matches!(p, Piece::Image(_)),
                page: pi + 1,
                pages: v.len(),
                ow: (w.round() as u32).max(1),
                oh: (h.round() as u32).max(1),
                pw: 0.0,
                ph: 0.0,
                max: 0,
                piece: *j,
            });
        }
    }
    let pdf_w = parts.iter().filter(|p| !p.image).map(|p| p.ow).max().unwrap_or(0);
    let width = if pdf_w > 0 { pdf_w } else { parts.iter().map(|p| p.ow).max().unwrap_or(1).min(ONE_IMAGE_WIDTH) };
    for p in &mut parts {
        if !pdf {
            (p.pw, p.max) = (width as f64, width);
        } else if p.image {
            (p.pw, p.ph) = a4_for(p.ow as f64, p.oh as f64);
            p.max = a4_max_width(p.ow as f64, p.oh as f64).floor() as u32;
        } else {
            // 문서 쪽은 그 크기 그대로 (그린 px → pt)
            let k = 72.0 / dpi as f64;
            (p.pw, p.ph, p.max) = (p.ow as f64 * k, p.oh as f64 * k, p.ow);
        }
    }
    let sheet = Sheet { pieces, parts, width, dpi, quality, tmp, dir: dirs[0].clone(), name: bundle_name(files), text_only, pdf };
    if sheet.parts.is_empty() {
        out.failed = files.len();
        return (None, out);
    }
    (Some(sheet), out)
}

#[allow(clippy::too_many_arguments)]
fn page_jpgs(pieces: &[(usize, Piece)], files: &[PathBuf], folder: &Path, dpi: u32, quality: u8, n: usize, progress: &mut dyn FnMut(usize, f64), out: &mut Outcome) -> Result<String, String> {
    let sizes = measure(pieces, dpi, out);
    if sizes.is_empty() {
        return Err(out.first_error.clone());
    }
    let total: usize = sizes.iter().map(|(_, v)| v.len()).sum();
    let digits = total.to_string().len().max(2);
    std::fs::create_dir_all(folder).map_err(|e| format!("폴더를 만들지 못했어요: {e}"))?;
    let k = dpi as f64 / DIP_PER_INCH;
    let mut seq = 0usize;
    let mut saved = 0usize;
    for (j, v) in &sizes {
        let (i, p) = &pieces[*j];
        progress(*i, *i as f64 / n as f64);
        let name = stem(&files[*i]);
        let r: Result<(), String> = match p {
            Piece::Image(f) => load_image(f).and_then(|img| {
                seq += 1;
                convert::save(&img, &unique_path(folder, &format!("{seq:0digits$}_{name}.jpg")), OutFormat::Jpg, quality, WHITE).map(|_| saved += 1)
            }),
            Piece::Pdf(f) => {
                let multi = v.len() > 1;
                let base = seq;
                seq += v.len();
                pdf_render::render(f, &move |_| k, &mut |m| {
                    if let Msg::Page(pi, img) = m {
                        let file = if multi { format!("{:0digits$}_{name}_{}쪽.jpg", base + pi + 1, pi + 1) } else { format!("{:0digits$}_{name}.jpg", base + 1) };
                        convert::save(&img, &unique_path(folder, &file), OutFormat::Jpg, quality, WHITE)?;
                        saved += 1;
                    }
                    Ok(())
                })
                .map(|_| ())
            }
        };
        if let Err(e) = r {
            out.fail(*i, e);
        }
    }
    if saved == 0 {
        let _ = std::fs::remove_dir(folder);
        return Err(out.first_error.clone());
    }
    out.out.push(folder.to_string_lossy().into());
    progress(n.saturating_sub(1), 1.0);
    Ok(format!("JPG {saved}장 · 새 폴더에"))
}

// ── 파일마다 PDF ──────────────────────────────────────────────────────────────

fn each_pdf(files: &[PathBuf], dirs: &[PathBuf], out: &mut Outcome, progress: &mut dyn FnMut(usize, f64)) {
    let n = files.len();
    let (mut kept, mut text_only) = (0usize, 0usize);
    for (i, f) in files.iter().enumerate() {
        progress(i, i as f64 / n as f64);
        let e = ext(f);
        let dst = unique_path(&dirs[i], &format!("{}.pdf", stem(f)));
        let r: Result<PathBuf, String> = if e == "pdf" {
            kept += 1;
            continue;
        } else if is_image(f) {
            load_image(f).and_then(|img| {
                let mut doc = Document::with_version("1.7");
                let pages_id = doc.new_object_id();
                let page = add_image_page(&mut doc, pages_id, &img, None, None, 90)?;
                doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![Object::Reference(page)], "Count" => 1 }));
                let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
                doc.trailer.set("Root", catalog);
                doc.compress();
                doc.save(&dst).map(|_| dst.clone()).map_err(|e| format!("PDF 를 저장하지 못했어요: {e}"))
            })
        } else if crate::text::TEXT_EXT.contains(&e.as_str()) {
            crate::text::pdf(f, &dirs[i]).map(|o| PathBuf::from(o.file))
        } else if crate::office::is_doc(&e) {
            crate::office::to_pdf(f, &dst).map(|how| {
                if how == crate::office::How::TextOnly {
                    text_only += 1;
                }
                dst.clone()
            })
        } else {
            Err(format!(".{e} 는 PDF 로 바꿀 수 없어요"))
        };
        match r {
            Ok(made) => {
                out.made += 1;
                out.out.push(made.to_string_lossy().into());
            }
            Err(e) => out.fail(i, e),
        }
    }
    progress(n.saturating_sub(1), 1.0);
    let mut note = Vec::new();
    if kept > 0 {
        note.push(format!("PDF {kept}개는 그대로"));
    }
    if text_only > 0 {
        note.push(format!("문서 {text_only}개는 글만"));
    }
    out.note = note.join(" · ");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mago-mix-t-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn png(p: &Path, w: u32, h: u32, c: [u8; 4]) {
        convert::save(&RgbaImage::from_pixel(w, h, image::Rgba(c)), p, OutFormat::Png, 90, WHITE).unwrap();
    }

    /// 그림 쪽 두 장짜리 PDF 를 만든다 (잇기 시험용 — 쪽 나무에서 MediaBox 를 물려받게)
    fn two_page_pdf(p: &Path) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        for c in [[200, 0, 0, 255], [0, 0, 200, 255]] {
            let id = add_image_page(&mut doc, pages_id, &RgbaImage::from_pixel(40, 60, image::Rgba(c)), Some((300.0, 400.0)), None, 90).unwrap();
            // MediaBox 를 쪽에서 빼서 쪽 나무로 올린다
            doc.get_dictionary_mut(id).unwrap().remove(b"MediaBox");
            kids.push(Object::Reference(id));
        }
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => 2,
            "MediaBox" => vec![0.into(), 0.into(), 300.into(), 400.into()] }));
        let cat = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", cat);
        doc.save(p).unwrap();
    }

    /// 순서대로 — 그림 · PDF(2쪽) · 그림 → 4쪽, 물려받던 MediaBox 가 쪽에 남는다
    #[test]
    fn one_pdf_keeps_the_order_and_inherited_page_sizes() {
        let d = tmp("pdf");
        let a = d.join("가 사진.png");
        png(&a, 300, 200, [255, 0, 0, 255]);
        let b = d.join("나 문서.pdf");
        two_page_pdf(&b);
        let c = d.join("다 사진.png");
        png(&c, 100, 400, [0, 255, 0, 255]);
        let files = vec![a.clone(), b.clone(), c.clone()];
        let dirs = vec![d.clone(); 3];
        let mut seen = Vec::new();
        let o = run("mix-pdf", &files, &dirs, 90, 150, &mut |i, _| seen.push(i));
        assert_eq!((o.made, o.failed), (1, 0), "{o:?}");
        assert_eq!(o.note, "PDF 4쪽");
        let out = d.join("가 사진 외 2개.pdf");
        assert_eq!(o.out, vec![out.to_string_lossy().to_string()]);
        let m = Document::load(&out).unwrap();
        let pages: Vec<ObjectId> = m.get_pages().into_values().collect();
        assert_eq!(pages.len(), 4);
        let boxes: Vec<Vec<f32>> = pages
            .iter()
            .map(|p| m.get_dictionary(*p).unwrap().get(b"MediaBox").unwrap().as_array().unwrap().iter().map(|v| v.as_float().unwrap()).collect())
            .collect();
        assert_eq!(boxes[0], vec![0.0, 0.0, A4.1 as f32, A4.0 as f32], "가로 그림은 가로 A4");
        assert_eq!(boxes[1], vec![0.0, 0.0, 300.0, 400.0], "PDF 쪽은 제 크기 그대로");
        assert_eq!(boxes[2], vec![0.0, 0.0, 300.0, 400.0]);
        assert_eq!(boxes[3], vec![0.0, 0.0, A4.0 as f32, A4.1 as f32], "세로 그림은 세로 A4");
        assert!(seen.windows(2).all(|w| w[0] <= w[1]), "진행은 1번부터 차례로: {seen:?}");
        assert_eq!((seen[0], *seen.last().unwrap()), (0, 3), "잇는 동안은 다 마친 것으로");
        for f in &files {
            assert!(f.exists(), "원본은 그대로");
        }
    }

    /// 한 장으로 — 순서대로 위에서 아래로, 그림은 폭에 맞춰 가운데
    #[test]
    fn one_jpg_stacks_in_order() {
        let d = tmp("jpg");
        let a = d.join("1.png");
        png(&a, 200, 100, [255, 0, 0, 255]);
        let b = d.join("2.pdf");
        two_page_pdf(&b);
        let c = d.join("3.png");
        png(&c, 50, 50, [0, 200, 0, 255]);
        let files = vec![a, b, c];
        let o = run("mix-jpg", &files, &vec![d.clone(); 3], 95, 96, &mut |_, _| {});
        assert_eq!((o.made, o.failed), (1, 0), "{o:?}");
        let img = image::open(d.join("1 외 2개.jpg")).unwrap().to_rgb8();
        // 96dpi 면 PDF 쪽(300×400pt = 400×533 DIP)은 폭 400 — 그림은 키우지 않는다
        assert_eq!(img.width(), 400);
        assert_eq!(img.height(), 100 + 533 * 2 + 50);
        let px = |x: u32, y: u32| img.get_pixel(x, y).0;
        assert!(px(200, 50)[0] > 200 && px(200, 50)[1] < 60, "맨 위는 1번(빨강)");
        assert!((180..230).contains(&px(200, 100 + 260)[0]) && px(200, 100 + 260)[2] < 60, "그다음 PDF 1쪽(짙은 빨강)");
        assert!(px(200, 100 + 533 + 260)[2] > 150, "PDF 2쪽(파랑)");
        assert!(px(200, 100 + 533 * 2 + 25)[1] > 150, "맨 아래는 3번(초록), 가운데");
        assert!(px(20, 100 + 533 * 2 + 25)[0] > 240, "좁은 그림 옆은 흰 바탕");
    }

    /// 쪽마다 — 새 폴더에 번호 순서로
    #[test]
    fn page_jpgs_go_into_a_new_folder_numbered_in_order() {
        let d = tmp("jpgs");
        let a = d.join("표지.png");
        png(&a, 30, 30, [9, 9, 9, 255]);
        let b = d.join("본문.pdf");
        two_page_pdf(&b);
        let o = run("mix-jpgs", &[a, b], &vec![d.clone(); 2], 90, 96, &mut |_, _| {});
        assert_eq!((o.made, o.failed), (1, 0), "{o:?}");
        let folder = d.join("표지 외 1개");
        let mut names: Vec<String> = std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names, ["01_표지.jpg", "02_본문_1쪽.jpg", "03_본문_2쪽.jpg"]);
    }

    /// 파일마다 PDF — 그림은 PDF 로, PDF 는 그대로, 못 읽는 파일은 실패로 세고 나머지는 만든다
    #[test]
    fn each_pdf_skips_pdfs_and_counts_failures() {
        let d = tmp("each");
        let a = d.join("사진.png");
        png(&a, 20, 20, [1, 2, 3, 255]);
        let b = d.join("이미.pdf");
        two_page_pdf(&b);
        let c = d.join("깨진.png");
        std::fs::write(&c, b"not a png").unwrap();
        let o = run("mix-pdfs", &[a.clone(), b.clone(), c], &vec![d.clone(); 3], 90, 150, &mut |_, _| {});
        assert_eq!((o.made, o.failed), (1, 1), "{o:?}");
        assert!(o.first_error.starts_with("3번:"), "{}", o.first_error);
        assert_eq!(o.note, "PDF 1개는 그대로");
        assert_eq!(Document::load(d.join("사진.pdf")).unwrap().get_pages().len(), 1);
        assert!(!d.join("이미 (2).pdf").exists(), "이미 PDF 인 것은 다시 만들지 않는다");
    }

    /// 묶음 하나가 깨져도 나머지로 만든다
    #[test]
    fn a_broken_file_does_not_stop_the_bundle() {
        let d = tmp("broken");
        let a = d.join("좋은.png");
        png(&a, 20, 20, [1, 2, 3, 255]);
        let b = d.join("깨진.pdf");
        std::fs::write(&b, b"%PDF-1.4 garbage").unwrap();
        let o = run("mix-pdf", &[a, b], &vec![d.clone(); 2], 90, 150, &mut |_, _| {});
        assert_eq!((o.made, o.failed), (1, 1), "{o:?}");
        assert_eq!(Document::load(d.join("좋은 외 1개.pdf")).unwrap().get_pages().len(), 1);
    }

    /// 10-25 CEO "화질 떨어지면 안돼 … 원본이 너무 커도 파일폭은 넘어가면 안돼" — 그림은 원본 크기가 기본, 넓은 것만 폭에 맞춤,
    /// 사람이 고른 폭은 파일 폭까지만, 고른 폭대로 그린다
    #[test]
    fn sheet_keeps_originals_and_caps_at_the_file_width() {
        let d = tmp("sheet");
        let a = d.join("1.png");
        png(&a, 50, 40, [255, 0, 0, 255]);
        let b = d.join("2.pdf");
        two_page_pdf(&b);
        let c = d.join("3.png");
        png(&c, 900, 300, [0, 0, 255, 255]);
        let files = vec![a, b, c];
        let (s, o) = sheet(&files, &vec![d.clone(); 3], 95, 96, false, &mut |_, _| {});
        let s = s.expect("준비");
        assert_eq!(o.failed, 0);
        // 96dpi 면 PDF 쪽 폭 400 = 파일 폭
        assert_eq!(s.width, 400);
        assert_eq!(s.parts.len(), 4);
        assert_eq!(s.default_widths(), vec![50, 400, 400, 400], "작은 그림은 원본 그대로, 넓은 그림(900)은 폭에 맞춰");
        assert_eq!(s.clamp(&[5000, 10, 10, 0]), vec![400, 400, 400, 1], "그림은 파일 폭까지만, PDF 쪽은 바꾸지 않는다");
        // 1번을 원본의 4배(200), 3번을 원본보다 작게(300)
        let dst = d.join("out.jpg");
        let mut out = Outcome::default();
        s.draw(&[200, 0, 0, 300], &dst, &mut |_| {}, &mut out).unwrap();
        let img = image::open(&dst).unwrap().to_rgb8();
        assert_eq!(img.width(), 400, "파일 폭은 그대로");
        assert_eq!(img.height(), 160 + 533 * 2 + 100);
        assert!(img.get_pixel(200, 80)[0] > 200 && img.get_pixel(60, 80)[0] > 240 && img.get_pixel(60, 80)[1] > 240, "1번은 가운데 200 폭");
        assert!(img.get_pixel(200, 160 + 1066 + 50)[2] > 200 && img.get_pixel(20, 160 + 1066 + 50)[1] > 240, "3번은 300 폭 가운데");
        let thumbs = s.thumbs(200, &mut |_| {});
        assert!(thumbs.iter().all(|t| t.starts_with("data:image/jpeg;base64,")), "미리 보기 그림 4칸");
    }

    /// 10-28 CEO "모두 진행해" — PDF 하나로도 미리 보기 · 그림 크기 조절. 그림은 A4 쪽에 원본 크기(1px = 1pt)가 기본, 여백 안까지만 키운다
    #[test]
    fn pdf_sheet_sizes_pictures_on_a4_pages() {
        let d = tmp("pdfsheet");
        let a = d.join("1.png");
        png(&a, 80, 100, [255, 0, 0, 255]);
        let b = d.join("2.pdf");
        two_page_pdf(&b);
        let c = d.join("3.png");
        png(&c, 3000, 1000, [0, 0, 255, 255]);
        let files = vec![a, b, c];
        let (s, o) = sheet(&files, &vec![d.clone(); 3], 95, 96, true, &mut |_, _| {});
        let s = s.expect("준비");
        assert_eq!(o.failed, 0);
        assert!(s.pdf);
        let p = &s.parts;
        assert_eq!((p[0].pw.round(), p[0].ph.round(), p[0].max), (595.0, 842.0, 539), "작은 그림은 세로 A4");
        assert_eq!((p[3].pw.round(), p[3].max), (842.0, 785), "가로로 긴 그림은 가로 A4 · 여백 안 폭");
        assert_eq!(s.default_widths(), vec![80, 300, 300, 785], "작은 그림은 원본 크기, 넓은 그림은 쪽에 맞춰, 문서 쪽은 제 크기(pt)");
        assert_eq!(s.clamp(&[9999, 1, 1, 9999]), vec![539, 300, 300, 785], "여백 밖으로는 안 키운다");
        let dst = d.join("out.pdf");
        let mut out = Outcome::default();
        assert_eq!(s.draw(&[400, 0, 0, 500], &dst, &mut |_| {}, &mut out).unwrap(), "PDF 4쪽");
        let doc = Document::load(&dst).unwrap();
        let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
        assert_eq!(pages.len(), 4);
        let content = String::from_utf8(doc.get_page_content(pages[0]).unwrap()).unwrap();
        assert!(content.starts_with("q 400.00 0 0 500.00 "), "1번 그림은 고른 폭 400pt · 비율 그대로: {content}");
        let last = String::from_utf8(doc.get_page_content(pages[3]).unwrap()).unwrap();
        assert!(last.starts_with("q 500.00 0 0 166.67 "), "{last}");
        assert!(s.thumbs(200, &mut |_| {}).iter().all(|t| t.starts_with("data:image/jpeg")));
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn bundle_names() {
        assert_eq!(bundle_name(&[PathBuf::from("C:\\a\\보고서.docx")]), "보고서");
        assert_eq!(bundle_name(&[PathBuf::from("C:\\a\\보고서.docx"), PathBuf::from("b.jpg"), PathBuf::from("c.txt")]), "보고서 외 2개");
    }
}
