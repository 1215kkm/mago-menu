//! **파일이 실제로 어디에 저장되는가.**
//!
//! ## 왜 생겼나 — GIF 슬롯머신이 안 되던 진짜 이유
//! 화면(`tab-resize.js` `outDir()`)이 저장 폴더를 아직 안 고른 사용자에게 `내 문서\컴뚝딱 이미지` 라는
//! **사람에게 보여 주려고 쓴 글자**를 경로처럼 돌려줬다. 그래서 `내 문서\컴뚝딱 이미지\GIF\슬롯머신_007.gif`
//! 라는 *존재하지 않는 상대 경로*로 저장을 시도했고, 폴더를 한 번도 안 고른 사용자는 100% 실패했다.
//! 크기 변환·워터마크·배경·아이콘도 같은 함수를 써서 같은 병을 앓았다.
//!
//! ## 그래서 기본 폴더는 Rust 가 정한다
//! `SHGetKnownFolderPath(FOLDERID_Documents)` 로 **진짜 내 문서 위치**를 묻는다.
//! `%USERPROFILE%\Documents` 를 그냥 쓰면 안 된다 — 문서 폴더를 D 드라이브로 옮겼거나 OneDrive 로
//! 리디렉션된 PC 가 흔하고, 그런 PC 에서는 엉뚱한 곳에 폴더가 생기거나 만들어지지도 않는다.
//!
//! ## 저장 직전에 한 번 더 본다
//! 절대 경로인가 → 폴더가 있나(없으면 만든다) → **진짜 쓸 수 있나**(작은 파일을 한 번 쓰고 지운다).
//! 안 되면 *왜* 안 되는지 사람 말로 돌려준다. 조용히 실패하는 것이 제일 나쁘다.

#![allow(dead_code)] // 컴뚝딱에서 가져온 그대로 — 일부(번호 이어 붙이기·다운로드 폴더)는 안 쓴다
use std::path::{Path, PathBuf};
use windows_sys::core::{GUID, PWSTR};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;

/// 우리가 만드는 폴더 이름. 사용자가 탐색기에서 이 이름을 읽는다.
pub const BASE_FOLDER: &str = "MAGO";

/// FOLDERID_Documents = {FDD39AD0-238F-46AF-ADB4-6C85480369C7}
const FOLDERID_DOCUMENTS: GUID = GUID {
    data1: 0xFDD3_9AD0,
    data2: 0x238F,
    data3: 0x46AF,
    data4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
};

/// FOLDERID_Downloads = {374DE290-123F-4565-9164-39C4925E467B}
const FOLDERID_DOWNLOADS: GUID = GUID {
    data1: 0x374D_E290,
    data2: 0x123F,
    data3: 0x4565,
    data4: [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B],
};

/// 진짜 다운로드 폴더 (09-29 CEO: 선택 영역 저장의 기본 위치). 옮긴 사용자도 있어 셸에 묻는다.
pub fn downloads_dir() -> Option<PathBuf> {
    if let Some(p) = known_folder(&FOLDERID_DOWNLOADS) {
        return Some(p);
    }
    std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Downloads"))
}

/// 진짜 내 문서 폴더. 못 물어보면 `%USERPROFILE%\Documents` 로 물러선다.
pub fn documents_dir() -> Option<PathBuf> {
    if let Some(p) = known_folder(&FOLDERID_DOCUMENTS) {
        return Some(p);
    }
    std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents"))
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    unsafe {
        let mut raw: PWSTR = std::ptr::null_mut();
        let hr = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw);
        if hr < 0 || raw.is_null() {
            return None;
        }
        let mut len = 0usize;
        while *raw.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(raw, len));
        CoTaskMemFree(raw as *const core::ffi::c_void);
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
}

