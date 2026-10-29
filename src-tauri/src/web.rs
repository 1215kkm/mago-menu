//! **인터넷에서 끌어 받기** — 10-28 CEO "유튜브 영상위에서 영상을 시프트 드래그 하면 다운받는 메뉴도 추가하자
//! 인터넷에서 뭔가를 잡고 시프트 드래그 하면 다운받게 이미지나 영상, 링크파일등등 다운받거나, 카톡으로 보내거나".
//!
//! 브라우저(크롬 · 엣지 · 웨일 · 파이어폭스)에서 그림 · 링크 · 주소창 주소를 Shift 를 누른 채 끌면 같은 고리가 뜬다.
//! 끌기 데이터에서 주소(UniformResourceLocatorW · text/x-moz-url · 주소처럼 생긴 글)와 브라우저가 같이 넘겨 주는
//! 그림 바이트(FileGroupDescriptorW + FileContents)를 읽는 것은 `ring.rs drop_win` 이다. 여기는 그 뒤의 일.
//! - 그림: 저장(받은 그대로) · JPG · PNG · 포토샵 · 피그마 · Word · 다른 프로그램으로 · 카톡
//! - 파일 링크(.pdf · .zip · .mp4 …): 받기 · 바로가기 · 다른 프로그램으로 · 카톡(파일)
//! 포토샵 · 피그마 · Word 칸은 이 PC 에 깔려 있을 때만 (10-29 CEO "피그마나 포토샵으로 바로 열리도록 … 워드같은 다른프로그램에도").
//! 포토샵은 받은 파일을 바로 연다. 피그마 · Word 는 파일을 넘겨 열 길이 없어 그림을 클립보드에 담고 그 프로그램을 연다 — 그 안에서 Ctrl+V.
//! - 그 밖의 링크 · 유튜브: 바로가기(.url) · 카톡(링크)
//!
//! 받은 것은 **다운로드 폴더**(설정에서 바꿀 수 있다 — Settings.web_dir)에, 같은 이름이 있으면 `(2)`.
//! 저장한 뒤에는 그 폴더를 열어 파일을 골라 둔다 (Settings.web_open, 기본 켬 — 10-29 CEO "저장한후에 폴더가 바로 열리는걸 기본으로"). 인터넷에서 온 표시(Zone.Identifier)를 브라우저처럼 붙인다 — 받은 exe 를 열면 윈도우가 경고한다.
//! 카톡: 카카오톡 PC 에는 다른 프로그램이 대화방에 보내는 길이 없다 → 보낼 파일(또는 링크 글)을 **클립보드에 담고 카카오톡을 연다**. 대화방에서 Ctrl+V.
//! MAGO 가 직접 PC 밖으로 보내는 것은 없다 (보내는 것은 사람이 카톡에서).
//! 유튜브 영상 받기는 아직 — 유튜브 약관이 받기를 막고 있어 대표님 결정 대기. 유튜브 주소는 바로가기 · 카톡만.

use std::path::{Path, PathBuf};

/// 브라우저에서 끌어 온 것
#[derive(Clone, Default)]
pub struct Web {
    pub url: String,
    pub title: String,
    /// 브라우저가 붙여 준 파일 이름 (그림을 끌 때 "cat.png") — 없으면 빈 글자
    pub name: String,
    /// image | file | link | youtube
    pub kind: &'static str,
    /// 브라우저가 넘겨 준 내용 (그림) — 놓을 때만 읽는다
    pub bytes: Option<Vec<u8>>,
    /// 브라우저가 임시 폴더에 써 둔 파일 (파이어폭스 그림)
    pub local: Option<PathBuf>,
}

/// 그림으로 치는 확장자 (JPG · PNG 로 바꿀 수 있는 것은 convert::READABLE_EXT)
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "jfif", "png", "gif", "webp", "bmp", "avif", "svg", "ico"];
/// 링크 주소가 이것으로 끝나면 '파일 링크' — 받기 칸을 보인다
const FILE_EXT: &[&str] = &[
    "pdf", "zip", "7z", "rar", "alz", "egg", "tar", "gz", "mp4", "mov", "webm", "mkv", "avi", "wmv", "mp3", "wav", "m4a", "flac", "ogg", "aac", "hwp", "hwpx", "doc", "docx",
    "xls", "xlsx", "ppt", "pptx", "txt", "csv", "rtf", "psd", "ai", "epub", "exe", "msi", "apk", "iso", "torrent", "ttf", "otf",
];

fn ext_of(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => e.to_ascii_lowercase(),
        _ => String::new(),
    }
}

/// 주소의 호스트 ("www.youtube.com") — 사용자 · 포트는 뺀다
pub fn host(url: &str) -> String {
    let rest = url.split_once("://").map(|x| x.1).unwrap_or("");
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    let auth = auth.rsplit('@').next().unwrap_or(auth);
    auth.split(':').next().unwrap_or("").to_ascii_lowercase()
}

