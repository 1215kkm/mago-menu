//! 누끼 (배경 지우기) — 10-20 CEO "이미지선택했을때 누끼버튼".
//! **사진은 PC 밖으로 나가지 않는다.** 윈도우에 들어 있는 기계 학습 엔진(Windows ML)으로 이 PC 에서 돌린다.
//! 모델은 IS-Net(isnet-general-use, Apache-2.0 — rembg 가 쓰는 것). 처음 한 번 GitHub 에서 받아
//! `%LOCALAPPDATA%\com.mago.app\models\` 에 두고, 받은 파일은 SHA-256 으로 확인한다 (받는 것만 있고 보내는 것은 없다).
//! 결과: 원본 옆 `이름_누끼.png` (투명 배경).

use crate::convert::convert::{self, unique_path, OutFormat};
use crate::convert::OutputFile;
use std::path::{Path, PathBuf};

const MODEL_FILE: &str = "isnet-general-use.onnx";
const MODEL_HOST: &str = "github.com";
const MODEL_PATH: &str = "/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx";
const MODEL_SHA256: &str = "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a";
/// 화면에 알리는 크기 (MB)
pub const MODEL_MB: u32 = 179;
/// 모델이 보는 그림 크기
const SIDE: u32 = 1024;

fn model_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("com.mago.app").join("models")
}

pub fn model_path() -> PathBuf {
    model_dir().join(MODEL_FILE)
}

/// 누끼 도구를 이미 받아 두었나 (고리 칸 안내 글이 달라진다)
pub fn ready() -> bool {
    std::fs::metadata(model_path()).map(|m| m.len() > 1_000_000).unwrap_or(false)
}

/// 원본 옆에 `이름_누끼.png`. progress: 0~1 (처음엔 받기 0~0.8, 그 뒤 지우기)
pub fn cutout(src: &Path, out_dir: &Path, progress: &mut dyn FnMut(f64)) -> Result<OutputFile, String> {
    let img = convert::load(src)?;
    let model = ensure_model(&mut |f| progress(f * 0.8))?;
    progress(0.85);
    let alpha = mask(&model, &img)?;
    progress(0.95);
    let mut out = img;
    for (p, a) in out.pixels_mut().zip(alpha.iter()) {
        p[3] = ((p[3] as u32 * *a as u32) / 255) as u8;
    }
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("그림");
    let dst = unique_path(out_dir, &format!("{stem}_누끼.png"));
    let bytes = convert::save(&out, &dst, OutFormat::Png, 90, [255, 255, 255, 255])?;
    Ok(OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: out.width(), height: out.height(), bytes, label: "누끼".into() })
}

// ── 모델 받기 ────────────────────────────────────────────────────────────────

fn ensure_model(progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
    let p = model_path();
    if ready() {
        return Ok(p);
    }
    std::fs::create_dir_all(model_dir()).map_err(|e| format!("누끼 도구 자리를 만들지 못했어요: {e}"))?;
    let part = p.with_extension("part");
    crate::log(&format!("[누끼] 도구 받기 시작 ({MODEL_MB}MB)"));
    let got = download(MODEL_HOST, MODEL_PATH, &part, progress);
    let sum = match got {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            crate::log(&format!("[누끼] 도구 받기 실패 — {e}"));
            return Err(e);
        }
    };
    if sum != MODEL_SHA256 {
        let _ = std::fs::remove_file(&part);
        crate::log(&format!("[누끼] 받은 파일 지문이 다름 {sum}"));
        return Err("받은 누끼 도구가 원본과 달라 쓰지 않았어요".into());
    }
    std::fs::rename(&part, &p).map_err(|e| format!("누끼 도구를 저장하지 못했어요: {e}"))?;
    crate::log("[누끼] 도구 받음 · 지문 확인");
    Ok(p)
}

