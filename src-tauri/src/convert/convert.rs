// 컴뚝딱(D:\computer imagery/convert.rs)에서 그대로 가져왔다. 일괄 변환(ConvertRequest·run)은 지금 안 쓰지만 나중 메뉴(용량 줄이기 등)용으로 둔다.
#![allow(dead_code)]
//! 이미지 읽기·쓰기 공통부 + 포맷 변환.
//!
//! **EXIF 는 자동으로 사라진다**: 우리는 파일을 픽셀로 풀었다가 다시 인코딩하므로 원본의 EXIF·GPS 가
//! 결과에 실려 가지 않는다. "개인정보(촬영 위치)가 따라가지 않는다" 를 기능으로 내세울 수 있다.
//! (테스트 `exif_is_dropped_on_reencode` 가 실제 EXIF 를 넣은 JPG 로 확인한다.)
//!
//! WebP 는 무손실만 지원한다 — image 크레이트(image-webp)가 지금 무손실 인코더만 갖고 있기 때문.
//! 그래서 WebP 로 바꿀 때 quality 값은 무시되고, 사진은 JPG 보다 파일이 커질 수 있다.

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::codecs::webp::WebPEncoder;
use image::{ImageEncoder, ImageReader, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::BufWriter;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutFormat {
    Png,
    Jpg,
    Webp,
}

impl OutFormat {
    pub fn ext(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpg => "jpg",
            OutFormat::Webp => "webp",
        }
    }

    /// 투명을 담을 수 있는 포맷인가. JPG 는 못 담으므로 배경색으로 깔아야 한다.
    pub fn supports_alpha(self) -> bool {
        self != OutFormat::Jpg
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "png" => Some(OutFormat::Png),
            "jpg" | "jpeg" => Some(OutFormat::Jpg),
            "webp" => Some(OutFormat::Webp),
            _ => None,
        }
    }

    /// 원본 파일 경로에서 포맷 이어받기 (지원하지 않는 확장자면 PNG).
    pub fn from_path(p: &Path) -> Self {
        p.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_name)
            .unwrap_or(OutFormat::Png)
    }
}

/// 우리가 읽을 수 있는 확장자. 이 목록에 없는 파일은 아예 열지 않는다 (엉뚱한 파일을 이미지로 오해하지 않게).
pub const READABLE_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif"];

pub fn is_readable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| READABLE_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// 한 변의 최대 길이. 포토샵 최대 캔버스(30000px)에 맞췄다 — 사용자가 고른 사진을 우리가 먼저 거절하지 않게.
pub const MAX_SIDE: u32 = 30_000;
/// 푸는 동안 허용하는 최대 메모리. RGBA 기준 약 1억3천만 픽셀(예: 13000×10000)까지 — 소비자 사진은 전부 들어온다.
pub const MAX_ALLOC: u64 = 512 * 1024 * 1024;
/// **바깥에서 받은 아이콘**은 훨씬 좁게 연다. 아이콘이 8192px 를 넘을 이유가 없다.
pub const ASSET_MAX_SIDE: u32 = 8_192;
pub const ASSET_MAX_ALLOC: u64 = 128 * 1024 * 1024;

/// 디코더 상한. **이걸 걸지 않으면 5MB PNG 하나가 수 GB 픽셀로 풀려 앱이 그냥 꺼진다**(디컴프레션 폭탄).
/// 사용자 눈에는 "컴뚝딱이 이유 없이 종료됨" 으로만 보이므로 원인을 찾기가 매우 어렵다.
/// `max_image_width/height` 는 강한 제한이라 *픽셀을 풀기 전에* 머리말(헤더)만 보고 끊는다.
fn limits(max_side: u32, max_alloc: u64) -> image::Limits {
    let mut l = image::Limits::no_limits();
    l.max_image_width = Some(max_side);
    l.max_image_height = Some(max_side);
    l.max_alloc = Some(max_alloc);
    l
}

pub fn load(path: &Path) -> Result<RgbaImage, String> {
    load_limited(path, MAX_SIDE, MAX_ALLOC)
}