/// 주소 경로의 마지막 조각 (%xx 를 풀어서) — "https://a.com/x/%EC%82%AC%EC%A7%84.png?w=1" → "사진.png"
fn last_segment(url: &str) -> String {
    let rest = url.split_once("://").map(|x| x.1).unwrap_or(url);
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let path = path.split_once('/').map(|x| x.1).unwrap_or("");
    percent_decode(path.rsplit('/').next().unwrap_or(""))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn is_youtube(url: &str) -> bool {
    let h = host(url);
    h == "youtu.be" || h == "youtube.com" || h.ends_with(".youtube.com")
}

/// 끌기 데이터에서 읽은 것 → 무엇인지. 쓸 만한 주소도 그림도 없으면 None
/// (`file:` · `javascript:` 같은 주소는 받지 않는다 — http · https · 그림 data: 만)
pub fn classify(url: &str, title: &str, name: &str, local: Option<PathBuf>) -> Option<Web> {
    let url = url.trim().to_string();
    let web_url = url.starts_with("http://") || url.starts_with("https://");
    let data_img = url.starts_with("data:image/");
    // 크롬은 링크를 끌 때 바탕화면용 바로가기("제목.url")를 파일처럼 같이 준다 — 이름은 제목으로만 쓴다
    let (name, title) = if ext_of(name) == "url" { (String::new(), if title.is_empty() { name[..name.len() - 4].to_string() } else { title.to_string() }) } else { (name.to_string(), title.to_string()) };
    let local_ext = local.as_ref().map(|p| ext_of(&p.to_string_lossy())).unwrap_or_default();
    if !web_url && !data_img && local.is_none() && name.is_empty() {
        return None;
    }
    let url_ext = if web_url { ext_of(&last_segment(&url)) } else { String::new() };
    let kind = if IMAGE_EXT.contains(&ext_of(&name).as_str()) || IMAGE_EXT.contains(&local_ext.as_str()) || data_img {
        "image"
    } else if !name.is_empty() || local.is_some() {
        // 브라우저가 내용을 같이 준 그림 아닌 파일 (지메일 첨부를 끌 때 등)
        "file"
    } else if is_youtube(&url) {
        "youtube"
    } else if IMAGE_EXT.contains(&url_ext.as_str()) {
        "image"
    } else if FILE_EXT.contains(&url_ext.as_str()) {
        "file"
    } else {
        "link"
    };
    Some(Web { url: if web_url || data_img { url } else { String::new() }, title, name, kind, bytes: None, local })
}

/// 설정에서 켜고 끌 수 있는 칸 (ui/settings.js WEB 과 같게). 받기 · 바로가기는 늘 보인다
pub const OPTS: &[&str] = &["web-save", "web-jpg", "web-png", "web-ps", "web-figma", "web-word", "web-with", "web-kakao"];

/// 이 PC 에서 쓸 수 있는 칸인가 — 포토샵 · 피그마 · Word 는 깔려 있을 때만
pub fn available(id: &str) -> bool {
    match id {
        #[cfg(windows)]
        "web-ps" => apps::photoshop().is_some(),
        #[cfg(windows)]
        "web-figma" => apps::figma().is_some(),
        #[cfg(windows)]
        "web-word" => apps::word().is_some(),
        #[cfg(not(windows))]
        "web-ps" | "web-figma" | "web-word" => false,
        _ => true,
    }
}

/// 고리 칸 (이름 · 설명은 ring::seg_of). 깔린 프로그램 · 설정으로 거르는 것은 ring::web_plan
pub fn ids(w: &Web) -> Vec<&'static str> {
    match w.kind {
        "image" => {
            let e = ext_of(&shown_name(w));
            // JPG · PNG 로 바꾸는 칸은 읽을 수 있는 그림일 때만, 이미 그 형식이면 뺀다 (avif · svg 는 받은 그대로만)
            let readable = e.is_empty() || crate::convert::convert::READABLE_EXT.contains(&e.as_str()) || e == "jfif";
            let mut v = vec!["web-save"];
            if readable && !matches!(e.as_str(), "jpg" | "jpeg" | "jfif") {
                v.push("web-jpg");
            }
            if readable && e != "png" {
                v.push("web-png");
            }
            v.push("web-ps");
            // 피그마 · Word 는 그림을 클립보드에 그려 넣어야 한다 — 읽을 수 있는 그림만
            if readable {
                v.extend(["web-figma", "web-word"]);
            }
            v.extend(["web-with", "web-kakao"]);
            v
        }
        "file" => vec!["web-get", "web-url", "web-with", "web-kakao"],
        _ => vec!["web-url", "web-kakao"],
    }
}

/// 고리 가운데 큰 글
pub fn title(w: &Web) -> String {
    match w.kind {
        "image" => "그림".into(),
        "youtube" => "유튜브".into(),
        "file" => {
            let e = ext_of(&shown_name(w));
            if e.is_empty() { "파일".into() } else { format!("{} 파일", e.to_uppercase()) }
        }
        _ => {
            let h = host(&w.url);
            h.strip_prefix("www.").unwrap_or(&h).to_string()
        }
    }
}

/// 고리 가운데 작은 글
pub fn note(w: &Web) -> String {
    if w.kind == "youtube" { "영상 받기는 아직이에요".into() } else { "가운데에 놓으면 취소".into() }
}

/// 받을 때 붙일 이름 — 브라우저가 준 이름 › 임시 파일 이름 › 주소 끝 조각
fn shown_name(w: &Web) -> String {
    if !w.name.is_empty() {
        return w.name.clone();
    }
    if let Some(p) = &w.local {
        if let Some(f) = p.file_name() {
            return f.to_string_lossy().into_owned();
        }
    }
    if w.url.starts_with("data:image/") {
        let mime = w.url[11..].split([';', ',']).next().unwrap_or("png");
        return format!("그림.{}", if mime == "jpeg" { "jpg" } else if mime == "svg+xml" { "svg" } else { mime });
    }
    last_segment(&w.url)
}

/// 윈도우 파일 이름으로 쓸 수 있게 — 금지 글자는 `_`, 앞뒤 점·빈칸은 빼고, 너무 길면 줄인다 (확장자는 남긴다)
pub fn safe_name(name: &str, fallback: &str) -> String {
    let mut s: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).collect();
    s = s.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    if s.is_empty() {
        s = fallback.to_string();
    }
    let (stem, ext) = match s.rsplit_once('.') {
        Some((a, b)) if !a.is_empty() && b.len() <= 8 && !b.contains(' ') => (a.to_string(), format!(".{b}")),
        _ => (s.clone(), String::new()),
    };
    let stem: String = if stem.chars().count() > 100 { stem.chars().take(100).collect::<String>().trim_end().to_string() } else { stem };
    let upper = stem.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str()) || ((upper.starts_with("COM") || upper.starts_with("LPT")) && upper.len() == 4 && upper.as_bytes()[3].is_ascii_digit());
    format!("{}{stem}{ext}", if reserved { "_" } else { "" })
}

/// `Content-Disposition` 의 파일 이름 — filename*=UTF-8''… 을 먼저, 없으면 filename="…"
fn disposition_name(h: &str) -> Option<String> {
    let mut plain = None;
    for part in h.split(';').map(str::trim) {
        let Some((k, v)) = part.split_once('=') else { continue };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        if k == "filename*" {
            let v = v.split_once("''").map(|x| x.1).unwrap_or(v);
            let n = percent_decode(v.trim_matches('"'));
            if !n.is_empty() {
                return Some(n);
            }
        } else if k == "filename" {
            let n = v.trim_matches('"').to_string();
            if !n.is_empty() {
                plain = Some(n);
            }
        }
    }
    plain
}

