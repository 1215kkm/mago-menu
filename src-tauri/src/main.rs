// 창 없는 콘솔이 같이 뜨지 않게 (release)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shift_menu_lib::run()
}
