//! 윈도우 켤 때 트레이에 같이 켜기 (컴뚝딱 startup/run_key.rs 에서 가져옴). `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 한 값만 쓴다.
//!
//! **HKCU 만 건드린다.** HKLM(모든 사용자)은 관리자 권한이 필요하고, 남의 계정까지 바꾸는 일이라 쓰지 않는다.
//! 값 이름은 우리 것 하나(`컴뚝딱`)뿐이고, 지울 때도 그 이름만 지운다 — 다른 프로그램의 시작 등록은 읽지도 않는다.
//!
//! 값 내용: `"<exe 경로>" --tray`. 경로에 공백·한글이 있어도 되도록 큰따옴표로 감싼다.

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

pub const RUN_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// 레지스트리에 보일 이름. 사용자가 작업 관리자 [시작프로그램] 에서 이 이름을 본다.
pub const VALUE_NAME: &str = "시프트 메뉴";
/// 트레이 대기 모드로 켜라는 표시. 이 인자로 켜지면 설정 창을 띄우지 않는다.
pub const TRAY_ARG: &str = "--tray";

/// 지금 exe 경로. 못 구하면 None (그러면 등록도 거절한다 — 틀린 경로를 박아 두면 더 나쁘다).
pub fn current_exe() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string())
}

/// 레지스트리에 넣을 명령줄.
pub fn command_line(exe: &str) -> String {
    format!("\"{exe}\" {TRAY_ARG}")
}

/// 우리 값이 있는가. 있으면 그 내용도 같이 (경로가 옛날 것이면 화면에서 다시 등록을 권할 수 있게).
pub fn get() -> Option<String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_PATH, KEY_READ)
        .ok()?
        .get_value::<String, _>(VALUE_NAME)
        .ok()
}

/// 등록됐는가 + 그 경로가 지금 exe 와 같은가.
#[allow(dead_code)] // 컴뚝딱 화면용 (경로가 옛것인지까지 보기)
pub fn status() -> (bool, bool) {
    let Some(v) = get() else { return (false, false) };
    let same = current_exe().map(|e| v == command_line(&e)).unwrap_or(false);
    (true, same)
}

pub fn enable() -> Result<String, String> {
    let exe = current_exe().ok_or("컴뚝딱 실행 파일 경로를 찾지 못했습니다.")?;
    let cmd = command_line(&exe);
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey_with_flags(RUN_PATH, KEY_SET_VALUE)
        .map_err(|e| format!("시작프로그램 등록 자리를 열지 못했습니다: {e}"))?;
    key.set_value(VALUE_NAME, &cmd).map_err(|e| format!("시작프로그램에 등록하지 못했습니다: {e}"))?;
    Ok(cmd)
}

/// 끄기. **없으면 없는 대로 성공**이다 (두 번 눌러도 오류가 안 나야 한다).
pub fn disable() -> Result<bool, String> {
    let key = match RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_PATH, KEY_SET_VALUE) {
        Ok(k) => k,
        Err(_) => return Ok(false), // Run 키 자체가 없는 PC — 지울 것도 없다
    };
    match key.delete_value(VALUE_NAME) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("시작프로그램 등록을 지우지 못했습니다: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_the_path() {
        // 한글·공백이 섞인 경로가 기본값인 PC 가 많다 (C:\Users\강경모\...)
        let c = command_line(r"C:\Users\강경모\내 프로그램\컴뚝딱.exe");
        assert_eq!(c, r#""C:\Users\강경모\내 프로그램\컴뚝딱.exe" --tray"#);
        assert!(c.starts_with('"') && c.contains("\" --tray"));
    }

    /// 진짜 레지스트리 왕복. 우리 값 이름 대신 시험용 이름을 써서 사용자 설정을 건드리지 않는다.
    #[test]
    fn registry_round_trip_with_a_test_value_name() {
        const TEST_NAME: &str = "KOMTEST_컴뚝딱_자동실행";
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey_with_flags(RUN_PATH, KEY_SET_VALUE | KEY_READ).unwrap();
        let _ = key.delete_value(TEST_NAME);
        assert!(key.get_value::<String, _>(TEST_NAME).is_err(), "시작 전엔 없어야 한다");

        let cmd = command_line(r"C:\tmp\komttukttak.exe");
        key.set_value(TEST_NAME, &cmd).unwrap();
        assert_eq!(key.get_value::<String, _>(TEST_NAME).unwrap(), cmd);

        key.delete_value(TEST_NAME).unwrap();
        assert!(key.get_value::<String, _>(TEST_NAME).is_err(), "끄면 흔적이 남지 않아야 한다");
    }

    /// 읽기만 하는 확인 — 이 테스트가 도는 PC 에 우리 값이 있든 없든 통과해야 한다.
    #[test]
    fn status_never_panics() {
        let _ = status();
    }
}