/// 이름에 확장자가 없을 때 받은 형식으로 붙일 것
fn ext_for_type(ct: &str) -> &'static str {
    match ct.split(';').next().unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "text/plain" => "txt",
        _ => "",
    }
}

fn with_ext(name: String, ct: &str) -> String {
    let e = ext_for_type(ct);
    if ext_of(&name).is_empty() && !e.is_empty() { format!("{name}.{e}") } else { name }
}

/// data:image/png;base64,… → 바이트
fn data_url(url: &str) -> Option<Vec<u8>> {
    let (head, body) = url.split_once(',')?;
    if head.ends_with(";base64") {
        base64(body)
    } else {
        Some(percent_decode(body).into_bytes())
    }
}

fn base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// 인터넷에서 온 파일 표시 — 브라우저가 받은 파일과 같게 (받은 exe · 문서를 열면 윈도우가 한 번 묻는다)
fn mark_from_web(p: &Path, url: &str) {
    let host_url = if url.starts_with("http") { format!("HostUrl={url}\r\n") } else { String::new() };
    let _ = std::fs::write(format!("{}:Zone.Identifier", p.display()), format!("[ZoneTransfer]\r\nZoneId=3\r\n{host_url}"));
}

/// 받은 것을 `dir` 에 원래 모양 그대로 둔다 — 브라우저가 준 내용 › 브라우저 임시 파일 › data: › 인터넷에서 받기
fn fetch(w: &Web, dir: &Path, progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("폴더를 만들지 못했어요: {e}"))?;
    let named = |fallback: &str| crate::convert::convert::unique_path(dir, &safe_name(&shown_name(w), fallback));
    let dst = if let Some(b) = &w.bytes {
        let p = named("받은 그림");
        std::fs::write(&p, b).map_err(|e| format!("저장하지 못했어요: {e}"))?;
        p
    } else if let Some(src) = &w.local {
        let p = named("받은 파일");
        std::fs::copy(src, &p).map_err(|e| format!("저장하지 못했어요: {e}"))?;
        p
    } else if w.url.starts_with("data:") {
        let b = data_url(&w.url).ok_or("그림 주소를 읽지 못했어요")?;
        let p = named("받은 그림");
        std::fs::write(&p, b).map_err(|e| format!("저장하지 못했어요: {e}"))?;
        p
    } else {
        #[cfg(windows)]
        {
            http::get(&w.url, dir, &shown_name(w), progress)?
        }
        #[cfg(not(windows))]
        return Err("윈도우에서만 돼요".into());
    };
    mark_from_web(&dst, &w.url);
    Ok(dst)
}

/// 바로가기(.url) — 누르면 그 주소가 열린다
fn shortcut(w: &Web, dir: &Path) -> Result<PathBuf, String> {
    let base = if !w.title.trim().is_empty() { w.title.clone() } else { title(w) };
    let p = crate::convert::convert::unique_path(dir, &format!("{}.url", safe_name(&base, "링크").trim_end_matches(".url")));
    std::fs::write(&p, format!("[InternetShortcut]\r\nURL={}\r\n", w.url)).map_err(|e| format!("바로가기를 만들지 못했어요: {e}"))?;
    Ok(p)
}

/// 칸 하나를 한 결과
pub struct Did {
    /// 고리 가운데 — "저장함" · "카톡에 담음"
    pub head: String,
    /// 그 아래 작은 글
    pub note: String,
    /// 캐릭터 말풍선 (굵게 · 작게)
    pub say: (String, String),
    /// 저장한 파일 — 폴더 열기에 쓴다 (카톡은 None: 카카오톡을 연다)
    pub file: Option<PathBuf>,
}

/// 말풍선에 쓰는 폴더 이름 — 다운로드 폴더면 "다운로드 폴더", 아니면 그 폴더 이름
pub fn place(dir: &Path) -> String {
    if crate::outdir::downloads_dir().is_some_and(|d| same_dir(&d, dir)) {
        return "다운로드 폴더".into();
    }
    format!("{} 폴더", dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| dir.display().to_string()))
}

fn same_dir(a: &Path, b: &Path) -> bool {
    let n = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    n(a) == n(b)
}

