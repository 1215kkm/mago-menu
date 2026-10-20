//! 글(TXT) 파일을 다른 문서로 (10-20 CEO "txt 잡고 끌면 pdf, 한글, word, 엑셀에 탭단위로 셀로").
//! - `docx`  : Word — 줄마다 문단, 탭은 Word 의 탭
//! - `hwpx`  : 한글 — python-hwpx 의 빈 문서 틀(assets/hwpx, Apache-2.0)의 첫 문단 자리부터 줄마다 문단
//! - `xlsx`  : 엑셀 — 줄마다 한 행, **탭마다 한 칸**. 숫자로만 된 칸은 숫자로(앞자리 0 은 글 그대로 — 전화번호·우편번호)
//! - PDF 는 화면 엔진(WebView2)의 'PDF 로 인쇄'로 만든다 — 한글·한자·일본어 글꼴이 그대로 들어간다 (`pdf`)
//! 모두 원본 옆에 `이름.docx` 처럼 새로 만들고, 같은 이름이 있으면 `(2)` 를 붙인다. 원본은 읽기만 한다.

use crate::convert::convert::unique_path;
use crate::convert::OutputFile;
use std::io::Write;
use std::path::Path;
use zip::write::SimpleFileOptions;

/// 바꿀 수 있는 글 파일 확장자
pub const TEXT_EXT: &[&str] = &["txt", "tsv"];
/// 너무 큰 글은 받지 않는다 (엑셀 한 장 · Word 한 문서로 열 만한 크기)
const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// 글 파일 읽기 — UTF-8 / UTF-16(BOM) / 한국어 윈도우 기본(CP949, 메모장의 'ANSI') 을 알아서
pub fn read_text(p: &Path) -> Result<String, String> {
    let len = std::fs::metadata(p).map_err(|e| format!("파일을 열지 못했어요: {e}"))?.len();
    if len > MAX_BYTES {
        return Err(format!("글이 너무 커요 ({}MB, 20MB 까지)", len / 1024 / 1024));
    }
    let b = std::fs::read(p).map_err(|e| format!("파일을 읽지 못했어요: {e}"))?;
    Ok(decode(&b))
}

fn decode(b: &[u8]) -> String {
    let t = if let Some(r) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(r).into_owned()
    } else if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        let (enc, _) = encoding_rs::Encoding::for_bom(b).unwrap();
        enc.decode_with_bom_removal(b).0.into_owned()
    } else if let Ok(s) = std::str::from_utf8(b) {
        s.to_string()
    } else {
        encoding_rs::EUC_KR.decode(b).0.into_owned()
    };
    t.replace("\r\n", "\n").replace('\r', "\n")
}

fn lines(t: &str) -> Vec<&str> {
    let mut v: Vec<&str> = t.split('\n').collect();
    // 끝의 빈 줄 하나(파일 끝 줄바꿈)는 문단으로 치지 않는다
    if v.len() > 1 && v.last() == Some(&"") {
        v.pop();
    }
    v
}

/// XML 안에 넣을 글 — 특수 문자를 바꾸고, XML 에 못 넣는 제어 문자(탭 · 줄바꿈 말고)는 뺀다
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\t' | '\n' => o.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => o.push(c),
        }
    }
    o
}

fn stem(src: &Path) -> String {
    src.file_stem().and_then(|s| s.to_str()).unwrap_or("글").to_string()
}

fn out(src: &Path, dst: &Path, label: &str) -> Result<OutputFile, String> {
    let bytes = std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0);
    Ok(OutputFile { source: src.to_string_lossy().into(), file: dst.to_string_lossy().into(), width: 0, height: 0, bytes, label: label.into() })
}

/// zip 묶음 쓰기. (이름, 내용, 압축 안 함)
fn write_zip(dst: &Path, parts: &[(&str, Vec<u8>, bool)]) -> Result<(), String> {
    let f = std::fs::File::create(dst).map_err(|e| format!("파일을 만들지 못했어요: {e}"))?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(f));
    let res = (|| -> zip::result::ZipResult<()> {
        for (name, data, stored) in parts {
            let o = SimpleFileOptions::default().compression_method(if *stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated });
            z.start_file(*name, o)?;
            z.write_all(data)?;
        }
        z.finish()?;
        Ok(())
    })();
    res.map_err(|e| {
        let _ = std::fs::remove_file(dst);
        format!("파일을 쓰지 못했어요: {e}")
    })
}

// ── Word ──────────────────────────────────────────────────────────────────

pub fn docx(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let t = read_text(src)?;
    let dst = unique_path(out_dir, &format!("{}.docx", stem(src)));
    write_zip(&dst, &docx_parts(&t))?;
    out(src, &dst, "Word")
}

