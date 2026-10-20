//! **움직이는 GIF** (10-20 CEO "동작이 이어지는 한 장의 이미지나 각 동작의 여러 장의 이미지를 넣었을 때 GIF 로 움직일 수 있게").
//! - 여러 장을 같이 끌어 GIF 칸에 놓으면 → 이름 순서(1, 2, … 10 — 숫자는 숫자로 센다)로 이어 한 GIF
//! - 한 장이면 → 동작 칸이 나뉜 그림(스프라이트 시트)인지 본다. 칸 사이가 비어 있거나(투명·바탕색 줄),
//!   가로가 세로의 정확히 몇 배면 그만큼 나눈다. 나눌 수 없는 그림이면 `None` — 보통 GIF 로 바꾼다(ring.rs).
//! 결과는 원본 옆 `이름_움직임.gif`. 원본은 고치지 않는다.

use crate::convert::convert;
use crate::convert::OutputFile;
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, Rgba, RgbaImage};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// 한 장(프레임)의 긴 변 상한 — GIF 는 크면 용량이 금방 수십 MB 가 된다
pub const MAX_SIDE: u32 = 800;
/// 이어 붙일 수 있는 장 수
pub const MAX_FRAMES: usize = 300;
/// 한 장을 보여 주는 시간 (ms) — 설정에서 고른다
pub const SPEEDS: &[u32] = &[60, 100, 200, 500];
pub const DEFAULT_MS: u32 = 100;

pub struct Made {
    pub out: OutputFile,
    pub frames: usize,
}

// ── 이름 순서 ─────────────────────────────────────────────────────────────────

/// 사람이 세는 순서 — "동작2" 가 "동작10" 보다 앞
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (si, sj) = (i, j);
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = a[si..i].iter().collect::<String>().trim_start_matches('0').into();
            let nb: String = b[sj..j].iter().collect::<String>().trim_start_matches('0').into();
            let o = na.len().cmp(&nb.len()).then_with(|| na.cmp(&nb));
            if o != Ordering::Equal {
                return o;
            }
        } else {
            if a[i] != b[j] {
                return a[i].cmp(&b[j]);
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

fn name_of(p: &Path) -> String {
    p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string()
}

// ── 한 장을 칸으로 나누기 ─────────────────────────────────────────────────────

/// 비어 있는 점 — 투명이거나 바탕색(왼쪽 위 점)과 거의 같다
fn blank(p: &Rgba<u8>, bg: &Rgba<u8>) -> bool {
    if p[3] < 16 {
        return true;
    }
    bg[3] >= 16 && (0..4).all(|i| (p[i] as i32 - bg[i] as i32).abs() <= 24)
}

/// 줄마다 통째로 비었나 (cols = 세로줄, 아니면 가로줄)
fn empty_lines(img: &RgbaImage, cols: bool) -> Vec<bool> {
    let bg = *img.get_pixel(0, 0);
    let (n, m) = if cols { (img.width(), img.height()) } else { (img.height(), img.width()) };
    (0..n).map(|a| (0..m).all(|b| blank(if cols { img.get_pixel(a, b) } else { img.get_pixel(b, a) }, &bg))).collect()
}

/// 똑같이 n 칸으로 나눴을 때 칸 경계가 모두 빈 줄에 걸리고, 칸마다 그림이 있으면 true
fn cuts_clean(empty: &[bool], n: usize) -> bool {
    let len = empty.len();
    let step = len as f64 / n as f64;
    if step < 4.0 {
        return false;
    }
    let tol = ((step / 16.0) as usize).max(1);
    for i in 1..n {
        let x = (i as f64 * step).round() as usize;
        let lo = x.saturating_sub(tol);
        let hi = (x + tol).min(len - 1);
        if !(lo..=hi).any(|k| empty[k]) {
            return false;
        }
    }
    (0..n).all(|i| {
        let (a, b) = ((i as f64 * step).round() as usize, (((i + 1) as f64 * step).round() as usize).min(len));
        empty[a..b].iter().any(|e| !e)
    })
}

/// 몇 칸으로 나뉘나 — 경계가 모두 빈 줄인 가장 큰 칸 수 (없으면 1)
fn count_cells(empty: &[bool]) -> usize {
    // 그림이 아예 없는 줄뿐이면 나누지 않는다
    if empty.iter().all(|e| *e) {
        return 1;
    }
    (2..=64usize.min(empty.len() / 4)).rev().find(|&n| cuts_clean(empty, n)).unwrap_or(1)
}

/// 동작 칸이 나뉜 한 장이면 칸들을 (왼쪽 위부터 가로로) 돌려준다. 나눌 수 없으면 빈 목록.
pub fn split_sheet(img: &RgbaImage) -> Vec<RgbaImage> {
    let (w, h) = img.dimensions();
    if w < 8 || h < 8 {
        return vec![];
    }
    let mut cols = count_cells(&empty_lines(img, true));
    let mut rows = count_cells(&empty_lines(img, false));
    if cols * rows < 2 {
        // 칸 사이가 붙어 있어도 가로(세로)가 정확히 몇 배면 정사각 칸으로 본다 — 예: 1200×300 → 4칸
        if w % h == 0 && (2..=64).contains(&(w / h)) {
            cols = (w / h) as usize;
            rows = 1;
        } else if h % w == 0 && (2..=64).contains(&(h / w)) {
            cols = 1;
            rows = (h / w) as usize;
        } else {
            return vec![];
        }
    }
    let (cw, ch) = (w / cols as u32, h / rows as u32);
    let bg = *img.get_pixel(0, 0);
    let mut out = Vec::new();
    for r in 0..rows as u32 {
        for c in 0..cols as u32 {
            let cell = image::imageops::crop_imm(img, c * cw, r * ch, cw, ch).to_image();
            // 마지막 줄이 덜 찬 시트(7장을 4×2 에) — 빈 칸은 건너뛴다
            if cell.pixels().any(|p| !blank(p, &bg)) {
                out.push(cell);
            }
        }
    }
    if out.len() < 2 {
        return vec![];
    }
    out
}

// ── GIF 쓰기 ─────────────────────────────────────────────────────────────────

/// 모든 장을 담을 판 크기 — 가장 큰 가로 × 가장 큰 세로, 긴 변이 MAX_SIDE 를 넘으면 같은 비율로 줄인다
fn canvas_of(sizes: &[(u32, u32)]) -> (u32, u32, f64) {
    let w = sizes.iter().map(|s| s.0).max().unwrap_or(1).max(1);
    let h = sizes.iter().map(|s| s.1).max().unwrap_or(1).max(1);
    let k = (MAX_SIDE as f64 / w.max(h) as f64).min(1.0);
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1), k)
}