/// 저장할 폴더 — 설정에 고른 폴더(없으면 만든다), 비었거나 못 쓰면 다운로드 폴더
pub fn save_dir(chosen: &str) -> Result<PathBuf, String> {
    let c = chosen.trim();
    if !c.is_empty() {
        let p = PathBuf::from(c);
        if p.is_absolute() && std::fs::create_dir_all(&p).is_ok() {
            return Ok(p);
        }
        crate::log(&format!("[웹] 고른 저장 폴더를 못 써서 다운로드 폴더로 — {c}"));
    }
    crate::outdir::downloads_dir().ok_or_else(|| "다운로드 폴더를 찾지 못했어요".into())
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn run(w: &Web, target: &str, s_quality: u8, down: &Path, progress: &mut dyn FnMut(f64)) -> Result<Did, String> {
    let down = down.to_path_buf();
    let at = place(&down);
    let saved = |p: &Path| Did { head: "저장함".into(), note: at.clone(), say: (format!("{at}에 저장했어요"), file_name(p)), file: Some(p.to_path_buf()) };
    match target {
        "web-save" | "web-get" => fetch(w, &down, progress).map(|p| saved(&p)),
        "web-url" => shortcut(w, &down).map(|p| saved(&p)),
        "web-jpg" | "web-png" => fetch_as(w, &down, &target[4..], s_quality, progress).map(|p| saved(&p)),
        "web-ps" => {
            // 포토샵(CS6 도)이 못 여는 webp · avif 는 PNG 로 바꿔 저장하고 그것을 연다
            let e = ext_of(&shown_name(w));
            let p = if matches!(e.as_str(), "" | "jpg" | "jpeg" | "jfif" | "png" | "gif" | "bmp" | "psd" | "tif" | "tiff") {
                fetch(w, &down, progress)?
            } else {
                fetch_as(w, &down, "png", s_quality, progress)?
            };
            #[cfg(windows)]
            apps::open_with(&apps::photoshop().ok_or("포토샵을 찾지 못했어요")?, Some(&p))?;
            Ok(Did { head: "포토샵으로".into(), note: at.clone(), say: ("포토샵으로 열었어요".into(), format!("{at}에도 저장했어요 · {}", file_name(&p))), file: None })
        }
        "web-figma" | "web-word" => {
            let p = fetch(w, &down, progress)?;
            let figma = target == "web-figma";
            let (name, where_) = if figma { ("피그마", "피그마 캔버스에서") } else { ("Word", "Word 문서에서") };
            #[cfg(windows)]
            {
                clip::image(&p)?;
                let exe = if figma { apps::figma() } else { apps::word() };
                apps::open_with(&exe.ok_or(format!("{name} 프로그램을 찾지 못했어요"))?, None)?;
            }
            Ok(Did { head: format!("{name}에 담음"), note: "Ctrl+V".into(), say: (format!("{where_} Ctrl+V"), format!("{name}을 열고 그림을 복사해 두었어요 · {at}에도 저장")), file: None })
        }
        "web-with" => {
            let p = fetch(w, &down, progress)?;
            #[cfg(windows)]
            apps::choose(&p)?;
            Ok(Did { head: "다른 프로그램".into(), note: at.clone(), say: ("고른 프로그램으로 열어요".into(), format!("{at}에 저장했어요 · {}", file_name(&p))), file: None })
        }
        "web-kakao" => {
            // 그림 · 파일은 받아서 파일로, 링크는 주소 글로 (카톡이 미리 보기를 붙인다)
            let what = if matches!(w.kind, "image" | "file") {
                let p = fetch(w, &down, progress)?;
                #[cfg(windows)]
                clip::files(&[p.clone()])?;
                file_name(&p)
            } else {
                #[cfg(windows)]
                clip::text(&w.url)?;
                w.url.clone()
            };
            #[cfg(windows)]
            let opened = kakao::open();
            #[cfg(not(windows))]
            let opened = false;
            Ok(if opened {
                Did { head: "카톡에 담음".into(), note: "Ctrl+V".into(), say: ("보낼 대화방에서 Ctrl+V".into(), format!("카카오톡을 열었어요 · {what}")), file: None }
            } else {
                Did { head: "복사해 둠".into(), note: "Ctrl+V".into(), say: ("카카오톡을 찾지 못했어요".into(), "복사해 두었어요 — 붙여 넣을 곳에서 Ctrl+V".into()), file: None }
            })
        }
        _ => Err(format!("모르는 칸이에요: {target}")),
    }
}

/// 받아서 `fmt`(jpg · png)로 바꾼 것만 `dir` 에 — 받은 그대로는 임시 폴더에 두었다 지운다
fn fetch_as(w: &Web, dir: &Path, fmt: &str, quality: u8, progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!("mago-web-{}-{}", std::process::id(), chrono::Local::now().timestamp_millis()));
    let src = fetch(w, &tmp, &mut |f| progress(f * 0.8));
    let r = src.and_then(|src| {
        let req = crate::convert::ext_convert::ExtRequest {
            files: vec![src.to_string_lossy().into()],
            out_dir: dir.to_string_lossy().into(),
            image_format: Some(fmt.into()),
            audio_format: None,
            quality: Some(quality),
            bitrate: None,
            background: None,
            pdf_format: None,
            pdf_mode: None,
            pdf_dpi: None,
        };
        let r = crate::convert::ext_convert::run(&req, &mut |_, _, _| {})?;
        match (r.outputs.first(), r.errors.first()) {
            (Some(o), _) => Ok(PathBuf::from(&o.file)),
            (None, Some(e)) => Err(e.error.clone()),
            _ => Err("바꾸지 못했어요".into()),
        }
    });
    let _ = std::fs::remove_dir_all(&tmp);
    let p = r?;
    mark_from_web(&p, &w.url);
    Ok(p)
}

/// 탐색기로 그 파일이 있는 폴더를 열고 파일을 골라 둔다. 경로에 빈칸이 있어도 되게 `/select,"…"` 를 그대로 넘긴다
pub fn reveal(p: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", p.display())).spawn().map(|_| ()).map_err(|e| format!("탐색기를 열지 못했어요: {e}"))
    }
    #[cfg(not(windows))]
    {
        let _ = p;
        Err("윈도우에서만 돼요".into())
    }
}

// ── 설정 창: 저장 폴더 ──────────────────────────────────────────────────────────

/// 설정 창이 처음에 받는 것 — 판 번호 · 다운로드 폴더 (저장 폴더를 안 골랐을 때 보여 줄 곳)
#[tauri::command]
pub fn app_info(app: tauri::AppHandle) -> serde_json::Value {
    serde_json::json!({
        "version": app.package_info().version.to_string(),
        "downloads": crate::outdir::downloads_dir().map(|d| d.display().to_string()).unwrap_or_default(),
        // 이 PC 에 없는 프로그램 칸은 설정에서 "없음" 으로 보인다
        "missing": OPTS.iter().filter(|id| !available(id)).collect::<Vec<_>>(),
    })
}

/// [폴더 바꾸기] — 윈도우 폴더 고르기 창. 고르지 않고 닫으면 None
#[tauri::command]
pub async fn web_folder_pick(app: tauri::AppHandle, current: String) -> Result<Option<String>, String> {
    use tauri::Manager;
    let start = save_dir(&current)?;
    let owner = app.get_webview_window("settings").and_then(|w| w.hwnd().ok()).map(|h| h.0 as isize).unwrap_or(0);
    // 폴더 고르기 창은 COM(STA) 스레드에서 — 화면 스레드를 막지 않게 따로 띄운다
    let got = tauri::async_runtime::spawn_blocking(move || std::thread::spawn(move || pick::folder(owner, &start)).join().ok().flatten())
        .await
        .map_err(|e| e.to_string())?;
    Ok(got.map(|p| p.display().to_string()))
}

/// [폴더 열기] — 지금 저장 폴더를 탐색기로
#[tauri::command]
pub fn web_folder_open(current: String) -> Result<(), String> {
    let d = save_dir(&current)?;
    std::process::Command::new("explorer").arg(&d).spawn().map(|_| ()).map_err(|e| format!("탐색기를 열지 못했어요: {e}"))
}

mod pick {
    use std::path::{Path, PathBuf};

