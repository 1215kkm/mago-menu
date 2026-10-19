//! 자동 업데이트 — tauri-plugin-updater + GitHub Releases(1215kkm/mago-menu latest.json) + minisign 서명.
//! 컴뚝딱 updater.rs 를 줄인 것.
//!
//! MAGO 는 트레이에서 늘 켜 두는 프로그램이라 "닫을 때 설치" 로는 영영 안 바뀐다.
//! 그래서 켤 때와 6시간마다 확인해 받아 두고, **고리가 떠 있지 않을 때** 조용히(passive) 설치하고 다시 켠다.
//! 서명이 안 맞으면 플러그인이 설치하지 않는다. 다운그레이드는 `version_comparator` 로 막는다.
//! 로그: `%APPDATA%\com.mago.app\logs\app.log` 의 `[업데이트]` 줄.
//!
//! 숨은 메뉴 글은 이것과 따로 간다 (content.rs — 프로그램을 다시 깔지 않아도 바뀐다).

use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = check(&app).await {
                crate::log(&format!("[업데이트] 건너뜀: {e}"));
            }
            let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_secs(6 * 3600))).await;
        }
    });
}

async fn check(app: &AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let updater = app.updater_builder().version_comparator(|current, remote| remote.version > current).build()?;
    let Some(update) = updater.check().await? else {
        crate::log(&format!("[업데이트] 최신 — {}", app.package_info().version));
        return Ok(());
    };
    crate::log(&format!("[업데이트] 새 판 {} (지금 {}) 받는 중", update.version, update.current_version));
    let bytes = update.download(|_, _| {}, || {}).await?;
    // 고리를 쓰는 중이면 끝날 때까지 기다린다 (끌던 파일을 놓기 직전에 꺼지면 안 된다)
    while crate::ring::in_use() {
        let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_secs(5))).await;
    }
    crate::log(&format!("[업데이트] {} 설치 — 끝나면 다시 켜짐", update.version));
    // 성공하면 돌아오지 않는다 (설치 프로그램을 띄우고 이 프로세스를 끝낸다)
    update.install(bytes)?;
    Ok(())
}