/// 한 장을 판 가운데에 (줄여서) 놓는다. 남는 곳은 투명
fn place(img: &RgbaImage, cw: u32, ch: u32, k: f64) -> RgbaImage {
    let img = if k < 1.0 {
        let (w, h) = (((img.width() as f64 * k).round() as u32).max(1), ((img.height() as f64 * k).round() as u32).max(1));
        image::imageops::resize(img, w, h, image::imageops::FilterType::Lanczos3)
    } else {
        img.clone()
    };
    if img.dimensions() == (cw, ch) {
        return img;
    }
    let mut c = RgbaImage::new(cw, ch);
    image::imageops::overlay(&mut c, &img, ((cw - img.width().min(cw)) / 2) as i64, ((ch - img.height().min(ch)) / 2) as i64);
    c
}

struct Writer {
    enc: GifEncoder<std::io::BufWriter<std::fs::File>>,
    ms: u32,
    n: usize,
}

impl Writer {
    fn new(dst: &Path, ms: u32) -> Result<Self, String> {
        let f = std::fs::File::create(dst).map_err(|e| format!("{} 를 만들지 못했어요: {e}", dst.display()))?;
        // 빠르기 10 = 색 고르기를 적당히 (1 이 가장 곱고 느림, 30 이 가장 빠름)
        let mut enc = GifEncoder::new_with_speed(std::io::BufWriter::new(f), 10);
        enc.set_repeat(Repeat::Infinite).map_err(|e| format!("GIF 저장 실패: {e}"))?;
        Ok(Self { enc, ms, n: 0 })
    }
    fn push(&mut self, img: RgbaImage) -> Result<(), String> {
        self.enc
            .encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(self.ms, 1)))
            .map_err(|e| format!("GIF 저장 실패: {e}"))?;
        self.n += 1;
        Ok(())
    }
}

fn finish(w: Writer, src: &Path, dst: &Path, cw: u32, ch: u32) -> Result<Made, String> {
    let n = w.n;
    drop(w);
    let bytes = std::fs::metadata(dst).map(|m| m.len()).map_err(|e| format!("저장한 파일을 확인하지 못했어요: {e}"))?;
    Ok(Made {
        out: OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: cw, height: ch, bytes, label: format!("움직이는 GIF {n}장") },
        frames: n,
    })
}

fn dst_for(src: &Path, out_dir: &Path) -> PathBuf {
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("그림");
    convert::unique_path(out_dir, &format!("{stem}_움직임.gif"))
}

