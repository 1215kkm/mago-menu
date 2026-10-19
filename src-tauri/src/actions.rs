//! 고리의 형식 바꾸기 말고 하는 일 (2026-10-20 — 파일 종류마다 다른 칸, 쓰레기통).
//! - `square` : 정사각 썸네일 1080×1080 (가운데를 채워 자름) → `이름_정사각.jpg`
//! - `small`  : 1MB 이하 JPG (화질을 찾아 내려 맞춤 — 카톡·메일용) → `이름_1MB.jpg`
//! - `recycle`: 쓰레기통에 먹인 파일을 **윈도우 휴지통으로** (되살릴 수 있다 — 영구 삭제 아님)
//! 원본은 고치지 않는다(휴지통만 예외 — 사용자가 쓰레기통에 직접 놓은 것).

use crate::convert::convert::{self, OutFormat};
use crate::convert::OutputFile;
use std::path::Path;

pub const SQUARE_PX: u32 = 1080;
pub const SMALL_KB: u32 = 1024;

fn stem(src: &Path) -> String {
    src.file_stem().and_then(|s| s.to_str()).unwrap_or("그림").to_string()
}

pub fn square(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let img = convert::load(src)?;
    // 한 변 = 1080 과 짧은 변 중 작은 쪽 (작은 그림을 억지로 키우지 않는다). 비율을 지키며 가운데를 채워 자른다
    let side = SQUARE_PX.min(img.width().min(img.height())).max(1);
    let sq = image::DynamicImage::ImageRgba8(img).resize_to_fill(side, side, image::imageops::FilterType::Lanczos3).to_rgba8();
    let dst = convert::unique_path(out_dir, &format!("{}_정사각.jpg", stem(src)));
    let bytes = convert::save(&sq, &dst, OutFormat::Jpg, 92, [255, 255, 255, 255])?;
    Ok(OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: side, height: side, bytes, label: "정사각".into() })
}

pub fn small(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let mut img = convert::load(src)?;
    // 화질만 내려서 안 되는 큰 사진은 긴 변 2560px 로 먼저 줄인다
    let long = img.width().max(img.height());
    if long > 2560 {
        let s = 2560.0 / long as f64;
        img = image::imageops::resize(&img, (img.width() as f64 * s).round() as u32, (img.height() as f64 * s).round() as u32, image::imageops::FilterType::Lanczos3);
    }
    let dst = convert::unique_path(out_dir, &format!("{}_1MB.jpg", stem(src)));
    let (mut bytes, mut q, mut ok) = convert::save_within(&img, &dst, OutFormat::Jpg, SMALL_KB, [255, 255, 255, 255])?;
    // 화질을 내려도 안 되면(잡티가 많은 사진) 크기를 3/4 씩 줄여 다시 — 다섯 번까지
    for _ in 0..5 {
        if ok {
            break;
        }
        img = image::imageops::resize(&img, (img.width() * 3 / 4).max(1), (img.height() * 3 / 4).max(1), image::imageops::FilterType::Lanczos3);
        (bytes, q, ok) = convert::save_within(&img, &dst, OutFormat::Jpg, SMALL_KB, [255, 255, 255, 255])?;
    }
    if !ok {
        let _ = std::fs::remove_file(&dst);
        return Err(format!("줄여도 {}KB 예요", bytes / 1024));
    }
    Ok(OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: img.width(), height: img.height(), bytes, label: format!("1MB 이하 (화질 {q})") })
}

/// 윈도우 휴지통으로 보낸다. 돌려주는 값: 보낸 수
#[cfg(windows)]
pub fn recycle(paths: &[String]) -> Result<usize, String> {
    use windows_sys::Win32::UI::Shell::{SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW};
    let live: Vec<&String> = paths.iter().filter(|p| Path::new(p).exists()).collect();
    if live.is_empty() {
        return Err("파일이 이미 없어요".into());
    }
    // 이름들을 \0 으로 잇고 끝에 \0 하나 더
    let mut from: Vec<u16> = Vec::new();
    for p in &live {
        from.extend(p.encode_utf16());
        from.push(0);
    }
    from.push(0);
    let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
    op.wFunc = FO_DELETE;
    op.pFrom = from.as_ptr();
    // ALLOWUNDO = 휴지통으로 (영구 삭제 아님). 묻는 창·진행 창은 띄우지 않는다 — 고리가 결과를 보여 준다
    op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16;
    let r = unsafe { SHFileOperationW(&mut op) };
    if r != 0 || op.fAnyOperationsAborted != 0 {
        return Err(format!("휴지통으로 보내지 못했어요 (코드 {r})"));
    }
    let gone = live.iter().filter(|p| !Path::new(p).exists()).count();
    Ok(gone)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("mago-act-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn square_fills_the_centre_and_keeps_the_original() {
        let d = tmp("sq");
        let src = d.join("가로.png");
        // 가로 1600×900: 왼쪽 빨강 · 가운데 초록 · 오른쪽 파랑
        let img = image::RgbaImage::from_fn(1600, 900, |x, _| if x < 350 { image::Rgba([255, 0, 0, 255]) } else if x < 1250 { image::Rgba([0, 200, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        convert::save(&img, &src, OutFormat::Png, 90, [255; 4]).unwrap();
        let o = square(&src, &d).unwrap();
        assert!(o.file.ends_with("가로_정사각.jpg"), "{}", o.file);
        let got = convert::load(Path::new(&o.file)).unwrap();
        assert_eq!(got.dimensions(), (900, 900), "긴 변 1600 이라도 짧은 변보다 크게 키우지 않고 1080 이하로");
        let c = got.get_pixel(450, 450);
        assert!(c[1] > 150 && c[0] < 60, "가운데는 초록: {c:?}");
        assert!(src.exists());
    }

    #[test]
    fn small_gets_under_one_megabyte() {
        let d = tmp("small");
        let src = d.join("큰사진.png");
        // 잡음이 많은 3000×2000 — 그냥 저장하면 수 MB
        let mut seed = 7u32;
        let img = image::RgbaImage::from_fn(3000, 2000, |_, _| {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let v = (seed >> 16) as u8;
            image::Rgba([v, v.wrapping_mul(3), v.wrapping_add(90), 255])
        });
        convert::save(&img, &src, OutFormat::Png, 90, [255; 4]).unwrap();
        let o = small(&src, &d).unwrap();
        assert!(o.bytes <= 1024 * 1024, "{}", o.bytes);
        assert!(o.file.ends_with("큰사진_1MB.jpg"));
        assert!(src.exists());
    }

    /// 손으로 돌리는 확인 — 진짜 휴지통으로 작은 파일 하나를 보낸다 (돌린 뒤 휴지통에 `mago-휴지통-시험.txt` 가 남는다)
    #[test]
    #[ignore = "진짜 휴지통에 파일을 하나 넣는다 — 손으로 돌린다"]
    fn recycle_really_moves_to_the_recycle_bin() {
        let d = tmp("bin");
        let f = d.join("mago-휴지통-시험.txt");
        std::fs::write(&f, "MAGO 쓰레기통 시험").unwrap();
        assert_eq!(recycle(&[f.to_string_lossy().into()]).unwrap(), 1);
        assert!(!f.exists());
    }
}