/// https 로 받아 파일에 쓰고 SHA-256(소문자 16진)을 돌려준다. WinHTTP — 윈도우 프록시 설정을 따른다
fn download(host: &str, path: &str, dst: &Path, progress: &mut dyn FnMut(f64)) -> Result<String, String> {
    use std::io::Write;
    use windows_sys::Win32::Networking::WinHttp::*;
    let w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    struct H(*mut core::ffi::c_void);
    impl Drop for H {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
    let err = |what: &str| format!("누끼 도구를 받지 못했어요 ({what} {})", std::io::Error::last_os_error());
    unsafe {
        let s = H(WinHttpOpen(w("MAGO").as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0));
        if s.0.is_null() {
            return Err(err("열기"));
        }
        let c = H(WinHttpConnect(s.0, w(host).as_ptr(), INTERNET_DEFAULT_HTTPS_PORT as u16, 0));
        if c.0.is_null() {
            return Err(err("연결"));
        }
        let r = H(WinHttpOpenRequest(c.0, w("GET").as_ptr(), w(path).as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), WINHTTP_FLAG_SECURE));
        if r.0.is_null() {
            return Err(err("요청"));
        }
        if WinHttpSendRequest(r.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 || WinHttpReceiveResponse(r.0, std::ptr::null_mut()) == 0 {
            return Err(err("응답"));
        }
        let num = |q: u32| -> u32 {
            let mut v = 0u32;
            let mut n = 4u32;
            WinHttpQueryHeaders(r.0, q | WINHTTP_QUERY_FLAG_NUMBER, std::ptr::null(), &mut v as *mut u32 as *mut _, &mut n, std::ptr::null_mut());
            v
        };
        let code = num(WINHTTP_QUERY_STATUS_CODE);
        if code != 200 {
            return Err(format!("누끼 도구를 받지 못했어요 (서버 {code})"));
        }
        let total = num(WINHTTP_QUERY_CONTENT_LENGTH) as f64;
        let mut f = std::io::BufWriter::new(std::fs::File::create(dst).map_err(|e| format!("누끼 도구를 저장하지 못했어요: {e}"))?);
        let mut sha = Sha256::new()?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0f64;
        loop {
            let mut n = 0u32;
            if WinHttpReadData(r.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut n) == 0 {
                return Err(err("받기"));
            }
            if n == 0 {
                break;
            }
            let chunk = &buf[..n as usize];
            f.write_all(chunk).map_err(|e| format!("누끼 도구를 저장하지 못했어요: {e}"))?;
            sha.update(chunk)?;
            done += n as f64;
            if total > 0.0 {
                progress((done / total).min(1.0));
            }
        }
        f.flush().map_err(|e| format!("누끼 도구를 저장하지 못했어요: {e}"))?;
        if total > 0.0 && (done - total).abs() > 0.5 {
            return Err("누끼 도구를 받다가 끊겼어요".into());
        }
        sha.finish()
    }
}

/// 윈도우 BCrypt 의 SHA-256 (조각씩)
struct Sha256(windows_sys::Win32::Security::Cryptography::BCRYPT_HASH_HANDLE);

impl Sha256 {
    fn new() -> Result<Self, String> {
        use windows_sys::Win32::Security::Cryptography::*;
        let mut h = std::ptr::null_mut();
        let st = unsafe { BCryptCreateHash(BCRYPT_SHA256_ALG_HANDLE, &mut h, std::ptr::null_mut(), 0, std::ptr::null(), 0, 0) };
        if st != 0 {
            return Err(format!("지문 계산을 시작하지 못했어요 ({st:#x})"));
        }
        Ok(Sha256(h))
    }
    fn update(&mut self, b: &[u8]) -> Result<(), String> {
        let st = unsafe { windows_sys::Win32::Security::Cryptography::BCryptHashData(self.0, b.as_ptr(), b.len() as u32, 0) };
        if st != 0 {
            return Err(format!("지문을 계산하지 못했어요 ({st:#x})"));
        }
        Ok(())
    }
    fn finish(self) -> Result<String, String> {
        let mut out = [0u8; 32];
        let st = unsafe { windows_sys::Win32::Security::Cryptography::BCryptFinishHash(self.0, out.as_mut_ptr(), 32, 0) };
        if st != 0 {
            return Err(format!("지문을 계산하지 못했어요 ({st:#x})"));
        }
        Ok(out.iter().map(|b| format!("{b:02x}")).collect())
    }
}

impl Drop for Sha256 {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Security::Cryptography::BCryptDestroyHash(self.0) };
    }
}

// ── 모델 돌리기 (Windows ML) ─────────────────────────────────────────────────