/// 아이콘 팩처럼 **우리가 만들지 않은 파일**을 열 때. 상한이 좁다.
pub fn load_asset(path: &Path) -> Result<RgbaImage, String> {
    load_limited(path, ASSET_MAX_SIDE, ASSET_MAX_ALLOC)
}

pub fn load_limited(path: &Path, max_side: u32, max_alloc: u64) -> Result<RgbaImage, String> {
    if !is_readable(path) {
        return Err(format!("이미지 파일이 아닙니다: {}", path.display()));
    }
    let mut reader = ImageReader::open(path)
        .map_err(|e| format!("{} 를 열지 못했습니다: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("{} 형식을 알아보지 못했습니다: {e}", path.display()))?;
    reader.limits(limits(max_side, max_alloc));
    let img = reader.decode().map_err(|e| match e {
        image::ImageError::Limits(_) => format!(
            "{} 는 너무 큽니다(한 변 {max_side}px · 메모리 {}MB 까지). 이 파일은 건너뜁니다.",
            path.display(),
            max_alloc / 1024 / 1024
        ),
        e => format!("{} 를 읽지 못했습니다(파일이 깨졌을 수 있습니다): {e}", path.display()),
    })?;
    Ok(img.to_rgba8())
}

/// 투명 위에 배경색을 깔아 RGB 로. JPG 저장 전에 쓴다.
pub fn flatten_on(img: &RgbaImage, bg: [u8; 4]) -> image::RgbImage {
    let mut out = image::RgbImage::new(img.width(), img.height());
    for (x, y, p) in img.enumerate_pixels() {
        let a = p[0 + 3] as u32;
        let blend = |fg: u8, bgc: u8| ((fg as u32 * a + bgc as u32 * (255 - a)) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([blend(p[0], bg[0]), blend(p[1], bg[1]), blend(p[2], bg[2])]));
    }
    out
}

/// 저장. 돌려주는 값은 만들어진 파일 크기(바이트).
pub fn save(img: &RgbaImage, path: &Path, fmt: OutFormat, quality: u8, bg: [u8; 4]) -> Result<u64, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("폴더를 만들지 못했습니다 {}: {e}", dir.display()))?;
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{} 를 만들지 못했습니다: {e}", path.display()))?;
    let mut w = BufWriter::new(file);
    match fmt {
        OutFormat::Png => PngEncoder::new_with_quality(&mut w, CompressionType::Default, PngFilter::Adaptive)
            .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("PNG 저장 실패: {e}"))?,
        OutFormat::Jpg => {
            let rgb = flatten_on(img, bg);
            JpegEncoder::new_with_quality(&mut w, quality.clamp(1, 100))
                .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("JPG 저장 실패: {e}"))?;
        }
        OutFormat::Webp => WebPEncoder::new_lossless(&mut w)
            .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("WebP 저장 실패: {e}"))?,
    }
    drop(w);
    std::fs::metadata(path).map(|m| m.len()).map_err(|e| format!("저장한 파일을 확인하지 못했습니다: {e}"))
}

/// 목표 용량(KB) 안에 들어오도록 JPG 품질을 이분 탐색해 저장. PNG·WebP 는 품질 조절이 없어 그냥 저장한다.
/// 돌려주는 값: (파일 크기, 실제로 쓴 품질, 목표를 맞췄는지)
pub fn save_within(img: &RgbaImage, path: &Path, fmt: OutFormat, target_kb: u32, bg: [u8; 4]) -> Result<(u64, u8, bool), String> {
    let limit = target_kb as u64 * 1024;
    if fmt != OutFormat::Jpg {
        let size = save(img, path, fmt, 90, bg)?;
        return Ok((size, 0, size <= limit));
    }
    let (mut lo, mut hi) = (20u8, 95u8);
    let mut best: Option<(u64, u8)> = None;
    // 8단계면 20~95 를 1 단위까지 좁힌다. 매번 실제로 인코딩하므로 결과가 추정이 아니다.
    for _ in 0..8 {
        let mid = (lo + hi) / 2;
        let size = save(img, path, fmt, mid, bg)?;
        if size <= limit {
            best = Some((size, mid));
            lo = mid + 1;
        } else {
            if mid == 20 {
                break;
            }
            hi = mid - 1;
        }
        if lo > hi {
            break;
        }
    }
    match best {
        Some((_, q)) => {
            let size = save(img, path, fmt, q, bg)?;
            Ok((size, q, size <= limit))
        }
        // 품질 20 으로도 못 맞추면 20 으로 저장하고 "못 맞췄다" 고 알린다 — 조용히 실패하지 않는다.
        None => {
            let size = save(img, path, fmt, 20, bg)?;
            Ok((size, 20, size <= limit))
        }
    }
}