    #[cfg(windows)]
    pub fn folder(owner: isize, start: &Path) -> Option<PathBuf> {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
        use windows::Win32::UI::Shell::{FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH};
        unsafe {
            let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
            let got = (|| {
                let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
                d.SetOptions(d.GetOptions().ok()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST).ok()?;
                let _ = d.SetTitle(&HSTRING::from("인터넷에서 끌어 받은 것을 저장할 폴더"));
                let _ = d.SetOkButtonLabel(&HSTRING::from("이 폴더로"));
                if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(start.as_os_str()), None) {
                    let _ = d.SetFolder(&item);
                }
                d.Show(Some(HWND(owner as *mut _))).ok()?;
                let name = d.GetResult().ok()?.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
                let s = name.to_string().ok();
                CoTaskMemFree(Some(name.0 as *const _));
                s.map(PathBuf::from)
            })();
            if init {
                CoUninitialize();
            }
            got
        }
    }

    #[cfg(not(windows))]
    pub fn folder(_owner: isize, _start: &Path) -> Option<PathBuf> {
        None
    }
}

// ── 인터넷에서 받기 (WinHTTP — 윈도우 프록시 설정을 따른다) ──────────────────────

#[cfg(windows)]
mod http {
    use super::*;
    use std::io::Write;
    use windows_sys::Win32::Networking::WinHttp::*;

    struct H(*mut core::ffi::c_void);
    impl Drop for H {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }
    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn header(r: *mut core::ffi::c_void, q: u32) -> String {
        let mut n = 0u32;
        WinHttpQueryHeaders(r, q, std::ptr::null(), std::ptr::null_mut(), &mut n, std::ptr::null_mut());
        if n == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; n as usize / 2 + 1];
        if WinHttpQueryHeaders(r, q, std::ptr::null(), buf.as_mut_ptr() as *mut _, &mut n, std::ptr::null_mut()) == 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize / 2])
    }

    /// `url` 을 받아 `dir` 에 저장하고 그 경로를. 이름은 서버가 준 이름 › 옮겨 간 주소 끝 › `name`.
    /// 웹 페이지(text/html)는 파일이 아니라고 돌려보낸다 (그림 주소가 '퍼 가기 막음' 페이지를 줄 때도)
    pub fn get(url: &str, dir: &Path, name: &str, progress: &mut dyn FnMut(f64)) -> Result<PathBuf, String> {
        let err = |what: &str| format!("받지 못했어요 ({what} · {})", std::io::Error::last_os_error());
        let wurl = w(url);
        unsafe {
            let mut uc: URL_COMPONENTS = std::mem::zeroed();
            uc.dwStructSize = std::mem::size_of::<URL_COMPONENTS>() as u32;
            uc.dwHostNameLength = u32::MAX;
            uc.dwUrlPathLength = u32::MAX;
            uc.dwExtraInfoLength = u32::MAX;
            uc.dwSchemeLength = u32::MAX;
            if WinHttpCrackUrl(wurl.as_ptr(), 0, 0, &mut uc) == 0 {
                return Err("주소를 읽지 못했어요".into());
            }
            let part = |p: *mut u16, n: u32| if p.is_null() { String::new() } else { String::from_utf16_lossy(std::slice::from_raw_parts(p, n as usize)) };
            let host = part(uc.lpszHostName, uc.dwHostNameLength);
            let path = part(uc.lpszUrlPath, uc.dwUrlPathLength) + &part(uc.lpszExtraInfo, uc.dwExtraInfoLength);
            let secure = uc.nScheme == WINHTTP_INTERNET_SCHEME_HTTPS as i32;

            let s = H(WinHttpOpen(w("Mozilla/5.0 (Windows NT 10.0; Win64; x64) MAGO").as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, std::ptr::null(), std::ptr::null(), 0));
            if s.0.is_null() {
                return Err(err("열기"));
            }
            WinHttpSetTimeouts(s.0, 10_000, 10_000, 20_000, 60_000);
            let c = H(WinHttpConnect(s.0, w(&host).as_ptr(), uc.nPort, 0));
            if c.0.is_null() {
                return Err(err("연결"));
            }
            let r = H(WinHttpOpenRequest(c.0, w("GET").as_ptr(), w(if path.is_empty() { "/" } else { &path }).as_ptr(), std::ptr::null(), std::ptr::null(), std::ptr::null(), if secure { WINHTTP_FLAG_SECURE } else { 0 }));
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
                return Err(match code {
                    401 | 403 => "로그인해야 받을 수 있는 곳이에요 — 브라우저에서 받아 주세요".into(),
                    404 => "그 주소에 파일이 없어요".into(),
                    _ => format!("받지 못했어요 (서버 {code})"),
                });
            }
            let ct = header(r.0, WINHTTP_QUERY_CONTENT_TYPE);
            if ct.to_ascii_lowercase().starts_with("text/html") {
                return Err("파일이 아니라 웹 페이지예요 — 바로가기로 저장해 보세요".into());
            }
            // 옮겨 간 뒤의 주소 (짧은 주소 → 진짜 파일 이름)
            let mut n = 0u32;
            WinHttpQueryOption(r.0, WINHTTP_OPTION_URL, std::ptr::null_mut(), &mut n);
            let mut fin = vec![0u16; n as usize / 2 + 1];
            let final_url = if n > 0 && WinHttpQueryOption(r.0, WINHTTP_OPTION_URL, fin.as_mut_ptr() as *mut _, &mut n) != 0 { String::from_utf16_lossy(&fin[..n as usize / 2]) } else { url.to_string() };
            let pick = disposition_name(&header(r.0, WINHTTP_QUERY_CONTENT_DISPOSITION))
                .or_else(|| Some(last_segment(&final_url)).filter(|x| !x.is_empty()))
                .unwrap_or_else(|| name.to_string());
            let dst = crate::convert::convert::unique_path(dir, &safe_name(&with_ext(pick, &ct), "받은 파일"));

            let total = num(WINHTTP_QUERY_CONTENT_LENGTH) as f64;
            let file = std::fs::File::create(&dst).map_err(|e| format!("저장하지 못했어요: {e}"))?;
            let mut f = std::io::BufWriter::new(file);
            let mut buf = vec![0u8; 256 * 1024];
            let mut done = 0f64;
            let res: Result<(), String> = (|| {
                loop {
                    let mut n = 0u32;
                    if WinHttpReadData(r.0, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut n) == 0 {
                        return Err(err("받기"));
                    }
                    if n == 0 {
                        break;
                    }
                    f.write_all(&buf[..n as usize]).map_err(|e| format!("저장하지 못했어요: {e}"))?;
                    done += n as f64;
                    if total > 0.0 {
                        progress((done / total).min(1.0));
                    }
                }
                f.flush().map_err(|e| format!("저장하지 못했어요: {e}"))?;
                if total > 0.0 && (done - total).abs() > 0.5 {
                    return Err("받다가 끊겼어요".into());
                }
                Ok(())
            })();
            drop(f);
            if let Err(e) = res {
                let _ = std::fs::remove_file(&dst);
                return Err(e);
            }
            Ok(dst)
        }
    }
}