/// 그림 크기 그대로의 알파(0~255)
fn mask(model: &Path, img: &image::RgbaImage) -> Result<Vec<u8>, String> {
    use windows::core::{Interface, HSTRING, IInspectable};
    use windows::AI::MachineLearning::*;
    let e = |what: &'static str| move |x: windows::core::Error| format!("배경을 지우지 못했어요 ({what}: {})", x.message());
    // 이 일꾼 스레드에서 WinRT 를 쓴다
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    // 모델 입력: 1024×1024, 0~1 로 나눈 뒤 0.5 를 뺀다 (rembg isnet 과 같게)
    let small = image::imageops::resize(img, SIDE, SIDE, image::imageops::FilterType::Lanczos3);
    let plane = (SIDE * SIDE) as usize;
    let mut data = vec![0f32; plane * 3];
    for (i, p) in small.pixels().enumerate() {
        // 투명한 곳은 흰 바탕 위에 놓인 것으로 본다
        let a = p[3] as f32 / 255.0;
        for c in 0..3 {
            data[c * plane + i] = (p[c] as f32 / 255.0) * a + (1.0 - a) - 0.5;
        }
    }
    let m = LearningModel::LoadFromFilePath(&HSTRING::from(model.as_os_str())).map_err(e("도구 열기"))?;
    let input = m.InputFeatures().and_then(|v| v.GetAt(0)).and_then(|f| f.Name()).map_err(e("입력"))?;
    let output = m.OutputFeatures().and_then(|v| v.GetAt(0)).and_then(|f| f.Name()).map_err(e("출력"))?;
    // 그래픽 카드(DirectX)가 되면 빠르다 — 안 되면 CPU
    let session = LearningModelDevice::Create(LearningModelDeviceKind::DirectX)
        .and_then(|d| LearningModelSession::CreateFromModelOnDevice(&m, &d))
        .or_else(|_| LearningModelDevice::Create(LearningModelDeviceKind::Cpu).and_then(|d| LearningModelSession::CreateFromModelOnDevice(&m, &d)))
        .map_err(e("준비"))?;
    let binding = LearningModelBinding::CreateFromSession(&session).map_err(e("묶기"))?;
    let t = TensorFloat::CreateFromShapeArrayAndDataArray(&[1, 3, SIDE as i64, SIDE as i64], &data).map_err(e("입력 만들기"))?;
    binding.Bind(&input, &t.cast::<IInspectable>().map_err(e("입력 바꾸기"))?).map_err(e("입력 넣기"))?;
    let res = session.Evaluate(&binding, &HSTRING::from("mago")).map_err(e("돌리기"))?;
    let out: TensorFloat = res.Outputs().and_then(|o| o.Lookup(&output)).and_then(|v| v.cast()).map_err(e("결과"))?;
    let view = out.GetAsVectorView().map_err(e("결과 읽기"))?;
    let mut pred = vec![0f32; plane];
    let n = view.GetMany(0, &mut pred).map_err(e("결과 읽기"))? as usize;
    if n < plane {
        return Err("배경을 지우지 못했어요 (결과 크기가 달라요)".into());
    }
    // 0~1 로 펴고 원래 크기로 늘린다
    let (lo, hi) = pred.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = (hi - lo).max(1e-6);
    let gray = image::GrayImage::from_fn(SIDE, SIDE, |x, y| image::Luma([(((pred[(y * SIDE + x) as usize] - lo) / span) * 255.0).round() as u8]));
    let full = image::imageops::resize(&gray, img.width(), img.height(), image::imageops::FilterType::Triangle);
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut a = full.into_raw();
    fill_small_holes(&mut a, w, h);
    Ok(a)
}