fn docx_parts(t: &str) -> Vec<(&'static str, Vec<u8>, bool)> {
    let mut body = String::new();
    for l in lines(t) {
        body.push_str("<w:p>");
        for (i, piece) in l.split('\t').enumerate() {
            if i > 0 {
                body.push_str("<w:r><w:tab/></w:r>");
            }
            if !piece.is_empty() {
                body.push_str(&format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(piece)));
            }
        }
        body.push_str("</w:p>");
    }
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1304" w:bottom="1440" w:left="1304" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    // 글꼴: 한글은 맑은 고딕 (Word 기본 글꼴이 한글을 못 그리는 PC 가 있다)
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Malgun Gothic" w:eastAsia="Malgun Gothic" w:hAnsi="Malgun Gothic" w:cs="Malgun Gothic"/><w:sz w:val="21"/><w:szCs w:val="21"/><w:lang w:val="en-US" w:eastAsia="ko-KR"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="0" w:line="300" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults></w:styles>"#;
    let types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    vec![
        ("[Content_Types].xml", types.into(), false),
        ("_rels/.rels", rels.into(), false),
        ("word/document.xml", doc.into_bytes(), false),
        ("word/styles.xml", styles.into(), false),
        ("word/_rels/document.xml.rels", doc_rels.into(), false),
    ]
}

// ── 엑셀 ──────────────────────────────────────────────────────────────────

pub fn xlsx(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let t = read_text(src)?;
    let dst = unique_path(out_dir, &format!("{}.xlsx", stem(src)));
    let rows = lines(&t).len();
    if rows > 1_048_576 {
        return Err("엑셀 한 장에 들어가지 않을 만큼 줄이 많아요".into());
    }
    write_zip(&dst, &xlsx_parts(&t))?;
    out(src, &dst, "엑셀")
}

/// 칸 이름 — 0 → A, 25 → Z, 26 → AA
fn col_name(mut i: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (i % 26) as u8);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// 숫자로 넣을 칸인가 — 엑셀이 숫자로 알아보는 모양만. 앞자리 0(010…, 00123)은 글로 둔다
fn as_number(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() || t.len() > 15 {
        return None;
    }
    let digits = t.strip_prefix('-').unwrap_or(t);
    if digits.len() > 1 && digits.starts_with('0') && !digits.starts_with("0.") {
        return None;
    }
    let ok = digits.chars().all(|c| c.is_ascii_digit() || c == '.') && digits.chars().filter(|c| *c == '.').count() <= 1 && digits.chars().any(|c| c.is_ascii_digit()) && !digits.ends_with('.') && !digits.starts_with('.');
    ok.then_some(t)
}