/// 아무것도 안 고른 사용자의 기본 저장 폴더. **만들지는 않는다** (화면에 보여 주기만 할 수도 있다).
pub fn default_base() -> PathBuf {
    documents_dir().unwrap_or_else(|| PathBuf::from(r"C:\")).join(BASE_FOLDER)
}

/// 경로가 온전한가. 윈도우에서 `D:\...` · `\\서버\공유` 만 통과시킨다.
pub fn is_absolute_windows(p: &str) -> bool {
    let t = p.trim();
    if t.len() >= 2 && t.starts_with("\\\\") {
        return true; // 네트워크 공유
    }
    let b = t.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// 폴더를 쓸 수 있게 해 둔다. 성공하면 정리된 절대 경로를 돌려준다.
pub fn ensure(path: &str) -> Result<String, String> {
    let t = path.trim();
    if t.is_empty() {
        return Err("저장 폴더가 비어 있어요. [바꾸기] 로 폴더를 골라 주세요.".into());
    }
    if !is_absolute_windows(t) {
        return Err(format!(
            "저장 위치가 온전한 경로가 아니에요 — \"{t}\".\n[바꾸기] 로 폴더를 다시 골라 주세요."
        ));
    }
    let dir = PathBuf::from(t);
    std::fs::create_dir_all(&dir).map_err(|e| format!("저장 폴더를 만들지 못했어요 — {t}\n({e})"))?;
    probe_write(&dir)?;
    Ok(dir.to_string_lossy().to_string())
}

/// 진짜 쓸 수 있는지 작은 파일 하나로 확인하고 지운다. 권한·읽기 전용 매체를 여기서 잡는다.
fn probe_write(dir: &Path) -> Result<(), String> {
    let probe = dir.join(format!(".komttukttak-쓰기시험-{}", std::process::id()));
    std::fs::write(&probe, b"kom").map_err(|e| {
        format!("그 폴더에 파일을 쓸 수 없어요 — {}\n({e})\n다른 폴더를 골라 주세요.", dir.to_string_lossy())
    })?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_dir_is_a_real_absolute_path() {
        let d = documents_dir().expect("내 문서 폴더를 찾아야 한다");
        let s = d.to_string_lossy().to_string();
        assert!(is_absolute_windows(&s), "절대 경로여야 한다: {s}");
        assert!(d.exists(), "실제로 있어야 한다: {s}");
    }

    #[test]
    fn downloads_dir_is_a_real_absolute_path() {
        let d = downloads_dir().expect("다운로드 폴더를 찾아야 한다");
        assert!(is_absolute_windows(&d.to_string_lossy()), "{d:?}");
    }

    #[test]
    fn default_base_ends_with_our_folder_name() {
        let b = default_base();
        assert!(b.ends_with(BASE_FOLDER), "{b:?}");
        assert!(is_absolute_windows(&b.to_string_lossy()), "{b:?}");
    }

    /// 이게 바로 GIF 가 안 되던 값이다 — 여기서 막혀야 한다.
    #[test]
    fn the_display_text_that_broke_gif_is_rejected() {
        let bad = r"내 문서\컴뚝딱 이미지\GIF";
        assert!(!is_absolute_windows(bad));
        let e = ensure(bad).unwrap_err();
        assert!(e.contains("온전한 경로가 아니"), "{e}");
        assert!(e.contains("바꾸기"), "왜 안 되는지와 무엇을 하면 되는지가 같이 있어야 한다: {e}");
    }

    #[test]
    fn absolute_paths_pass_and_relative_ones_do_not() {
        assert!(is_absolute_windows(r"D:\사진"));
        assert!(is_absolute_windows(r"C:/사진"));
        assert!(is_absolute_windows(r"\\NAS\공유\사진"));
        assert!(!is_absolute_windows("사진"));
        assert!(!is_absolute_windows(r"..\사진"));
        assert!(!is_absolute_windows(""));
        assert!(ensure("  ").is_err());
    }

    #[test]
    fn ensure_makes_the_folder_and_proves_it_is_writable() {
        let dir = std::env::temp_dir().join(format!("컴뚝딱 시험 {}", std::process::id())).join("GIF");
        let _ = std::fs::remove_dir_all(&dir);
        let got = ensure(&dir.to_string_lossy()).unwrap();
        assert!(PathBuf::from(&got).is_dir(), "{got}");
        // 쓰기 시험 파일은 남으면 안 된다
        let leftovers: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
        assert!(leftovers.is_empty(), "시험 파일이 남았다: {leftovers:?}");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