/// 여러 장 → 이름 순서로 이은 GIF 하나. 이름은 맨 앞 그림을 따른다
pub fn from_files(files: &[String], out_dir: &Path, ms: u32, tick: &mut dyn FnMut(usize, usize)) -> Result<Made, String> {
    let mut files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
    files.sort_by(|a, b| natural_cmp(&name_of(a), &name_of(b)));
    if files.len() < 2 {
        return Err("여러 장을 같이 끌어 주세요".into());
    }
    if files.len() > MAX_FRAMES {
        return Err(format!("{MAX_FRAMES}장까지 이을 수 있어요 ({}장을 끌었어요)", files.len()));
    }
    // 머리말만 읽어 판 크기를 먼저 정한다 — 그림을 다 펼쳐 두지 않고 한 장씩 읽어 바로 쓴다
    let sizes: Vec<(u32, u32)> = files
        .iter()
        .map(|f| image::image_dimensions(f).map_err(|e| format!("{} 를 읽지 못했어요: {e}", name_of(f))))
        .collect::<Result<_, _>>()?;
    let (cw, ch, k) = canvas_of(&sizes);
    let dst = dst_for(&files[0], out_dir);
    let mut w = Writer::new(&dst, ms)?;
    let total = files.len();
    for (i, f) in files.iter().enumerate() {
        tick(i, total);
        let r = convert::load(f).and_then(|img| w.push(place(&img, cw, ch, k)));
        if let Err(e) = r {
            drop(w);
            let _ = std::fs::remove_file(&dst);
            return Err(e);
        }
    }
    finish(w, &files[0], &dst, cw, ch)
}