fn xlsx_parts(t: &str) -> Vec<(&'static str, Vec<u8>, bool)> {
    let mut sheet = String::from(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#);
    let rows: Vec<Vec<&str>> = lines(t).iter().map(|l| l.split('\t').collect()).collect();
    // 칸 너비 — 칸마다 가장 긴 글에 맞춰 (한글은 두 칸 몫), 8~60
    let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let mut width = vec![8.0f64; ncol];
    for r in &rows {
        for (i, c) in r.iter().enumerate() {
            let w: f64 = c.chars().map(|ch| if ch.is_ascii() { 1.0 } else { 2.0 }).sum::<f64>() + 2.0;
            width[i] = width[i].max(w.min(60.0));
        }
    }
    sheet.push_str("<cols>");
    for (i, w) in width.iter().enumerate() {
        sheet.push_str(&format!(r#"<col min="{0}" max="{0}" width="{1:.1}" customWidth="1"/>"#, i + 1, w));
    }
    sheet.push_str("</cols><sheetData>");
    for (ri, r) in rows.iter().enumerate() {
        sheet.push_str(&format!(r#"<row r="{}">"#, ri + 1));
        for (ci, c) in r.iter().enumerate() {
            if c.is_empty() {
                continue;
            }
            let at = format!("{}{}", col_name(ci), ri + 1);
            match as_number(c) {
                Some(n) => sheet.push_str(&format!(r#"<c r="{at}"><v>{n}</v></c>"#)),
                None => sheet.push_str(&format!(r#"<c r="{at}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#, esc(c))),
            }
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");
    let types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
    let wb = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let wb_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="맑은 고딕"/><family val="3"/><charset val="129"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;
    vec![
        ("[Content_Types].xml", types.into(), false),
        ("_rels/.rels", rels.into(), false),
        ("xl/workbook.xml", wb.into(), false),
        ("xl/_rels/workbook.xml.rels", wb_rels.into(), false),
        ("xl/worksheets/sheet1.xml", sheet.into_bytes(), false),
        ("xl/styles.xml", styles.into(), false),
    ]
}

// ── 한글 ──────────────────────────────────────────────────────────────────

const HWPX_SECTION: &str = include_str!("../assets/hwpx/section0.xml");
/// 빈 문서 틀의 첫 문단 안 빈 글 자리 — 여기에 첫 줄을 넣는다
const HWPX_EMPTY_RUN: &str = r#"<hp:run charPrIDRef="0"><hp:t/></hp:run>"#;

pub fn hwpx(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let t = read_text(src)?;
    let dst = unique_path(out_dir, &format!("{}.hwpx", stem(src)));
    write_zip(&dst, &hwpx_parts(&t))?;
    out(src, &dst, "한글")
}

fn hwpx_run(l: &str) -> String {
    // 탭은 한글 문단에서 빈칸 네 개로 (탭 칸 정보는 틀에 맞춰야 해서 쓰지 않는다)
    let l = l.replace('\t', "    ");
    if l.is_empty() {
        r#"<hp:run charPrIDRef="0"><hp:t/></hp:run>"#.into()
    } else {
        format!(r#"<hp:run charPrIDRef="0"><hp:t>{}</hp:t></hp:run>"#, esc(&l))
    }
}

fn hwpx_section(t: &str) -> String {
    let ls = lines(t);
    let mut sec = HWPX_SECTION.replacen(HWPX_EMPTY_RUN, &hwpx_run(ls.first().copied().unwrap_or("")), 1);
    // 줄 높이 정보는 한글이 열 때 다시 잰다 — 틀의 첫 줄 정보는 빈 문단 기준이라 뺀다
    if let (Some(a), Some(b)) = (sec.find("<hp:linesegarray>"), sec.find("</hp:linesegarray>")) {
        sec.replace_range(a..b + "</hp:linesegarray>".len(), "");
    }
    let mut more = String::new();
    for (i, l) in ls.iter().enumerate().skip(1) {
        more.push_str(&format!(r#"<hp:p id="{i}" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0">{}</hp:p>"#, hwpx_run(l)));
    }
    sec.replacen("</hs:sec>", &format!("{more}</hs:sec>"), 1)
}

fn hwpx_parts(t: &str) -> Vec<(&'static str, Vec<u8>, bool)> {
    let preview: String = t.chars().take(1000).collect();
    vec![
        // mimetype 은 맨 앞에, 압축하지 않고 (한글이 이걸 보고 HWPX 로 알아본다)
        ("mimetype", b"application/hwp+zip".to_vec(), true),
        ("version.xml", include_bytes!("../assets/hwpx/version.xml").to_vec(), false),
        ("Contents/header.xml", include_bytes!("../assets/hwpx/header.xml").to_vec(), false),
        ("Contents/section0.xml", hwpx_section(t).into_bytes(), false),
        ("Preview/PrvText.txt", preview.into_bytes(), false),
        ("settings.xml", include_bytes!("../assets/hwpx/settings.xml").to_vec(), false),
        ("META-INF/container.rdf", include_bytes!("../assets/hwpx/container.rdf").to_vec(), false),
        ("Contents/content.hpf", include_bytes!("../assets/hwpx/content.hpf").to_vec(), false),
        ("META-INF/container.xml", include_bytes!("../assets/hwpx/container.xml").to_vec(), false),
        ("META-INF/manifest.xml", include_bytes!("../assets/hwpx/manifest.xml").to_vec(), false),
    ]
}

// ── PDF ───────────────────────────────────────────────────────────────────

/// 인쇄용 HTML — 글은 모두 이스케이프하고 스크립트는 넣지 않는다
fn pdf_html(t: &str, title: &str) -> String {
    let body = esc(t).replace('"', "&quot;");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>{}</title><style>
html, body {{ margin: 0; background: #fff; color: #111; }}
pre {{ margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; tab-size: 4;
  font: 10.5pt/1.65 "Malgun Gothic", "맑은 고딕", "Yu Gothic", "Microsoft YaHei", "Microsoft JhengHei", "Segoe UI", sans-serif; }}
</style></head><body><pre>{body}</pre></body></html>"#,
        esc(title)
    )
}

/// PDF — 숨은 화면 창에서 'PDF 로 인쇄' (lib.rs print_pdf)
pub fn pdf(src: &Path, out_dir: &Path) -> Result<OutputFile, String> {
    let t = read_text(src)?;
    let dst = unique_path(out_dir, &format!("{}.pdf", stem(src)));
    let html = std::env::temp_dir().join(format!("mago-pdf-{}-{}.html", std::process::id(), chrono::Local::now().timestamp_millis()));
    std::fs::write(&html, pdf_html(&t, &stem(src))).map_err(|e| format!("임시 파일을 만들지 못했어요: {e}"))?;
    let r = crate::pdfprint::print(&html, &dst);
    let _ = std::fs::remove_file(&html);
    r?;
    if std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(&dst);
        return Err("PDF 를 만들지 못했어요".into());
    }
    out(src, &dst, "PDF")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mago-text-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn part(zip_path: &Path, name: &str) -> String {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path).unwrap()).unwrap();
        let mut s = String::new();
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn reads_utf8_utf16_and_cp949() {
        assert_eq!(decode("가나\r\n다".as_bytes()), "가나\n다");
        assert_eq!(decode(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        let u16: Vec<u8> = [0xFF, 0xFE].into_iter().chain("한글".encode_utf16().flat_map(|c| c.to_le_bytes())).collect();
        assert_eq!(decode(&u16), "한글");
        // 메모장 'ANSI'(CP949)로 저장한 "안녕"
        assert_eq!(decode(&[0xBE, 0xC8, 0xB3, 0xE7]), "안녕");
    }

    #[test]
    fn excel_puts_each_tab_in_its_own_cell() {
        let d = tmp("xlsx");
        let src = d.join("명단.txt");
        std::fs::write(&src, "이름\t나이\t전화\n김철수\t31\t01012345678\n<이영희>\t-2.5\t\n").unwrap();
        let o = xlsx(&src, &d).unwrap();
        assert!(o.file.ends_with("명단.xlsx"));
        let s = part(Path::new(&o.file), "xl/worksheets/sheet1.xml");
        assert!(s.contains(r#"<c r="A1" t="inlineStr"><is><t xml:space="preserve">이름</t></is></c>"#), "{s}");
        assert!(s.contains(r#"<c r="C1" t="inlineStr">"#));
        assert!(s.contains(r#"<c r="B2"><v>31</v></c>"#), "숫자는 숫자 칸");
        assert!(s.contains(r#"<c r="C2" t="inlineStr"><is><t xml:space="preserve">01012345678</t>"#), "앞자리 0 은 글 그대로");
        assert!(s.contains("&lt;이영희&gt;") && s.contains(r#"<c r="B3"><v>-2.5</v></c>"#));
        assert!(!s.contains(r#"r="C3""#), "빈 칸은 쓰지 않는다");
        assert!(src.exists());
    }

    #[test]
    fn column_names() {
        assert_eq!([col_name(0), col_name(25), col_name(26), col_name(701), col_name(702)], ["A", "Z", "AA", "ZZ", "AAA"]);
        assert_eq!(as_number("010"), None);
        assert_eq!(as_number("0.5"), Some("0.5"));
        assert_eq!(as_number("1,000"), None);
        assert_eq!(as_number("2026-10-20"), None);
    }

    #[test]
    fn word_has_a_paragraph_per_line_and_word_tabs() {
        let d = tmp("docx");
        let src = d.join("메모.txt");
        std::fs::write(&src, "첫 줄\n둘\t셋 & <넷>\n").unwrap();
        let o = docx(&src, &d).unwrap();
        let s = part(Path::new(&o.file), "word/document.xml");
        assert_eq!(s.matches("<w:p>").count(), 2);
        assert!(s.contains("<w:r><w:tab/></w:r>") && s.contains("셋 &amp; &lt;넷&gt;"), "{s}");
        // 같은 이름이 있으면 (2)
        let o2 = docx(&src, &d).unwrap();
        assert!(o2.file.ends_with("메모 (2).docx") || o2.file.ends_with("메모(2).docx"), "{}", o2.file);
    }

    #[test]
    fn hangul_file_keeps_the_skeleton_and_adds_paragraphs() {
        let d = tmp("hwpx");
        let src = d.join("보고서.txt");
        std::fs::write(&src, "제목\n\n본문\t탭").unwrap();
        let o = hwpx(&src, &d).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&o.file).unwrap()).unwrap();
        // mimetype 이 맨 앞 · 압축 안 함
        let first = z.by_index(0).unwrap();
        assert_eq!((first.name(), first.compression()), ("mimetype", zip::CompressionMethod::Stored));
        drop(first);
        let s = part(Path::new(&o.file), "Contents/section0.xml");
        assert!(s.contains("<hp:secPr") && s.contains("<hp:t>제목</hp:t>"), "첫 줄은 구역 정보가 있는 첫 문단에");
        assert!(s.contains(r#"<hp:p id="2" "#) && s.contains("<hp:t>본문    탭</hp:t>"));
        assert!(!s.contains("linesegarray"));
        assert!(s.trim_end().ends_with("</hs:sec>"));
    }

    #[test]
    fn pdf_page_escapes_the_text() {
        let h = pdf_html("<script>alert(1)</script>\n두 줄", "a<b");
        assert!(!h.contains("<script>") && h.contains("&lt;script&gt;") && h.contains("<title>a&lt;b</title>"));
    }
}