/// 몸 안에 갇힌 작은 구멍을 채운다 — 눈 흰자 · 귀 안쪽처럼 밝은 곳을 바탕으로 잘못 본 것.
/// 바깥(그림 테두리)과 이어진 바탕은 그대로, 갇힌 구멍 중 큰 것(그림의 2% 이상 — 도넛 가운데 · 팔과 몸 사이)도 그대로 둔다.
fn fill_small_holes(a: &mut [u8], w: usize, h: usize) {
    const CLEAR: u8 = 200;
    let n = w * h;
    // 0 = 아직, 1 = 바깥 바탕, 2 = 갇힌 구멍(확인함)
    let mut seen = vec![0u8; n];
    let mut stack = Vec::new();
    let flood = |start: usize, seen: &mut Vec<u8>, stack: &mut Vec<usize>, mark: u8, a: &[u8]| -> Vec<usize> {
        let mut got = Vec::new();
        stack.clear();
        stack.push(start);
        seen[start] = mark;
        while let Some(i) = stack.pop() {
            got.push(i);
            let (x, y) = (i % w, i / w);
            let mut go = |j: usize| {
                if seen[j] == 0 && a[j] < CLEAR {
                    seen[j] = mark;
                    stack.push(j);
                }
            };
            if x > 0 { go(i - 1); }
            if x + 1 < w { go(i + 1); }
            if y > 0 { go(i - w); }
            if y + 1 < h { go(i + w); }
        }
        got
    };
    for x in 0..w {
        for i in [x, (h - 1) * w + x] {
            if seen[i] == 0 && a[i] < CLEAR {
                flood(i, &mut seen, &mut stack, 1, a);
            }
        }
    }
    for y in 0..h {
        for i in [y * w, y * w + w - 1] {
            if seen[i] == 0 && a[i] < CLEAR {
                flood(i, &mut seen, &mut stack, 1, a);
            }
        }
    }
    let big = n / 50;
    for i in 0..n {
        if seen[i] == 0 && a[i] < CLEAR {
            let hole = flood(i, &mut seen, &mut stack, 2, a);
            if hole.len() < big {
                for j in hole {
                    a[j] = 255;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_holes_inside_are_filled_but_big_holes_and_the_outside_stay_clear() {
        // 100×100: 가운데 60×60 몸, 몸 안에 4×4 작은 구멍(눈)과 30×30 큰 구멍(도넛 가운데)
        let (w, h) = (100usize, 100usize);
        let mut a = vec![0u8; w * h];
        for y in 20..80 {
            for x in 20..80 {
                a[y * w + x] = 255;
            }
        }
        for y in 25..29 {
            for x in 25..29 {
                a[y * w + x] = 40;
            }
        }
        for y in 40..70 {
            for x in 40..70 {
                a[y * w + x] = 0;
            }
        }
        fill_small_holes(&mut a, w, h);
        assert_eq!(a[26 * w + 26], 255, "눈 같은 작은 구멍은 채운다");
        assert_eq!(a[55 * w + 55], 0, "큰 구멍은 그대로");
        assert_eq!(a[5 * w + 5], 0, "바깥 바탕은 그대로");
    }

    #[test]
    fn sha256_matches_a_known_value() {
        let mut s = Sha256::new().unwrap();
        s.update(b"ab").unwrap();
        s.update(b"c").unwrap();
        assert_eq!(s.finish().unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    /// 손으로 돌리는 확인 — 이 PC 에 받아 둔 모델(또는 MAGO_CUTOUT_MODEL)로 그림 하나의 배경을 지운다
    #[test]
    #[ignore = "179MB 모델이 있어야 한다 — 손으로 돌린다"]
    fn removes_the_background_with_windows_ml() {
        let model = std::env::var_os("MAGO_CUTOUT_MODEL").map(PathBuf::from).unwrap_or_else(model_path);
        let d = std::env::temp_dir().join(format!("mago-cut-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // 흰 바탕 가운데 빨간 원
        let src = d.join("원.png");
        let img = image::RgbaImage::from_fn(400, 300, |x, y| {
            let (dx, dy) = (x as f32 - 200.0, y as f32 - 150.0);
            if dx * dx + dy * dy < 90.0 * 90.0 { image::Rgba([220, 30, 40, 255]) } else { image::Rgba([250, 250, 250, 255]) }
        });
        convert::save(&img, &src, OutFormat::Png, 90, [255; 4]).unwrap();
        let a = mask(&model, &img).unwrap();
        let at = |x: u32, y: u32| a[(y * 400 + x) as usize];
        assert!(at(200, 150) > 200, "가운데(원)는 남는다 {}", at(200, 150));
        assert!(at(5, 5) < 40, "구석(바탕)은 지운다 {}", at(5, 5));
    }
}