/// 한 장 → 동작 칸이 나뉘어 있으면 이은 GIF. 나눌 수 없는 그림이면 Ok(None)
pub fn from_sheet(src: &Path, out_dir: &Path, ms: u32) -> Result<Option<Made>, String> {
    let img = convert::load(src)?;
    let cells = split_sheet(&img);
    if cells.len() < 2 {
        return Ok(None);
    }
    let cells = &cells[..cells.len().min(MAX_FRAMES)];
    let (cw, ch, k) = canvas_of(&cells.iter().map(|c| c.dimensions()).collect::<Vec<_>>());
    let dst = dst_for(src, out_dir);
    let mut w = Writer::new(&dst, ms)?;
    for c in cells {
        if let Err(e) = w.push(place(c, cw, ch, k)) {
            drop(w);
            let _ = std::fs::remove_file(&dst);
            return Err(e);
        }
    }
    finish(w, src, &dst, cw, ch).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mago-anim-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 만든 GIF 의 장 수 · 한 장 크기 · 한 장 시간(ms)
    fn read_gif(p: &str) -> (usize, (u32, u32), u32) {
        let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(std::fs::File::open(p).unwrap())).unwrap();
        let frames = dec.into_frames().collect_frames().unwrap();
        let (n, d) = frames[0].delay().numer_denom_ms();
        (frames.len(), frames[0].buffer().dimensions(), n / d)
    }

    /// 투명 바탕에 칸마다 다른 색 공 (칸 사이는 비어 있다)
    fn sheet(cols: u32, rows: u32, cell: u32, opaque_bg: bool) -> RgbaImage {
        RgbaImage::from_fn(cols * cell, rows * cell, |x, y| {
            let (cx, cy) = (x % cell, y % cell);
            let k = (y / cell) * cols + x / cell;
            let m = cell / 6;
            if cx > m && cx < cell - m && cy > m && cy < cell - m {
                Rgba([(40 * k) as u8, 120, 200 - (20 * k) as u8, 255])
            } else if opaque_bg {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        })
    }

    #[test]
    fn numbers_sort_like_people_count() {
        let mut v = vec!["동작10.png", "동작2.png", "동작1.png", "동작 3.png", "동작02b.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["동작 3.png", "동작1.png", "동작2.png", "동작02b.png", "동작10.png"]);
        assert_eq!(natural_cmp("a2", "a10"), Ordering::Less);
        assert_eq!(natural_cmp("A1", "a1"), Ordering::Equal);
    }

    #[test]
    fn a_strip_with_gaps_splits_into_its_cells() {
        assert_eq!(split_sheet(&sheet(4, 1, 100, false)).len(), 4, "투명 칸 사이");
        assert_eq!(split_sheet(&sheet(6, 1, 90, true)).len(), 6, "흰 바탕 칸 사이");
        assert_eq!(split_sheet(&sheet(3, 2, 80, false)).len(), 6, "3×2 격자");
        assert_eq!(split_sheet(&sheet(1, 5, 64, true)).len(), 5, "세로로 쌓인 시트");
        let c = split_sheet(&sheet(4, 1, 100, false));
        assert_eq!(c[0].dimensions(), (100, 100));
        assert_ne!(c[0].get_pixel(50, 50), c[3].get_pixel(50, 50), "칸 순서대로 다른 그림");
    }

    #[test]
    fn a_grid_with_a_half_empty_last_row_skips_the_blank_cells() {
        let mut img = sheet(4, 2, 60, false);
        // 마지막 줄의 오른쪽 한 칸을 비운다 → 7장
        for y in 60..120 {
            for x in 180..240 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
        assert_eq!(split_sheet(&img).len(), 7);
    }

    #[test]
    fn touching_cells_split_when_the_width_is_an_exact_multiple() {
        // 칸 사이가 붙은 그림 — 1200×300 은 4칸
        let img = RgbaImage::from_fn(1200, 300, |x, y| Rgba([(x / 300 * 60) as u8, (y % 255) as u8, 90, 255]));
        assert_eq!(split_sheet(&img).len(), 4);
    }

    #[test]
    fn an_ordinary_photo_is_not_split() {
        // 잡음 사진 1600×900 — 빈 줄도 없고 배수도 아니다
        let mut seed = 3u32;
        let img = RgbaImage::from_fn(1600, 900, |_, _| {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let v = (seed >> 16) as u8;
            Rgba([v, v ^ 0x5a, 200, 255])
        });
        assert!(split_sheet(&img).is_empty());
        // 가운데에 공 하나만 있는 투명 그림도 한 장이다
        let one = sheet(1, 1, 200, false);
        assert!(split_sheet(&one).is_empty());
    }

    #[test]
    fn a_sheet_becomes_a_looping_gif_next_to_the_original() {
        let d = tmp("sheet");
        let src = d.join("걷기.png");
        convert::save(&sheet(5, 1, 120, false), &src, convert::OutFormat::Png, 90, [255; 4]).unwrap();
        let m = from_sheet(&src, &d, 100).unwrap().expect("나뉘어야 한다");
        assert!(m.out.file.ends_with("걷기_움직임.gif"), "{}", m.out.file);
        assert_eq!(m.frames, 5);
        assert_eq!(read_gif(&m.out.file), (5, (120, 120), 100));
        assert!(src.exists(), "원본은 그대로");
        // 나눌 수 없는 한 장은 None — 보통 GIF 로 바꾸러 간다
        let plain = d.join("공.png");
        convert::save(&sheet(1, 1, 120, false), &plain, convert::OutFormat::Png, 90, [255; 4]).unwrap();
        assert!(from_sheet(&plain, &d, 100).unwrap().is_none());
    }

    #[test]
    fn several_files_join_in_name_order_on_one_canvas() {
        let d = tmp("files");
        let mut files = Vec::new();
        // 크기가 다른 세 장, 이름 순서는 일부러 섞어서 넘긴다
        for (name, w, h, r) in [("동작10.png", 90, 60, 250u8), ("동작2.png", 120, 80, 120), ("동작1.png", 100, 100, 10)] {
            let p = d.join(name);
            convert::save(&RgbaImage::from_pixel(w, h, Rgba([r, 0, 0, 255])), &p, convert::OutFormat::Png, 90, [255; 4]).unwrap();
            files.push(p.to_string_lossy().to_string());
        }
        let mut ticks = 0;
        let m = from_files(&files, &d, 200, &mut |_, _| ticks += 1).unwrap();
        assert_eq!(ticks, 3);
        assert!(m.out.file.ends_with("동작1_움직임.gif"), "맨 앞(이름 순) 그림 이름으로: {}", m.out.file);
        assert_eq!(read_gif(&m.out.file), (3, (120, 100), 200), "가장 큰 가로 × 가장 큰 세로");
        // 첫 장은 동작1 (빨강 10)
        let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(std::fs::File::open(&m.out.file).unwrap())).unwrap();
        let f = dec.into_frames().collect_frames().unwrap();
        assert!(f[0].buffer().get_pixel(60, 50)[0] < 40, "첫 장은 동작1");
        assert!(f[2].buffer().get_pixel(60, 50)[0] > 200, "마지막 장은 동작10");
        assert!(f[2].buffer().get_pixel(2, 2)[3] < 16, "작은 그림의 남는 곳은 투명");
    }

    #[test]
    fn big_frames_are_shrunk_to_the_side_limit() {
        assert_eq!(canvas_of(&[(1600, 1200), (400, 300)]), (800, 600, 0.5));
        assert_eq!(canvas_of(&[(300, 200)]), (300, 200, 1.0));
    }
}