// ── 클립보드 (카톡에 붙여 넣을 것) ──────────────────────────────────────────────

#[cfg(windows)]
pub mod clip {
    use std::path::PathBuf;
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, HWND_MESSAGE};

    const CF_UNICODETEXT: u32 = 13;
    const CF_HDROP: u32 = 15;
    const CF_DIB: u32 = 8;

    fn put(format: u32, data: &[u8]) -> Result<(), String> {
        put_all(&[(format, data.to_vec())])
    }

    /// 그림을 클립보드에 — 피그마 · Word 가 Ctrl+V 로 받는 형식 둘 다 ("PNG" 는 투명 그대로, CF_DIB 는 어디서나)
    pub fn image(p: &std::path::Path) -> Result<(), String> {
        let img = image::open(p).map_err(|e| format!("그림을 읽지 못했어요: {e}"))?.to_rgba8();
        let (w, h) = img.dimensions();
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img.clone()).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| format!("그림을 담지 못했어요: {e}"))?;
        // BITMAPINFOHEADER(40) + 아래 줄부터 BGRA
        let mut dib = Vec::with_capacity(40 + (w * h * 4) as usize);
        for v in [40u32, w] {
            dib.extend_from_slice(&v.to_le_bytes());
        }
        dib.extend_from_slice(&(h as i32).to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        for v in [0u32, w * h * 4, 0, 0, 0, 0] {
            dib.extend_from_slice(&v.to_le_bytes());
        }
        for y in (0..h).rev() {
            for x in 0..w {
                let [r, g, b, a] = img.get_pixel(x, y).0;
                dib.extend_from_slice(&[b, g, r, a]);
            }
        }
        let name: Vec<u16> = "PNG\0".encode_utf16().collect();
        let cf_png = unsafe { windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW(name.as_ptr()) };
        put_all(&[(cf_png, png), (CF_DIB, dib)])
    }

    /// 바이트를 그 형식들로 클립보드에. 클립보드의 주인이 될 보이지 않는 창을 잠깐 만든다 (주인 없이 비우면 넣기가 실패한다)
    fn put_all(items: &[(u32, Vec<u8>)]) -> Result<(), String> {
        unsafe {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let h = CreateWindowExW(0, class.as_ptr(), std::ptr::null(), 0, 0, 0, 0, 0, HWND_MESSAGE, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null());
            // 다른 프로그램이 클립보드를 잠깐 쥐고 있을 수 있다 — 조금 기다려 본다
            let mut open = false;
            for _ in 0..20 {
                if OpenClipboard(h) != 0 {
                    open = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            if !open {
                if !h.is_null() {
                    DestroyWindow(h);
                }
                return Err("클립보드를 쓰지 못했어요".into());
            }
            EmptyClipboard();
            let mut ok = true;
            for (format, data) in items {
                let g = GlobalAlloc(GMEM_MOVEABLE, data.len());
                let one = !g.is_null() && {
                    let p = GlobalLock(g) as *mut u8;
                    if !p.is_null() {
                        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                        GlobalUnlock(g);
                    }
                    !p.is_null() && !SetClipboardData(*format, g).is_null()
                };
                if !one && !g.is_null() {
                    GlobalFree(g);
                }
                ok &= one;
            }
            CloseClipboard();
            if !h.is_null() {
                DestroyWindow(h);
            }
            if ok { Ok(()) } else { Err("클립보드에 넣지 못했어요".into()) }
        }
    }

    pub fn text(s: &str) -> Result<(), String> {
        let b: Vec<u8> = s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        put(CF_UNICODETEXT, &b)
    }

    /// 탐색기에서 Ctrl+C 한 것과 같은 파일 목록 (DROPFILES + 넓은 글자 이름들 + 빈 이름)
    pub fn files(paths: &[PathBuf]) -> Result<(), String> {
        let mut b: Vec<u8> = Vec::new();
        for v in [20i32, 0, 0, 0, 1] {
            b.extend_from_slice(&v.to_le_bytes()); // pFiles · pt.x · pt.y · fNC · fWide
        }
        for p in paths {
            b.extend(p.to_string_lossy().encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
        }
        b.extend_from_slice(&[0, 0]);
        put(CF_HDROP, &b)
    }

    /// 지금 클립보드의 글 (시험용)
    #[cfg(test)]
    pub fn read_text() -> Option<String> {
        use windows_sys::Win32::System::DataExchange::GetClipboardData;
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }
            let g = GetClipboardData(CF_UNICODETEXT);
            let mut out = None;
            if !g.is_null() {
                let p = GlobalLock(g) as *const u16;
                if !p.is_null() {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)));
                    GlobalUnlock(g);
                }
            }
            CloseClipboard();
            out
        }
    }
}

// ── 포토샵 · 피그마 · Word · 다른 프로그램으로 ──────────────────────────────────

#[cfg(windows)]
mod apps {
    use std::path::{Path, PathBuf};

