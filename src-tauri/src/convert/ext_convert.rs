//! **확장자 바꾸기** (10-12 CEO "webp 를 다른 이미지 확장자들로, 서로 바꿀 수도 있고, 소리 wav 도 mp3 로 서로").
//!
//! 그림: png · jpg · webp · bmp · gif 를 서로. 소리: wav ↔ mp3.
//! PDF → jpg · png (10-16 CEO "각 장으로 저장, pdf 전체를 한 장으로 저장") — 그리기는 `pdf_render.rs`.
//! 이름만 바꾸는 게 아니라 **풀었다가 그 형식으로 다시 담는다** — 이름만 바꾼 파일은 프로그램이 못 연다.
//!
//! ## 지키는 것
//! - 원본은 건드리지 않는다. 결과는 저장 폴더에 새 파일로, 같은 이름이 있으면 `(2)` 를 붙인다.
//! - **투명은 살린다.** webp → png 는 알파까지 픽셀 그대로다(테스트가 손실 webp 표본으로 확인).
//!   투명을 못 담는 jpg · bmp 로 갈 때만 배경색(기본 흰색)을 깐다.
//! - 한 파일이 실패해도 나머지는 계속한다 (`BatchResult.errors`).
//! - 소리는 **조각 단위로 흘려** 바꾼다. 한 시간짜리 wav 를 통째로 메모리에 올리지 않는다.
//!
//! MP3 인코딩은 LAME(libmp3lame, LGPL) 을 쓴다 — 이 앱의 소스는 공개 저장소에 있다.

use super::convert::{self, OutFormat};
use super::{BatchError, BatchResult, OutputFile};
use image::RgbaImage;
use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, MonoPcm, Quality};
use serde::Deserialize;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const IMAGE_TARGETS: &[&str] = &["png", "jpg", "webp", "bmp", "gif"];
pub const AUDIO_TARGETS: &[&str] = &["mp3", "wav"];
pub const AUDIO_EXT: &[&str] = &["wav", "mp3"];
pub const PDF_TARGETS: &[&str] = &["jpg", "png"];
/// 한 장으로 이을 때의 상한. JPG 는 한 변 65,535px 이 끝이고, 그보다 훨씬 전에 보는 프로그램들이 버벅인다.
pub const ONE_MAX_HEIGHT: u64 = 60_000;
/// 한 장으로 이을 때 픽셀 수 상한 (RGBA 약 400MB)
pub const ONE_MAX_PIXELS: u64 = 100_000_000;

fn ext_of(p: &Path) -> String {
    let e = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if e == "jpeg" { "jpg".into() } else { e }
}

pub fn is_audio(p: &Path) -> bool {
    AUDIO_EXT.contains(&ext_of(p).as_str())
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExtRequest {
    pub files: Vec<String>,
    pub out_dir: String,
    /// 그림을 무엇으로 — png | jpg | webp | bmp | gif. 그림이 없으면 안 줘도 된다.
    pub image_format: Option<String>,
    /// 소리를 무엇으로 — mp3 | wav
    pub audio_format: Option<String>,
    /// JPG 품질 1~100 (기본 92)
    pub quality: Option<u8>,
    /// MP3 음질 kbps — 128 | 192 | 320 (기본 192)
    pub bitrate: Option<u32>,
    /// 투명을 못 담는 형식(jpg · bmp)으로 갈 때 깔 색. 기본 흰색
    pub background: Option<[u8; 4]>,
    /// PDF 를 무엇으로 — jpg | png (기본 jpg)
    pub pdf_format: Option<String>,
    /// PDF 를 어떻게 — "pages" 쪽마다 한 장씩 | "one" 전체를 세로로 이어 한 장 (기본 pages)
    pub pdf_mode: Option<String>,
    /// PDF 를 그릴 해상도 dpi — 150 | 300 (기본 150)
    pub pdf_dpi: Option<u32>,
}

pub fn is_pdf(p: &Path) -> bool {
    ext_of(p) == "pdf"
}

// ── 그림 ───────────────────────────────────────────────────────────────────

/// 그림 한 장. 돌려주는 값: (파일 크기, 가로, 세로)
pub fn convert_image(src: &Path, dst: &Path, target: &str, quality: u8, bg: [u8; 4]) -> Result<(u64, u32, u32), String> {
    let img = convert::load(src)?;
    let (w, h) = (img.width(), img.height());
    let bytes = match target {
        "png" => convert::save(&img, dst, OutFormat::Png, quality, bg)?,
        "jpg" => convert::save(&img, dst, OutFormat::Jpg, quality, bg)?,
        "webp" => convert::save(&img, dst, OutFormat::Webp, quality, bg)?,
        "bmp" => {
            // 32비트 BMP 의 투명은 읽는 프로그램마다 다르게 보인다 — 배경을 깔아 24비트로 담는다
            let rgb = convert::flatten_on(&img, bg);
            image::DynamicImage::ImageRgb8(rgb)
                .save_with_format(dst, image::ImageFormat::Bmp)
                .map_err(|e| format!("BMP 저장 실패: {e}"))?;
            std::fs::metadata(dst).map(|m| m.len()).map_err(|e| format!("저장한 파일을 확인하지 못했습니다: {e}"))?
        }
        "gif" => {
            image::DynamicImage::ImageRgba8(img)
                .save_with_format(dst, image::ImageFormat::Gif)
                .map_err(|e| format!("GIF 저장 실패: {e}"))?;
            std::fs::metadata(dst).map(|m| m.len()).map_err(|e| format!("저장한 파일을 확인하지 못했습니다: {e}"))?
        }
        other => return Err(format!("{other} 로는 바꿀 수 없어요.")),
    };
    Ok((bytes, w, h))
}

// ── 소리 ───────────────────────────────────────────────────────────────────

/// 풀린 소리 조각을 받는 쪽. `pcm` 은 채널이 번갈아 놓인(interleaved) 16비트.
trait Sink {
    fn write(&mut self, pcm: &[i16]) -> Result<(), String>;
    fn finish(self: Box<Self>) -> Result<(), String>;
}

struct WavSink {
    file: std::io::BufWriter<std::fs::File>,
    data_bytes: u64,
}

impl WavSink {
    fn new(dst: &Path, rate: u32, channels: u16) -> Result<Self, String> {
        let f = std::fs::File::create(dst).map_err(|e| format!("{} 를 만들지 못했습니다: {e}", dst.display()))?;
        let mut file = std::io::BufWriter::new(f);
        let block = channels * 2;
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&0u32.to_le_bytes()); // 끝나면 채운다
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes()); // PCM
        h.extend_from_slice(&channels.to_le_bytes());
        h.extend_from_slice(&rate.to_le_bytes());
        h.extend_from_slice(&(rate * block as u32).to_le_bytes());
        h.extend_from_slice(&block.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&0u32.to_le_bytes());
        file.write_all(&h).map_err(|e| format!("WAV 를 쓰지 못했습니다: {e}"))?;
        Ok(Self { file, data_bytes: 0 })
    }
}

