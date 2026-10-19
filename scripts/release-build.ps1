# release-build.ps1 — MAGO 서명된 설치 파일 만들기 (자동 업데이트용 .sig 포함).
# 실행: powershell -ExecutionPolicy Bypass -File scripts\release-build.ps1
#
# 서명키와 비밀번호는 %USERPROFILE%\.mago-keys\ 에서 이 창 안에서만 읽어 쓴다 — 화면·로그에 찍지 않는다.
# 컴뚝딱 키(.komttukttak-keys)와 따로다. 이 폴더를 잃으면 이미 깐 사람에게 업데이트를 더 보낼 수 없다 (다른 곳에 백업해 둘 것).
$ErrorActionPreference = 'Stop'
$keys = Join-Path $HOME '.mago-keys'
$keyFile = Join-Path $keys 'mago.key'
$pwFile = Join-Path $keys 'PASSWORD.txt'
if (-not (Test-Path $keyFile)) { Write-Host "서명키가 없어요: $keyFile"; exit 1 }
if (-not (Test-Path $pwFile)) { Write-Host "비밀번호 파일이 없어요: $pwFile"; exit 1 }
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw $keyFile
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content -Raw $pwFile).Trim()
Set-Location (Split-Path -Parent $PSScriptRoot)
# 실행 중인 exe 는 링크를 깨뜨린다
try { taskkill /F /T /IM mago.exe 2>&1 | Out-Null } catch {}
# --features custom-protocol: 화면(ui/)을 exe 에 넣는다 (cargo build --release 단독은 Cargo.toml default 로 켜져 있지만 CLI 와 맞춘다)
npx --yes '@tauri-apps/cli@2' build --features custom-protocol
$code = $LASTEXITCODE
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
if ($code -eq 0) { Write-Host "`n설치 파일 완성 — src-tauri\target\release\bundle\nsis\" } else { Write-Host "`n빌드 실패 (코드 $code)" }
exit $code