    /// 윈도우가 프로그램 이름으로 찾는 자리 (App Paths) — 시작 메뉴 "실행" 과 같은 곳
    fn app_path(exe: &str) -> Option<PathBuf> {
        use winreg::enums::*;
        use winreg::RegKey;
        for (root, key) in [
            (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths"),
        ] {
            let Ok(k) = RegKey::predef(root).open_subkey(format!("{key}\\{exe}")) else { continue };
            let v: String = k.get_value("").unwrap_or_default();
            let p = PathBuf::from(v.trim().trim_matches('"'));
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    pub fn photoshop() -> Option<PathBuf> {
        app_path("Photoshop.exe").or_else(|| {
            // App Paths 가 없는 판 — Adobe 폴더에서 이름 순으로 마지막(대개 가장 새것)
            let mut v: Vec<PathBuf> = std::fs::read_dir(r"C:\Program Files\Adobe").ok()?.flatten().map(|d| d.path().join("Photoshop.exe")).filter(|p| p.is_file()).collect();
            v.sort();
            v.pop()
        })
    }

    pub fn word() -> Option<PathBuf> {
        app_path("WINWORD.EXE")
    }

    /// 피그마 데스크톱 — 사용자 폴더에 깔린다
    pub fn figma() -> Option<PathBuf> {
        let local = std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join("Figma").join("Figma.exe"));
        local.filter(|p| p.is_file()).or_else(|| app_path("Figma.exe"))
    }

    /// 그 프로그램을 연다 (파일이 있으면 그 파일로). 켜져 있으면 그 창이 앞으로 나온다
    pub fn open_with(exe: &Path, file: Option<&Path>) -> Result<(), String> {
        let mut c = std::process::Command::new(exe);
        if let Some(f) = file {
            c.arg(f);
        }
        c.spawn().map(|_| crate::log(&format!("[웹] 엶 {}", exe.display()))).map_err(|e| format!("프로그램을 열지 못했어요: {e}"))
    }

    /// 윈도우의 "연결 프로그램" 창 — 고른 프로그램으로 연다 (기본 앱은 바꾸지 않는다)
    pub fn choose(file: &Path) -> Result<(), String> {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::{SHOpenWithDialog, OAIF_EXEC, OAIF_HIDE_REGISTRATION, OPENASINFO};
        let f = HSTRING::from(file.as_os_str());
        let info = OPENASINFO { pcszFile: windows::core::PCWSTR(f.as_ptr()), pcszClass: windows::core::PCWSTR::null(), oaifInFlags: OAIF_EXEC | OAIF_HIDE_REGISTRATION };
        // 창을 닫고 고르지 않아도 오류가 온다 — 저장은 이미 했으니 실패로 치지 않는다
        if let Err(e) = unsafe { SHOpenWithDialog(None, &info) } {
            crate::log(&format!("[웹] 연결 프로그램 창: {e}"));
        }
        Ok(())
    }
}

// ── 카카오톡 열기 ─────────────────────────────────────────────────────────────

#[cfg(windows)]
mod kakao {
    use std::path::PathBuf;

    /// 깔린 카카오톡 — 지우기 목록(설치 위치) › 흔한 자리
    pub fn exe() -> Option<PathBuf> {
        use winreg::enums::*;
        use winreg::RegKey;
        for (root, key) in [
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
            (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        ] {
            let Ok(k) = RegKey::predef(root).open_subkey(key) else { continue };
            for name in k.enum_keys().flatten() {
                let Ok(app) = k.open_subkey(&name) else { continue };
                let shown: String = app.get_value("DisplayName").unwrap_or_default();
                if shown != "카카오톡" && !shown.eq_ignore_ascii_case("KakaoTalk") {
                    continue;
                }
                let icon: String = app.get_value("DisplayIcon").unwrap_or_default();
                let p = PathBuf::from(icon.split(',').next().unwrap_or("").trim_matches('"'));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        [r"C:\Program Files (x86)\Kakao\KakaoTalk\KakaoTalk.exe", r"C:\Program Files\Kakao\KakaoTalk\KakaoTalk.exe"].iter().map(PathBuf::from).find(|p| p.is_file())
    }

    /// 카카오톡을 연다 (켜져 있으면 그 창이 앞으로 나온다). 깔려 있지 않으면 false
    pub fn open() -> bool {
        let Some(p) = exe() else { return false };
        match std::process::Command::new(&p).spawn() {
            Ok(_) => {
                crate::log("[웹] 카카오톡 엶");
                true
            }
            Err(e) => {
                crate::log(&format!("[웹] 카카오톡을 못 엶: {e}"));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(url: &str, name: &str) -> &'static str {
        classify(url, "", name, None).map(|w| w.kind).unwrap_or("-")
    }

    #[test]
    fn what_was_dragged_from_the_browser() {
        assert_eq!(kind("https://www.youtube.com/watch?v=abc", ""), "youtube");
        assert_eq!(kind("https://youtu.be/abc", ""), "youtube");
        assert_eq!(kind("https://m.youtube.com/shorts/abc", ""), "youtube");
        assert_eq!(kind("https://notyoutube.com/watch", ""), "link");
        // 크롬이 그림을 끌 때 — 내용과 이름을 같이 준다. 링크 안의 그림이어도 그림
        assert_eq!(kind("https://shop.com/item/12", "상품.webp"), "image");
        // 크롬이 링크를 끌 때 같이 주는 바로가기 이름은 제목으로만
        let w = classify("https://news.com/a/1", "", "오늘의 뉴스.url", None).unwrap();
        assert_eq!((w.kind, w.title.as_str(), w.name.as_str()), ("link", "오늘의 뉴스", ""));
        assert_eq!(kind("https://a.com/files/보고서.pdf?dl=1", ""), "file");
        assert_eq!(kind("https://a.com/img/cat.JPG", ""), "image");
        assert_eq!(kind("data:image/png;base64,iVBORw0KGgo=", ""), "image");
        // 받지 않는 주소
        assert_eq!(kind("file:///C:/Windows/win.ini", ""), "-");
        assert_eq!(kind("javascript:alert(1)", ""), "-");
        assert_eq!(kind("", ""), "-");
        assert_eq!(classify("javascript:x", "", "a.png", None).unwrap().url, "", "이상한 주소는 버리고 내용만");
    }

    /// 설정의 저장 폴더 — 고른 폴더(없으면 만든다), 비었거나 상대 경로면 다운로드 폴더
    #[test]
    fn the_save_folder_comes_from_settings() {
        let down = crate::outdir::downloads_dir().unwrap();
        assert_eq!(save_dir("").unwrap(), down);
        assert_eq!(save_dir("상대/경로").unwrap(), down, "상대 경로는 못 쓴다");
        let mine = std::env::temp_dir().join(format!("mago-web-dir-{}", std::process::id())).join("받은 그림");
        assert_eq!(save_dir(&mine.to_string_lossy()).unwrap(), mine);
        assert!(mine.is_dir(), "없는 폴더는 만든다");
        assert_eq!(place(&down), "다운로드 폴더");
        assert_eq!(place(&mine), "받은 그림 폴더");
        let _ = std::fs::remove_dir_all(mine.parent().unwrap());
    }

    #[test]
    fn cells_for_each_kind() {
        let w = |url: &str, name: &str| classify(url, "", name, None).unwrap();
        assert_eq!(ids(&w("https://a.com/x", "a.webp")), ["web-save", "web-jpg", "web-png", "web-ps", "web-figma", "web-word", "web-with", "web-kakao"]);
        assert_eq!(ids(&w("https://a.com/x", "a.jpg")), ["web-save", "web-png", "web-ps", "web-figma", "web-word", "web-with", "web-kakao"], "이미 JPG 면 JPG 칸은 없다");
        assert_eq!(ids(&w("https://a.com/x", "a.avif")), ["web-save", "web-ps", "web-with", "web-kakao"], "못 읽는 그림은 바꾸기 · 클립보드 칸이 없다");
        assert_eq!(ids(&w("https://a.com/a.zip", "")), ["web-get", "web-url", "web-with", "web-kakao"]);
        assert_eq!(ids(&w("https://www.youtube.com/watch?v=1", "")), ["web-url", "web-kakao"]);
        assert_eq!(title(&w("https://www.naver.com/news", "")), "naver.com");
        assert_eq!(title(&w("https://a.com/a.zip", "")), "ZIP 파일");
        assert_eq!(note(&w("https://youtu.be/1", "")), "영상 받기는 아직이에요");
    }

    #[test]
    fn names_are_safe_for_windows() {
        assert_eq!(safe_name("a/b:c?.png", "x"), "a_b_c_.png");
        assert_eq!(safe_name("  ..  ", "받은 파일"), "받은 파일");
        assert_eq!(safe_name("CON.txt", "x"), "_CON.txt");
        assert_eq!(safe_name(&format!("{}.jpg", "가".repeat(300)), "x").chars().count(), 104);
        assert_eq!(last_segment("https://a.com/x/%EC%82%AC%EC%A7%84.png?w=1#t"), "사진.png");
        assert_eq!(last_segment("https://a.com"), "");
        assert_eq!(host("https://user@WWW.A.com:8080/p"), "www.a.com");
        assert_eq!(disposition_name("attachment; filename=\"a b.pdf\"; filename*=UTF-8''%EB%B3%B4%EA%B3%A0.pdf").as_deref(), Some("보고.pdf"));
        assert_eq!(disposition_name("inline; filename=report.pdf").as_deref(), Some("report.pdf"));
        assert_eq!(disposition_name("inline"), None);
        assert_eq!(with_ext("photo".into(), "image/jpeg; charset=x"), "photo.jpg");
        assert_eq!(with_ext("a.png".into(), "image/jpeg"), "a.png");
        assert_eq!(base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(data_url("data:image/png;base64,aGk=").unwrap(), b"hi");
    }

    /// 이 PC 안의 작은 서버에서 받기 — 서버가 준 이름 · 형식으로 붙인 확장자 · 웹 페이지 거절 · 인터넷에서 온 표시
    #[test]
    fn downloads_a_file_link_into_the_folder() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for s in l.incoming().take(3).flatten() {
                let mut s = s;
                let mut req = [0u8; 2048];
                let n = s.read(&mut req).unwrap_or(0);
                let line = String::from_utf8_lossy(&req[..n]).lines().next().unwrap_or("").to_string();
                let (head, body): (&str, &[u8]) = if line.contains("/report") {
                    ("Content-Type: application/pdf\r\nContent-Disposition: attachment; filename*=UTF-8''%EB%B3%B4%EA%B3%A0%EC%84%9C.pdf", b"%PDF-1.4 hello")
                } else if line.contains("/pic") {
                    ("Content-Type: image/png", b"\x89PNG....")
                } else {
                    ("Content-Type: text/html; charset=utf-8", b"<html></html>")
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\n{head}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = s.write_all(body);
            }
        });
        let d = std::env::temp_dir().join(format!("mago-web-get-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let w = |p: &str, kind: &'static str| Web { url: format!("http://127.0.0.1:{port}{p}"), kind, ..Default::default() };
        let mut ticks = 0;
        let a = fetch(&w("/report?id=7", "file"), &d, &mut |_| ticks += 1).unwrap();
        assert_eq!(file_name(&a), "보고서.pdf", "서버가 준 이름");
        assert_eq!(std::fs::read(&a).unwrap(), b"%PDF-1.4 hello");
        assert!(ticks > 0);
        let zone = std::fs::read_to_string(format!("{}:Zone.Identifier", a.display())).unwrap();
        assert!(zone.contains("ZoneId=3") && zone.contains("HostUrl=http://127.0.0.1"), "{zone}");
        let b = fetch(&w("/pic", "image"), &d, &mut |_| {}).unwrap();
        assert_eq!(file_name(&b), "pic.png", "이름에 확장자가 없으면 형식으로");
        let e = fetch(&w("/page", "file"), &d, &mut |_| {}).unwrap_err();
        assert!(e.contains("웹 페이지"), "{e}");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2, "실패한 것은 남기지 않는다");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn saves_what_the_browser_handed_over_and_makes_shortcuts() {
        let d = std::env::temp_dir().join(format!("mago-web-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let w = Web { url: "https://a.com/x".into(), name: "고양이.png".into(), kind: "image", bytes: Some(vec![1, 2, 3]), ..Default::default() };
        let p1 = fetch(&w, &d, &mut |_| {}).unwrap();
        let p2 = fetch(&w, &d, &mut |_| {}).unwrap();
        assert_eq!((file_name(&p1).as_str(), file_name(&p2).as_str()), ("고양이.png", "고양이 (2).png"), "같은 이름이면 (2)");
        let link = Web { url: "https://news.com/a?b=1".into(), title: "뉴스: 오늘?".into(), kind: "link", ..Default::default() };
        let s = shortcut(&link, &d).unwrap();
        assert_eq!(file_name(&s), "뉴스_ 오늘_.url");
        assert_eq!(std::fs::read_to_string(&s).unwrap(), "[InternetShortcut]\r\nURL=https://news.com/a?b=1\r\n");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// 손으로 돌리는 확인 — 이 PC 의 클립보드를 쓴다 (원래 글은 되돌려 둔다. 글이 아닌 것은 지워진다)
    #[test]
    #[ignore]
    fn a_link_for_kakao_goes_to_the_clipboard() {
        let before = clip::read_text();
        clip::text("https://youtu.be/abc").unwrap();
        let got = clip::read_text();
        clip::files(&[PathBuf::from(r"C:.png")]).unwrap();
        if let Some(t) = before {
            clip::text(&t).unwrap();
        }
        assert_eq!(got.as_deref(), Some("https://youtu.be/abc"));
    }
}