impl Sink for WavSink {
    fn write(&mut self, pcm: &[i16]) -> Result<(), String> {
        let mut buf = Vec::with_capacity(pcm.len() * 2);
        for s in pcm {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        self.data_bytes += buf.len() as u64;
        self.file.write_all(&buf).map_err(|e| format!("WAV 를 쓰지 못했습니다: {e}"))
    }
    fn finish(mut self: Box<Self>) -> Result<(), String> {
        // WAV 는 크기 칸이 32비트다 — 4GB 를 넘으면 담을 수 없다
        if self.data_bytes > u32::MAX as u64 - 36 {
            return Err("WAV 로 담기에는 너무 길어요 (4GB 를 넘어요).".into());
        }
        let e = |e: std::io::Error| format!("WAV 를 마무리하지 못했습니다: {e}");
        self.file.flush().map_err(e)?;
        let f = self.file.get_mut();
        f.seek(SeekFrom::Start(4)).map_err(e)?;
        f.write_all(&((self.data_bytes + 36) as u32).to_le_bytes()).map_err(e)?;
        f.seek(SeekFrom::Start(40)).map_err(e)?;
        f.write_all(&(self.data_bytes as u32).to_le_bytes()).map_err(e)?;
        f.flush().map_err(e)
    }
}

struct Mp3Sink {
    enc: mp3lame_encoder::Encoder,
    file: std::io::BufWriter<std::fs::File>,
    channels: u16,
    out: Vec<u8>,
}

fn bitrate_of(kbps: u32) -> Bitrate {
    match kbps {
        0..=112 => Bitrate::Kbps96,
        113..=160 => Bitrate::Kbps128,
        161..=256 => Bitrate::Kbps192,
        _ => Bitrate::Kbps320,
    }
}

impl Mp3Sink {
    fn new(dst: &Path, rate: u32, channels: u16, kbps: u32) -> Result<Self, String> {
        let bad = |what: &str, e: String| format!("MP3 인코더를 준비하지 못했습니다 ({what}): {e}");
        let mut b = Builder::new().ok_or_else(|| "MP3 인코더를 만들지 못했습니다.".to_string())?;
        b.set_num_channels(channels as u8).map_err(|e| bad("채널", format!("{e:?}")))?;
        b.set_sample_rate(rate).map_err(|e| bad("샘플레이트", format!("{e:?}")))?;
        b.set_brate(bitrate_of(kbps)).map_err(|e| bad("음질", format!("{e:?}")))?;
        b.set_quality(Quality::Best).map_err(|e| bad("품질", format!("{e:?}")))?;
        let enc = b.build().map_err(|e| bad("시작", format!("{e:?}")))?;
        let f = std::fs::File::create(dst).map_err(|e| format!("{} 를 만들지 못했습니다: {e}", dst.display()))?;
        Ok(Self { enc, file: std::io::BufWriter::new(f), channels, out: Vec::new() })
    }
}

impl Sink for Mp3Sink {
    fn write(&mut self, pcm: &[i16]) -> Result<(), String> {
        let frames = pcm.len() / self.channels as usize;
        self.out.clear();
        self.out.reserve(mp3lame_encoder::max_required_buffer_size(frames));
        let n = if self.channels == 1 {
            self.enc.encode(MonoPcm(pcm), self.out.spare_capacity_mut())
        } else {
            self.enc.encode(InterleavedPcm(pcm), self.out.spare_capacity_mut())
        }
        .map_err(|e| format!("MP3 로 담지 못했습니다: {e:?}"))?;
        // SAFETY: encode 가 앞 n 바이트를 채웠다고 돌려줬다
        unsafe { self.out.set_len(n) };
        self.file.write_all(&self.out).map_err(|e| format!("MP3 를 쓰지 못했습니다: {e}"))
    }
    fn finish(mut self: Box<Self>) -> Result<(), String> {
        self.out.clear();
        self.out.reserve(7200);
        let n = self.enc.flush::<FlushNoGap>(self.out.spare_capacity_mut()).map_err(|e| format!("MP3 를 마무리하지 못했습니다: {e:?}"))?;
        // SAFETY: flush 가 앞 n 바이트를 채웠다고 돌려줬다
        unsafe { self.out.set_len(n) };
        self.file.write_all(&self.out).and_then(|_| self.file.flush()).map_err(|e| format!("MP3 를 쓰지 못했습니다: {e}"))
    }
}

/// 3채널 이상은 앞 두 채널만 쓴다 (5.1 을 스테레오로 섞는 건 이 기능의 일이 아니다 — 흔한 wav·mp3 는 1~2채널이다).
fn to_stereo(pcm: &[i16], channels: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(pcm.len() / channels * 2);
    for fr in pcm.chunks_exact(channels) {
        out.push(fr[0]);
        out.push(fr[1]);
    }
    out
}

/// 소리 한 개. 돌려주는 값: (파일 크기, 길이 초)
pub fn convert_audio(src: &Path, dst: &Path, target: &str, kbps: u32) -> Result<(u64, f64), String> {
    let r = convert_audio_inner(src, dst, target, kbps);
    if r.is_err() {
        let _ = std::fs::remove_file(dst); // 반쯤 쓴 파일을 남기지 않는다
    }
    r
}

fn convert_audio_inner(src: &Path, dst: &Path, target: &str, kbps: u32) -> Result<(u64, f64), String> {
    let file = std::fs::File::open(src).map_err(|e| format!("{} 를 열지 못했습니다: {e}", src.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(&ext_of(src));
    let fopts = FormatOptions { enable_gapless: true, ..Default::default() };
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fopts, &MetadataOptions::default())
        .map_err(|e| format!("{} 는 소리 파일로 읽을 수 없어요(파일이 깨졌거나 다른 형식이에요): {e}", src.display()))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "소리가 들어 있지 않아요.".to_string())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("이 소리 형식은 풀 수 없어요: {e}"))?;