/// `{원본이름}_{가로}x{세로}.{확장자}` 같은 결과 파일명. `suffix` 가 비면 이름만 바꾼다.
pub fn out_name(src: &Path, suffix: &str, fmt: OutFormat) -> String {
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("이미지");
    if suffix.is_empty() {
        format!("{stem}.{}", fmt.ext())
    } else {
        format!("{stem}_{suffix}.{}", fmt.ext())
    }
}

/// 같은 이름이 있으면 `(2)`, `(3)` … 을 붙인다. 일괄 처리에서 앞 결과를 덮지 않기 위해.
pub fn unique_path(dir: &Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    for n in 2..10_000 {
        let cand = dir.join(format!("{stem} ({n}){ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    p
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub files: Vec<String>,
    pub out_dir: String,
    pub format: OutFormat,
    /// 1~100. 목표 용량을 주면 무시된다.
    pub quality: Option<u8>,
    /// 목표 용량(KB). 주면 JPG 품질을 이 안에 들어오게 낮춘다.
    pub target_kb: Option<u32>,
    /// JPG 로 바꿀 때 투명한 곳에 깔 색. 기본 흰색
    pub background: Option<[u8; 4]>,
    /// 파일 이름 꼬리표 (기본 없음 = 이름 그대로, 확장자만 바뀜)
    pub suffix: Option<String>,
}

/// 포맷 일괄 변환. EXIF 는 재인코딩 과정에서 자동으로 빠진다.
pub fn run(req: &ConvertRequest, mut on_progress: impl FnMut(usize, usize, &str)) -> Result<super::BatchResult, String> {
    if req.files.is_empty() {
        return Err("변환할 파일을 골라 주세요.".into());
    }
    let out_dir = std::path::PathBuf::from(&req.out_dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("저장 폴더를 만들지 못했습니다: {e}"))?;
    let bg = req.background.unwrap_or([255, 255, 255, 255]);
    let suffix = req.suffix.clone().unwrap_or_default();
    let total = req.files.len();
    let mut out = super::BatchResult { total, ..Default::default() };

    for (i, f) in req.files.iter().enumerate() {
        let src = Path::new(f);
        match load(src) {
            Ok(img) => {
                let path = unique_path(&out_dir, &out_name(src, &suffix, req.format));
                let saved = match req.target_kb {
                    Some(kb) if kb > 0 => save_within(&img, &path, req.format, kb, bg).map(|(b, q, ok)| {
                        (b, if ok { format!("{}KB 이하", kb) } else { format!("품질 {q} 로도 {kb}KB 를 못 맞춤") })
                    }),
                    _ => save(&img, &path, req.format, req.quality.unwrap_or(90), bg).map(|b| (b, req.format.ext().to_string())),
                };
                match saved {
                    Ok((bytes, label)) => out.outputs.push(super::OutputFile {
                        source: f.clone(),
                        file: path.to_string_lossy().to_string(),
                        width: img.width(),
                        height: img.height(),
                        bytes,
                        label,
                    }),
                    Err(e) => out.errors.push(super::BatchError { file: f.clone(), error: e }),
                }
            }
            Err(e) => out.errors.push(super::BatchError { file: f.clone(), error: e }),
        }
        on_progress(i + 1, total, f);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]))
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kom-img-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn format_names_and_alpha_support() {
        assert_eq!(OutFormat::from_name("JPEG"), Some(OutFormat::Jpg));
        assert_eq!(OutFormat::from_name(".png"), Some(OutFormat::Png));
        assert_eq!(OutFormat::from_name("psd"), None);
        assert!(!OutFormat::Jpg.supports_alpha());
        assert!(OutFormat::Png.supports_alpha());
        assert_eq!(OutFormat::from_path(Path::new("a/b.WEBP")), OutFormat::Webp);
        assert_eq!(OutFormat::from_path(Path::new("a/b.tif")), OutFormat::Png);
    }

    #[test]
    fn roundtrip_through_every_format() {
        let img = sample(40, 24);
        for fmt in [OutFormat::Png, OutFormat::Jpg, OutFormat::Webp] {
            let p = tmp(&format!("round.{}", fmt.ext()));
            let size = save(&img, &p, fmt, 92, [255, 255, 255, 255]).unwrap();
            assert!(size > 0, "{:?}: 빈 파일", fmt);
            let back = load(&p).unwrap();
            assert_eq!((back.width(), back.height()), (40, 24), "{:?}: 크기가 바뀌었다", fmt);
            if fmt != OutFormat::Jpg {
                // 무손실 포맷은 픽셀이 그대로여야 한다
                assert_eq!(back.as_raw(), img.as_raw(), "{:?}: 무손실인데 픽셀이 달라졌다", fmt);
            }
            let _ = std::fs::remove_file(&p);
        }
    }

    #[test]
    fn jpg_flattens_transparency_onto_background() {
        let mut img = RgbaImage::new(4, 4);
        for p in img.pixels_mut() {
            *p = image::Rgba([255, 0, 0, 0]); // 완전 투명한 빨강
        }
        let rgb = flatten_on(&img, [0, 0, 255, 255]);
        assert_eq!(rgb.get_pixel(0, 0).0, [0, 0, 255], "투명한 곳은 배경색이 보여야 한다");
        let half = flatten_on(&RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 128])), [0, 0, 0, 255]);
        assert_eq!(half.get_pixel(0, 0).0[0], 128, "반투명은 절반만 섞인다");
    }

    #[test]
    fn exif_is_dropped_on_reencode() {
        // EXIF(APP1) 조각을 끼운 JPG 를 만들어, 변환 결과에 그 표식이 남지 않는지 본다.
        let src = tmp("exif-src.jpg");
        save(&sample(16, 16), &src, OutFormat::Jpg, 90, [255; 4]).unwrap();
        let mut bytes = std::fs::read(&src).unwrap();
        let payload: Vec<u8> = b"Exif\0\0KOMTTUKTTAK-GPS-TEST".to_vec();
        let len = (payload.len() + 2) as u16;
        let mut app1 = vec![0xFF, 0xE1, (len >> 8) as u8, (len & 0xFF) as u8];
        app1.extend_from_slice(&payload);
        bytes.splice(2..2, app1); // SOI 바로 뒤에 APP1 삽입
        std::fs::write(&src, &bytes).unwrap();
        assert!(String::from_utf8_lossy(&std::fs::read(&src).unwrap()).contains("KOMTTUKTTAK-GPS-TEST"), "테스트 준비 실패");

        let dst = tmp("exif-out.jpg");
        let img = load(&src).unwrap();
        save(&img, &dst, OutFormat::Jpg, 90, [255; 4]).unwrap();
        let out = std::fs::read(&dst).unwrap();
        assert!(!String::from_utf8_lossy(&out).contains("KOMTTUKTTAK-GPS-TEST"), "EXIF 가 결과에 그대로 남았다");
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }

    #[test]
    fn target_size_search_hits_the_budget() {
        // 잡음이 많은 큰 이미지 = 잘 안 줄어드는 이미지. 목표를 주면 품질을 낮춰서라도 맞춘다.
        let img = RgbaImage::from_fn(600, 600, |x, y| {
            let n = ((x * 7919 + y * 104729) % 251) as u8;
            image::Rgba([n, n.wrapping_mul(3), n.wrapping_add(90), 255])
        });
        let p = tmp("budget.jpg");
        let full = save(&img, &p, OutFormat::Jpg, 95, [255; 4]).unwrap();
        let budget_kb = (full / 1024 / 3).max(8) as u32;
        let (size, q, ok) = save_within(&img, &p, OutFormat::Jpg, budget_kb, [255; 4]).unwrap();
        assert!(ok, "목표 {budget_kb}KB 를 못 맞췄다 (실제 {}KB, 품질 {q})", size / 1024);
        assert!(size <= budget_kb as u64 * 1024);
        assert!((20..=95).contains(&q), "품질 범위 밖: {q}");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn out_names_and_collision_suffix() {
        assert_eq!(out_name(Path::new(r"C:\a\사진.JPG"), "1080x1080", OutFormat::Png), "사진_1080x1080.png");
        assert_eq!(out_name(Path::new("사진.png"), "", OutFormat::Jpg), "사진.jpg");

        let dir = std::env::temp_dir().join(format!("kom-uniq-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = unique_path(&dir, "a.png");
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(&dir, "a.png");
        assert!(second.file_name().unwrap().to_string_lossy().contains("(2)"), "{second:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_image_files_are_refused_before_opening() {
        let p = tmp("가짜.exe");
        std::fs::write(&p, b"MZ").unwrap();
        assert!(load(&p).unwrap_err().contains("이미지 파일이 아닙니다"));
        let _ = std::fs::remove_file(&p);
    }

    /// 디컴프레션 폭탄 — 파일은 몇십 바이트인데 머리말에 "100000×100000" 이라고 적힌 PNG.
    /// 상한이 없으면 image 가 수십 GB 를 잡으려다 앱이 그냥 꺼진다(사용자 눈엔 원인 불명 종료).
    /// 헤더만 보고 끊어야 하므로 픽셀 데이터는 넣지 않는다 — 통과하면 *그 전에* 실패해야 한다는 뜻.
    #[test]
    fn a_decompression_bomb_is_refused_before_any_allocation() {
        fn crc32(data: &[u8]) -> u32 {
            let mut c = 0xffff_ffffu32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
                }
            }
            !c
        }
        fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
            let mut v = (body.len() as u32).to_be_bytes().to_vec();
            v.extend_from_slice(kind);
            v.extend_from_slice(body);
            let mut crc_in = kind.to_vec();
            crc_in.extend_from_slice(body);
            v.extend_from_slice(&crc32(&crc_in).to_be_bytes());
            v
        }
        // IHDR: 폭·높이 100000, 8비트 RGBA → 풀면 40GB
        let mut ihdr = 100_000u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&100_000u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&chunk(b"IHDR", &ihdr));
        png.extend_from_slice(&chunk(b"IDAT", &[0x78, 0x9c, 0x63, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01]));
        png.extend_from_slice(&chunk(b"IEND", &[]));
        assert!(png.len() < 200, "샘플 자체는 작아야 폭탄이다: {} 바이트", png.len());

        let p = tmp("폭탄.png");
        std::fs::write(&p, &png).unwrap();
        let e = load(&p).unwrap_err();
        assert!(e.contains("너무 큽니다"), "상한이 걸리지 않았다: {e}");
        // 아이콘 팩 경로는 더 좁은 상한이라 당연히 걸린다
        assert!(load_asset(&p).unwrap_err().contains("너무 큽니다"));
        let _ = std::fs::remove_file(&p);
    }

    /// 상한이 "아무거나 다 막는" 것이면 기능이 죽은 것이다 — 보통 그림은 그대로 열려야 한다.
    #[test]
    fn normal_images_still_open_under_the_limits() {
        let p = tmp("보통.png");
        save(&sample(600, 400), &p, OutFormat::Png, 92, [0, 0, 0, 0]).unwrap();
        assert_eq!(load(&p).unwrap().dimensions(), (600, 400));
        assert_eq!(load_asset(&p).unwrap().dimensions(), (600, 400));
        let _ = std::fs::remove_file(&p);
    }
}
