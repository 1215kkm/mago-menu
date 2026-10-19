fn main() {
    // 테스트 exe 에는 앱 매니페스트(공용 컨트롤 v6)가 안 들어가 TaskDialogIndirect 를 가져다 쓰는 순간 아예 안 뜬다.
    // debug 빌드(= cargo test · 개발 실행)에서만 comctl32 를 처음 쓸 때 불러오게 한다 (컴뚝딱 build.rs 와 같은 이유).
    let is_release = std::env::var("PROFILE").as_deref() == Ok("release");
    if !is_release && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
    tauri_build::build()
}