    let mut sink: Option<Box<dyn Sink>> = None;
    let mut buf: Option<SampleBuffer<i16>> = None;
    let (mut rate, mut frames) = (0u32, 0u64);
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(format!("소리를 읽다가 멈췄어요: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) => continue, // 깨진 조각 하나는 건너뛴다
            Err(e) => return Err(format!("소리를 풀다가 멈췄어요: {e}")),
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count();
        if ch == 0 {
            continue;
        }
        if sink.is_none() {
            rate = spec.rate;
            let out_ch = ch.min(2) as u16;
            sink = Some(match target {
                "wav" => Box::new(WavSink::new(dst, rate, out_ch)?),
                "mp3" => Box::new(Mp3Sink::new(dst, rate, out_ch, kbps)?),
                other => return Err(format!("{other} 로는 바꿀 수 없어요.")),
            });
        }
        if buf.as_ref().map(|b| b.capacity() < decoded.capacity() * ch).unwrap_or(true) {
            buf = Some(SampleBuffer::<i16>::new(decoded.capacity() as u64, spec));
        }
        let b = buf.as_mut().unwrap();
        b.copy_interleaved_ref(decoded);
        let pcm = b.samples();
        frames += (pcm.len() / ch) as u64;
        let s = sink.as_mut().unwrap();
        if ch > 2 {
            s.write(&to_stereo(pcm, ch))?;
        } else {
            s.write(pcm)?;
        }
    }
    let sink = sink.ok_or_else(|| "소리가 들어 있지 않아요.".to_string())?;
    sink.finish()?;
    let bytes = std::fs::metadata(dst).map(|m| m.len()).map_err(|e| format!("저장한 파일을 확인하지 못했습니다: {e}"))?;
    Ok((bytes, if rate > 0 { frames as f64 / rate as f64 } else { 0.0 }))
}

// ── PDF ────────────────────────────────────────────────────────────────────

/// 한 장으로 이을 때 배율: 원하는 dpi 로 그리되, 다 이은 높이·픽셀 수가 상한을 넘으면 줄인다.
pub fn one_scale(dips: &[(f64, f64)], dpi: u32) -> f64 {
    let want = dpi as f64 / super::pdf_render::DIP_PER_INCH;
    let h: f64 = dips.iter().map(|d| d.1).sum();
    let w = dips.iter().map(|d| d.0).fold(1.0, f64::max);
    let by_h = ONE_MAX_HEIGHT as f64 / h.max(1.0);
    let by_px = (ONE_MAX_PIXELS as f64 / (w * h).max(1.0)).sqrt();
    want.min(by_h).min(by_px)
}

/// PDF 하나. `mode` "pages" 면 쪽마다 파일 하나(2쪽 이상이면 PDF 이름 폴더에), "one" 이면 세로로 이은 파일 하나.
pub fn convert_pdf(
    src: &Path, out_dir: &Path, target: &str, mode: &str, dpi: u32, quality: u8,
    on_page: &mut dyn FnMut(usize, usize),
) -> Result<Vec<OutputFile>, String> {
    let fmt = if target == "png" { OutFormat::Png } else { OutFormat::Jpg };
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("PDF").to_string();
    let source = src.to_string_lossy().to_string();
    let white = [255, 255, 255, 255];
    let mut outs = Vec::new();

    if mode == "one" {
        let want = dpi as f64 / super::pdf_render::DIP_PER_INCH;
        let used = std::sync::Mutex::new(want);
        let scale = |d: &[(f64, f64)]| {
            let k = one_scale(d, dpi);
            *used.lock().unwrap() = k;
            k
        };
        let mut canvas: Option<RgbaImage> = None;
        let mut tops: Vec<i64> = Vec::new();
        let n = super::pdf_render::render(src, &scale, &mut |m| {
            match m {
                super::pdf_render::Msg::Plan(plan) => {
                    let w = plan.iter().map(|p| p.0).max().unwrap_or(1);
                    let mut y = 0i64;
                    for p in &plan {
                        tops.push(y);
                        y += p.1 as i64;
                    }
                    canvas = Some(RgbaImage::from_pixel(w, y.max(1) as u32, image::Rgba(white)));
                    on_page(0, plan.len());
                }
                super::pdf_render::Msg::Page(i, img) => {
                    let c = canvas.as_mut().unwrap();
                    // 폭이 다른 쪽(가로 쪽)이 섞이면 가운데에 둔다
                    let x = (c.width() as i64 - img.width() as i64) / 2;
                    image::imageops::overlay(c, &img, x, tops[i]);
                    on_page(i + 1, tops.len());
                }
            }
            Ok(())
        })?;
        let c = canvas.ok_or("쪽이 하나도 없는 PDF 예요.")?;
        let dst = convert::unique_path(out_dir, &format!("{stem}.{target}"));
        let bytes = convert::save(&c, &dst, fmt, quality, white)?;
        let k = *used.lock().unwrap();
        let mut label = format!("pdf → {target} · {n}쪽을 한 장으로");
        if k < want * 0.99 {
            label.push_str(&format!(" · 너무 길어서 {}dpi 로 줄였어요", (k * super::pdf_render::DIP_PER_INCH).round() as u32));
        }
        outs.push(OutputFile { source, file: dst.to_string_lossy().to_string(), width: c.width(), height: c.height(), bytes, label });
        return Ok(outs);
    }

    let k = dpi as f64 / super::pdf_render::DIP_PER_INCH;
    let scale = move |_: &[(f64, f64)]| k;
    let mut dir = out_dir.to_path_buf();
    let mut total = 0usize;
    super::pdf_render::render(src, &scale, &mut |m| {
        match m {
            super::pdf_render::Msg::Plan(plan) => {
                total = plan.len();
                if total > 1 {
                    dir = convert::unique_path(out_dir, &stem);
                    std::fs::create_dir_all(&dir).map_err(|e| format!("폴더를 만들지 못했어요: {e}"))?;
                }
                on_page(0, total);
            }
            super::pdf_render::Msg::Page(i, img) => {
                let digits = total.to_string().len().max(2);
                let name = if total > 1 { format!("{stem}_{:0digits$}.{target}", i + 1) } else { format!("{stem}.{target}") };
                let dst = convert::unique_path(&dir, &name);
                let bytes = convert::save(&img, &dst, fmt, quality, white)?;
                outs.push(OutputFile { source: source.clone(), file: dst.to_string_lossy().to_string(), width: img.width(), height: img.height(), bytes, label: format!("pdf → {target} · {}/{total}쪽", i + 1) });
                on_page(i + 1, total);
            }
        }
        Ok(())
    })?;
    Ok(outs)
}

// ── 일괄 ───────────────────────────────────────────────────────────────────

pub fn run(req: &ExtRequest, on_progress: &mut dyn FnMut(usize, usize, &str)) -> Result<BatchResult, String> {
    if req.files.is_empty() {
        return Err("바꿀 파일을 골라 주세요.".into());
    }
    let img_to = req.image_format.as_deref().map(|s| if s == "jpeg" { "jpg" } else { s }).unwrap_or("png");
    let aud_to = req.audio_format.as_deref().unwrap_or("mp3");
    if !IMAGE_TARGETS.contains(&img_to) {
        return Err(format!("그림을 {img_to} 로는 바꿀 수 없어요."));
    }
    if !AUDIO_TARGETS.contains(&aud_to) {
        return Err(format!("소리를 {aud_to} 로는 바꿀 수 없어요."));
    }
    let pdf_to = req.pdf_format.as_deref().map(|s| if s == "jpeg" { "jpg" } else { s }).unwrap_or("jpg");
    if !PDF_TARGETS.contains(&pdf_to) {
        return Err(format!("PDF 를 {pdf_to} 로는 바꿀 수 없어요."));
    }
    let pdf_mode = if req.pdf_mode.as_deref() == Some("one") { "one" } else { "pages" };
    let pdf_dpi = req.pdf_dpi.unwrap_or(150).clamp(72, 600);
    let out_dir = std::path::PathBuf::from(&req.out_dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("저장 폴더를 만들지 못했습니다: {e}"))?;
    let bg = req.background.unwrap_or([255, 255, 255, 255]);
    let quality = req.quality.unwrap_or(92).clamp(1, 100);
    let kbps = req.bitrate.unwrap_or(192);
    let total = req.files.len();
    let mut out = BatchResult { total, ..Default::default() };

    for (i, f) in req.files.iter().enumerate() {
        on_progress(i, total, f);
        let src = Path::new(f);
        let from = ext_of(src);
        let audio = is_audio(src);
        let to = if audio { aud_to } else { img_to };
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("파일");
        if is_pdf(src) {
            let mut tick = |done: usize, pages: usize| on_progress(i, total, &format!("{f} · {done}/{pages}쪽"));
            match convert_pdf(src, &out_dir, pdf_to, pdf_mode, pdf_dpi, quality, &mut tick) {
                Ok(v) => out.outputs.extend(v),
                Err(e) => out.errors.push(BatchError { file: f.clone(), error: e }),
            }
            continue;
        }
        let r: Result<OutputFile, String> = (|| {
            if !audio && !convert::is_readable(src) {
                return Err(format!(".{from} 파일은 아직 바꿀 수 없어요 (그림 png·jpg·webp·bmp·gif, 소리 wav·mp3, PDF)."));
            }
            if from == to {
                return Err(format!("이미 {} 파일이에요 — 바꿀 게 없어 건너뛰었어요.", to.to_uppercase()));
            }
            let dst = convert::unique_path(&out_dir, &format!("{stem}.{to}"));
            let label = format!("{from} → {to}");
            if audio {
                let (bytes, _secs) = convert_audio(src, &dst, to, kbps)?;
                Ok(OutputFile { source: f.clone(), file: dst.to_string_lossy().to_string(), width: 0, height: 0, bytes, label })
            } else {
                let (bytes, w, h) = convert_image(src, &dst, to, quality, bg)?;
                Ok(OutputFile { source: f.clone(), file: dst.to_string_lossy().to_string(), width: w, height: h, bytes, label })
            }
        })();
        match r {
            Ok(o) => out.outputs.push(o),
            Err(e) => out.errors.push(BatchError { file: f.clone(), error: e }),
        }
    }
    on_progress(total, total, "");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 손실 압축 + 투명이 들어 있는 진짜 webp (ffmpeg libwebp 로 만든 64×64 표본).
    const LOSSY_ALPHA_WEBP: &[u8] = include_bytes!("testdata/lossy_alpha.webp");

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kom-ext-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn req(files: Vec<String>, out: &Path, img: &str, aud: &str) -> ExtRequest {
        ExtRequest { files, out_dir: out.to_string_lossy().into(), image_format: Some(img.into()), audio_format: Some(aud.into()), quality: None, bitrate: None, background: None, pdf_format: None, pdf_mode: None, pdf_dpi: None }
    }

    /// CEO "webp 에서 png 로 바꿀 경우에도 잘 나와야 해" — 손실 webp 를 png 로 바꾼 뒤 다시 열어
    /// **픽셀과 투명이 한 칸도 다르지 않은지** 본다 (png 는 무손실이라 달라질 이유가 없다).
    #[test]
    fn lossy_webp_with_transparency_becomes_an_identical_png() {
        let d = dir("webp");
        let src = d.join("그림.webp");
        std::fs::write(&src, LOSSY_ALPHA_WEBP).unwrap();
        let before = convert::load(&src).expect("손실 webp 를 읽어야 한다");
        assert!(before.pixels().any(|p| p[3] == 0), "표본에 완전히 투명한 곳이 있어야 한다");
        assert!(before.pixels().any(|p| p[3] == 255), "표본에 불투명한 곳도 있어야 한다");

        let r = run(&req(vec![src.to_string_lossy().into()], &d.join("out"), "png", "mp3"), &mut |_, _, _| {}).unwrap();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let png = Path::new(&r.outputs[0].file);
        assert_eq!(png.extension().unwrap(), "png");
        assert_eq!(&std::fs::read(png).unwrap()[..8], b"\x89PNG\r\n\x1a\n", "이름만이 아니라 속이 PNG 여야 한다");
        let after = convert::load(png).unwrap();
        assert_eq!(after.dimensions(), before.dimensions());
        assert_eq!(after.as_raw(), before.as_raw(), "색·투명이 webp 와 한 칸도 달라지면 안 된다");
    }

    #[test]
    fn every_image_format_converts_to_every_other() {
        let d = dir("matrix");
        let base = image::RgbaImage::from_fn(40, 30, |x, y| image::Rgba([(x * 6) as u8, (y * 8) as u8, 90, 255]));
        for from in IMAGE_TARGETS {
            let src = d.join(format!("원본_{from}.{from}"));
            convert_image_from_pixels(&base, &src, from);
            for to in IMAGE_TARGETS.iter().filter(|t| *t != from) {
                let r = run(&req(vec![src.to_string_lossy().into()], &d.join(format!("{from}_{to}")), to, "mp3"), &mut |_, _, _| {}).unwrap();
                assert!(r.errors.is_empty(), "{from} → {to}: {:?}", r.errors);
                let o = &r.outputs[0];
                let back = convert::load(Path::new(&o.file)).unwrap_or_else(|e| panic!("{from} → {to} 결과를 다시 못 열었다: {e}"));
                assert_eq!(back.dimensions(), (40, 30), "{from} → {to}");
                assert!(image::ImageFormat::from_path(&o.file).is_ok());
                let guessed = image::ImageReader::open(&o.file).unwrap().with_guessed_format().unwrap().format().unwrap();
                assert_eq!(guessed.extensions_str()[0].replace("jpeg", "jpg"), **to, "{from} → {to}: 속 형식이 확장자와 같아야 한다");
            }
        }
    }

    fn convert_image_from_pixels(img: &image::RgbaImage, dst: &Path, fmt: &str) {
        let tmp = dst.with_extension("src.png");
        convert::save(img, &tmp, OutFormat::Png, 90, [255; 4]).unwrap();
        if fmt == "png" {
            std::fs::rename(&tmp, dst).unwrap();
        } else {
            convert_image(&tmp, dst, fmt, 92, [255; 4]).unwrap();
            let _ = std::fs::remove_file(&tmp);
        }
    }

    #[test]
    fn transparent_png_to_jpg_gets_the_background_colour() {
        let d = dir("alpha");
        let src = d.join("투명.png");
        convert::save(&image::RgbaImage::from_pixel(20, 20, image::Rgba([0, 0, 0, 0])), &src, OutFormat::Png, 90, [255; 4]).unwrap();
        let dst = d.join("투명.jpg");
        convert_image(&src, &dst, "jpg", 92, [255, 255, 255, 255]).unwrap();
        let p = convert::load(&dst).unwrap();
        let px = p.get_pixel(10, 10);
        assert!(px[0] > 250 && px[1] > 250 && px[2] > 250, "투명한 곳이 검게 나오면 안 된다: {px:?}");
    }

    /// 440Hz 1초 · 스테레오 16비트 wav 를 손으로 만든다.
    fn tone_wav(path: &Path, rate: u32, channels: u16) {
        let mut s = Box::new(WavSink::new(path, rate, channels).unwrap());
        let mut pcm = Vec::new();
        for n in 0..rate {
            let v = ((n as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 12000.0) as i16;
            for _ in 0..channels {
                pcm.push(v);
            }
        }
        s.write(&pcm).unwrap();
        s.finish().unwrap();
    }

    fn rms_of_wav(path: &Path) -> (f64, u32, u16, f64) {
        let b = std::fs::read(path).unwrap();
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        let channels = u16::from_le_bytes([b[22], b[23]]);
        let rate = u32::from_le_bytes([b[24], b[25], b[26], b[27]]);
        let data = u32::from_le_bytes([b[40], b[41], b[42], b[43]]) as usize;
        assert_eq!(data, b.len() - 44, "WAV 머리말의 크기 칸이 실제 크기와 같아야 한다");
        let n = data / 2;
        let sum: f64 = (0..n).map(|i| i16::from_le_bytes([b[44 + i * 2], b[45 + i * 2]]) as f64).map(|v| v * v).sum();
        ((sum / n as f64).sqrt(), rate, channels, n as f64 / channels as f64 / rate as f64)
    }

    #[test]
    fn wav_becomes_a_real_mp3_and_comes_back_as_wav_with_the_same_sound() {
        for (rate, ch) in [(44_100u32, 2u16), (48_000, 1)] {
            let d = dir(&format!("audio{rate}{ch}"));
            let wav = d.join("소리.wav");
            tone_wav(&wav, rate, ch);
            let (rms0, _, _, _) = rms_of_wav(&wav);

            let r = run(&req(vec![wav.to_string_lossy().into()], &d.join("mp3"), "png", "mp3"), &mut |_, _, _| {}).unwrap();
            assert!(r.errors.is_empty(), "{:?}", r.errors);
            let mp3 = std::path::PathBuf::from(&r.outputs[0].file);
            let head = std::fs::read(&mp3).unwrap();
            assert!(head.len() > 10_000 && head.len() < 60_000, "192kbps 1초면 약 24KB: {}", head.len());
            assert!(head[0] == 0xFF && head[1] & 0xE0 == 0xE0 || &head[..3] == b"ID3", "MP3 프레임으로 시작해야 한다");

            let r2 = run(&req(vec![mp3.to_string_lossy().into()], &d.join("wav"), "png", "wav"), &mut |_, _, _| {}).unwrap();
            assert!(r2.errors.is_empty(), "{:?}", r2.errors);
            let (rms1, rate1, ch1, secs) = rms_of_wav(Path::new(&r2.outputs[0].file));
            assert_eq!((rate1, ch1), (rate, ch), "샘플레이트·채널이 그대로여야 한다");
            assert!((secs - 1.0).abs() < 0.08, "길이가 1초 근처여야 한다: {secs}");
            assert!((rms1 - rms0).abs() / rms0 < 0.1, "소리 크기가 원본과 비슷해야 한다: {rms0} → {rms1}");
        }
    }

    #[test]
    fn same_format_and_unknown_files_are_reported_not_silently_dropped() {
        let d = dir("skip");
        let png = d.join("a.png");
        convert::save(&image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])), &png, OutFormat::Png, 90, [255; 4]).unwrap();
        let txt = d.join("메모.txt");
        std::fs::write(&txt, "x").unwrap();
        let fake = d.join("가짜.wav");
        std::fs::write(&fake, "이건 소리가 아니다").unwrap();
        let r = run(&req(vec![png.to_string_lossy().into(), txt.to_string_lossy().into(), fake.to_string_lossy().into()], &d.join("out"), "png", "mp3"), &mut |_, _, _| {}).unwrap();
        assert!(r.outputs.is_empty());
        assert_eq!(r.errors.len(), 3);
        assert!(r.errors[0].error.contains("이미 PNG"));
        assert!(r.errors[1].error.contains("아직 바꿀 수 없어요"));
        assert!(!d.join("out").join("가짜.mp3").exists(), "실패한 파일을 반쯤 남기면 안 된다");
        assert!(png.exists() && txt.exists() && fake.exists(), "원본은 그대로");
    }

    /// 손으로 짠 PDF. 쪽마다 (가로pt, 세로pt, 빨강 0~1, 초록, 파랑) — 왼쪽 아래 (0,0) 에서 100pt 네모를 칠한다.
    pub(crate) fn tiny_pdf(pages: &[(u32, u32, f32, f32, f32)]) -> Vec<u8> {
        let mut objs: Vec<String> = Vec::new();
        let n = pages.len();
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + i * 2)).collect();
        objs.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        objs.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
        for (i, &(w, h, r, g, b)) in pages.iter().enumerate() {
            objs.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents {} 0 R >>", 4 + i * 2));
            let body = format!("{r} {g} {b} rg 0 0 100 100 re f");
            objs.push(format!("<< /Length {} >>\nstream\n{body}\nendstream", body.len()));
        }
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offs {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    fn pdf_req(files: Vec<String>, out: &Path, to: &str, mode: &str) -> ExtRequest {
        ExtRequest { pdf_format: Some(to.into()), pdf_mode: Some(mode.into()), pdf_dpi: Some(150), ..req(files, out, "png", "mp3") }
    }

    const RED: (f32, f32, f32) = (1.0, 0.0, 0.0);

    /// CEO "pdf 를 jpg 로 — 각 장으로 저장". 3쪽 → PDF 이름 폴더에 _01 · _02 · _03.jpg, 150dpi 크기, 네모가 제자리.
    #[test]
    fn pdf_pages_become_one_jpg_each_in_a_folder() {
        let d = dir("pdfpages");
        let src = d.join("견적서.pdf");
        std::fs::write(&src, tiny_pdf(&[(612, 792, RED.0, RED.1, RED.2), (612, 792, 0.0, 0.0, 1.0), (792, 612, 0.0, 0.6, 0.0)])).unwrap();
        let mut seen = Vec::new();
        let r = run(&pdf_req(vec![src.to_string_lossy().into()], &d.join("out"), "jpg", "pages"), &mut |_, _, c| seen.push(c.to_string())).unwrap();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.outputs.len(), 3);
        assert!(seen.iter().any(|c| c.ends_with("· 2/3쪽")), "쪽마다 진행이 보여야 한다: {seen:?}");
        let names: Vec<String> = r.outputs.iter().map(|o| Path::new(&o.file).file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["견적서_01.jpg", "견적서_02.jpg", "견적서_03.jpg"]);
        assert!(r.outputs.iter().all(|o| Path::new(&o.file).parent().unwrap().ends_with("견적서")), "PDF 이름 폴더에 모여야 한다");
        // 612pt = 8.5인치 → 150dpi 면 1275px
        let p1 = convert::load(Path::new(&r.outputs[0].file)).unwrap();
        assert!((p1.width() as i32 - 1275).abs() <= 1 && (p1.height() as i32 - 1650).abs() <= 1, "{:?}", p1.dimensions());
        assert_eq!(&std::fs::read(&r.outputs[0].file).unwrap()[..2], b"\xFF\xD8", "속이 JPG 여야 한다");
        // 왼쪽 아래 100pt(≈208px) 네모가 빨강, 오른쪽 위는 흰 바탕
        let px = p1.get_pixel(50, p1.height() - 50);
        assert!(px[0] > 220 && px[1] < 40 && px[2] < 40, "왼쪽 아래가 빨강이어야 한다: {px:?}");
        let wpx = p1.get_pixel(p1.width() - 50, 50);
        assert!(wpx[0] > 245 && wpx[1] > 245 && wpx[2] > 245, "바탕은 흰색: {wpx:?}");
        let p3 = convert::load(Path::new(&r.outputs[2].file)).unwrap();
        assert!(p3.width() > p3.height(), "가로 쪽은 가로로 나와야 한다: {:?}", p3.dimensions());
        assert!(src.exists(), "원본은 그대로");
    }

    /// CEO "pdf 전체를 한 장으로 저장". 쪽 높이를 다 더한 한 장, 가로 쪽은 가운데.
    #[test]
    fn whole_pdf_becomes_one_tall_image() {
        let d = dir("pdfone");
        let src = d.join("상세.pdf");
        std::fs::write(&src, tiny_pdf(&[(612, 792, RED.0, RED.1, RED.2), (612, 792, 0.0, 0.0, 1.0), (792, 612, 0.0, 0.6, 0.0)])).unwrap();
        let r = run(&pdf_req(vec![src.to_string_lossy().into()], &d.join("out"), "png", "one"), &mut |_, _, _| {}).unwrap();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert_eq!(r.outputs.len(), 1);
        let o = &r.outputs[0];
        assert!(o.file.ends_with("상세.png"), "{}", o.file);
        assert!(o.label.contains("3쪽을 한 장으로") && !o.label.contains("줄였"), "{}", o.label);
        let img = convert::load(Path::new(&o.file)).unwrap();
        // 폭 = 가장 넓은 쪽(792pt → 1650px), 높이 = 1650 + 1650 + 1275
        assert!((img.width() as i32 - 1650).abs() <= 1, "{:?}", img.dimensions());
        assert!((img.height() as i32 - 4575).abs() <= 3, "{:?}", img.dimensions());
        // 1쪽(세로 쪽, 폭 1275)은 가운데 — 왼쪽 여백 (1650-1275)/2 ≈ 187px 은 흰색, 그 오른쪽 아래가 빨강
        let gap = img.get_pixel(100, 1650 - 50);
        assert!(gap[0] > 245 && gap[1] > 245 && gap[2] > 245, "옆 여백은 흰색: {gap:?}");
        let red = img.get_pixel(187 + 50, 1650 - 50);
        assert!(red[0] > 220 && red[1] < 40, "1쪽 왼쪽 아래 빨강: {red:?}");
        let blue = img.get_pixel(187 + 50, 3300 - 50);
        assert!(blue[2] > 220 && blue[0] < 40, "2쪽 왼쪽 아래 파랑: {blue:?}");
        let green = img.get_pixel(50, img.height() - 50);
        assert!(green[1] > 120 && green[0] < 40, "3쪽(가로) 왼쪽 아래 초록: {green:?}");
    }

    #[test]
    fn a_one_page_pdf_is_saved_without_a_folder_and_bad_pdfs_are_reported() {
        let d = dir("pdfone1");
        let one = d.join("한장.pdf");
        std::fs::write(&one, tiny_pdf(&[(612, 792, RED.0, RED.1, RED.2)])).unwrap();
        let bad = d.join("깨짐.pdf");
        std::fs::write(&bad, b"%PDF-1.4 not really").unwrap();
        let r = run(&pdf_req(vec![one.to_string_lossy().into(), bad.to_string_lossy().into()], &d.join("out"), "jpg", "pages"), &mut |_, _, _| {}).unwrap();
        assert_eq!(r.outputs.len(), 1, "{:?}", r.errors);
        assert_eq!(Path::new(&r.outputs[0].file), d.join("out").join("한장.jpg"));
        assert_eq!(r.errors.len(), 1);
        assert!(r.errors[0].error.contains("PDF"), "{}", r.errors[0].error);
    }

    #[test]
    fn a_very_long_pdf_is_shrunk_to_fit_one_image() {
        // A4(DIP 794×1123) 200쪽 — 150dpi 면 높이 35만px. 6만px 안으로 줄어야 한다
        let dips = vec![(794.0, 1123.0); 200];
        let k = one_scale(&dips, 150);
        let h: f64 = dips.iter().map(|d| d.1 * k).sum();
        assert!(h <= ONE_MAX_HEIGHT as f64 + 1.0 && h > 50_000.0, "{h}");
        assert!(k < 150.0 / 96.0);
        // 짧은 PDF 는 그대로 150dpi
        assert_eq!(one_scale(&dips[..3], 150), 150.0 / 96.0);
    }
}

