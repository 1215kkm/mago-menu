//! 한 번만 실행 — 두 번째로 켜면 떠 있는 쪽의 설정 창을 앞으로 부르고 끝낸다 (컴뚝딱 startup/single.rs 를 줄인 것).

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW};

const MUTEX_NAME: &str = "Local\\Mago.SingleInstance";
pub const WINDOW_TITLE: &str = "MAGO";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `true` = 내가 첫 번째(계속). `false` = 이미 떠 있다(끝내야 한다). 핸들은 프로세스가 끝날 때 OS 가 닫는다.
pub fn acquire() -> bool {
    let name = wide(MUTEX_NAME);
    let h = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
    if h.is_null() {
        return true;
    }
    unsafe { GetLastError() != ERROR_ALREADY_EXISTS }
}

/// 떠 있는 쪽의 설정 창을 앞으로 (숨어 있어도 보이게)
pub fn focus_existing() {
    let title = wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
    }
}
