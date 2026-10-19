//! 파일 형식 바꾸기 엔진 — 컴뚝딱(D:\\computer) `src-tauri/src/imagery/` 에서 그대로 가져왔다 (2026-10-20).
//! - `convert`    : 그림 읽기·저장 (png·jpg·webp·bmp·gif, 디컴프레션 폭탄 상한)
//! - `ext_convert`: 그림끼리 · wav ↔ mp3 · PDF → jpg·png 일괄 바꾸기
//! - `pdf_render` : Windows 내장 PDF 그리기 (Windows.Data.Pdf)
//! 컴뚝딱 쪽을 고치면 여기도 같이 고칠지 판단할 것 — 두 프로그램은 코드를 공유하지 않는다(따로 배포).

use serde::Serialize;

pub mod convert;
pub mod ext_convert;
pub mod pdf_render;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputFile {
    pub source: String,
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// 어떤 규칙으로 만든 것인지 (프리셋 id·위치 이름 등)
    pub label: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BatchError {
    pub file: String,
    pub error: String,
}

/// 일괄 처리 결과. **한 파일이 실패해도 나머지는 계속한다** — 스무 장 중 한 장 깨졌다고 전부 버리면 안 된다.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub outputs: Vec<OutputFile>,
    pub errors: Vec<BatchError>,
    /// 하려고 했던 작업 수 (파일 수 × 크기 수)
    pub total: usize,
}