#[cfg(test)]
mod real_files {
    use super::*;

    /// 손으로 돌리는 확인 — `KOM_REAL_DIR` 폴더의 진짜 파일(photo.webp · voice.wav · music.mp3)을 바꿔 본다.
    #[test]
    #[ignore = "진짜 파일이 든 폴더를 KOM_REAL_DIR 로 줘야 한다"]
    fn real_files_convert() {
        let d = std::path::PathBuf::from(std::env::var("KOM_REAL_DIR").expect("KOM_REAL_DIR"));
        let files = ["photo.webp", "voice.wav", "music.mp3"].iter().map(|n| d.join(n).to_string_lossy().to_string()).collect::<Vec<_>>();
        for (img, aud) in [("png", "mp3"), ("jpg", "wav")] {
            let req = ExtRequest { files: files.clone(), out_dir: d.join(format!("out_{img}_{aud}")).to_string_lossy().into(), image_format: Some(img.into()), audio_format: Some(aud.into()), quality: None, bitrate: Some(192), background: None, pdf_format: None, pdf_mode: None, pdf_dpi: None };
            let r = run(&req, &mut |_, _, _| {}).unwrap();
            for o in &r.outputs {
                println!("[실물] {} · {} · {}바이트 · {}x{}", o.label, o.file, o.bytes, o.width, o.height);
            }
            for e in &r.errors {
                println!("[실물] 건너뜀 {} — {}", e.file, e.error);
            }
            assert_eq!(r.outputs.len(), 2, "그림 1 + 소리 1 이 바뀌어야 한다");
        }
    }
    /// 손으로 돌리는 확인 — `KOM_REAL_PDF` 의 진짜 PDF 를 쪽마다 · 한 장으로 바꿔 `KOM_REAL_OUT` 에 둔다.
    #[test]
    #[ignore = "진짜 PDF 를 KOM_REAL_PDF 로 줘야 한다"]
    fn real_pdf_converts() {
        let pdf = std::env::var("KOM_REAL_PDF").expect("KOM_REAL_PDF");
        let out = std::path::PathBuf::from(std::env::var("KOM_REAL_OUT").expect("KOM_REAL_OUT"));
        for mode in ["pages", "one"] {
            let t = std::time::Instant::now();
            let req = ExtRequest { files: vec![pdf.clone()], out_dir: out.join(mode).to_string_lossy().into(), image_format: None, audio_format: None, quality: None, bitrate: None, background: None, pdf_format: Some("jpg".into()), pdf_mode: Some(mode.into()), pdf_dpi: Some(150) };
            let r = run(&req, &mut |_, _, _| {}).unwrap();
            assert!(r.errors.is_empty(), "{:?}", r.errors);
            let bytes: u64 = r.outputs.iter().map(|o| o.bytes).sum();
            println!("[실물] {mode}: {}개 · {}KB · {:?} · 첫 장 {}x{} · {}", r.outputs.len(), bytes / 1024, t.elapsed(), r.outputs[0].width, r.outputs[0].height, r.outputs[0].label);
        }
    }
}
