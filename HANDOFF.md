# 인수인계 — 시프트 메뉴 (2026-10-20)

## 어디서 왔나
- 컴뚝딱 v0.3.45 ~ v0.3.51 에서 만든 '끌어서 바로 바꾸기'(탐색기에서 Shift 끌기 → 원형 메뉴 → 원본 옆에 형식 바꾸기)를 그대로 떼어 왔다.
- 대표님 결정: 컴뚝딱과 **별개의 프로그램으로 따로 배포**한다. 컴뚝딱에서는 이 기능을 뺀다 (v0.3.52).
- 대표님이 컴뚝딱 판으로 실제 탐색기에서 끌어 써 봤다 — 2026-10-19 23:04·23:05 에 png → webp 가 원본 옆에 만들어진 기록이 컴뚝딱 로그에 있다.

## 지금 되는 것 (v0.1.0)
- 트레이에서 기다리는 프로그램. 처음 켜면 설정 창이 뜨고, 그 뒤로는 트레이 아이콘을 누르면 뜬다. 창 X = 트레이로 숨기기, 트레이 메뉴·설정 창 [프로그램 끝내기] = 끝내기.
- 한 번만 실행 (두 번째로 켜면 떠 있는 쪽 설정 창을 앞으로).
- 탐색기에서 **Shift 를 누르고 있는 동안** 원형 메뉴가 뜬다. 끄는 도중에 눌러도 되고, 떼면 0.25초 뒤 닫힌다(놓을 때 Shift 를 살짝 먼저 떼도 되게). Esc · 가운데 놓기 = 취소.
- 칸: 그림(png·jpg·webp·bmp·gif 끼리) · 소리(wav ↔ mp3) · PDF(jpg·png 쪽마다 / 한 장으로). 끈 파일 종류에 맞는 칸만, 이미 그 형식이면 그 칸은 뺀다.
- 설정: 켬/끔(기본 켬) · 누를 키(Shift 기본 · Ctrl · Alt) · 종류별로 보일 칸 · 윈도우 켤 때 트레이에 같이 켜기(HKCU Run, 값 이름 "시프트 메뉴", 기본 끔) · 원형 메뉴 미리 보기.
- 설정 파일: `%APPDATA%\com.shiftmenu.app\prefs.json` 의 `quickConvert`. 로그: `%APPDATA%\com.shiftmenu.app\logs\app.log` (`[시프트 메뉴]` 줄 — 메뉴를 띄운 이유·바꾼 결과가 남는다).

## 파일 지도
| 파일 | 하는 일 |
|---|---|
| `src-tauri/src/lib.rs` | 트레이·설정 창·한 번만 실행·로그 |
| `src-tauri/src/ring.rs` | **핵심.** 끌기+키 감지(watch) · 원형 메뉴 창 띄우기 · 놓기 받는 둥근 창(drop_win, OLE) · 칸 판정(hit) · 바꾸기 실행 · 설정 |
| `src-tauri/src/convert/` | 바꾸기 엔진 (컴뚝딱 imagery 에서 그대로): ext_convert · convert · pdf_render(Windows 내장 PDF) |
| `src-tauri/src/outdir.rs` · `prefs.rs` · `run_key.rs` · `single.rs` | 저장 폴더 · 설정 저장 · 시작프로그램 등록 · 한 번만 실행 |
| `ui/index.html` · `settings.js` · `styles/settings.css` | 설정 창 |
| `ui/qconv.html` · `qconv.js` · `styles/qconv.css` | 원형 메뉴 그림 (마우스가 통과하는 창 — 그림만 그린다. 어느 칸 위인지는 Rust 가 알려 준다) |

## 끌기 감지 방법 (ring.rs watch)
왼쪽 버튼을 누른 채 8px 넘게 움직였고 + 키가 눌려 있고 + (셸 끌기 그림 창 `SysDragImage` 가 떠 있음 **또는** 탐색기·바탕화면에서 시작했고 영역 선택(올가미)이 아님).
올가미 판별: 그 창 스레드에서 마우스를 잡은 창이 `DirectUIHWND`·`SysListView32` 면 올가미. 로그에 "끌기 그림=… 마우스 잡은 창=…" 이 남는다.

## 확인한 것
- `cargo test --lib` 40개 통과 — 탐색기와 같은 셸 끌기 데이터(SHCreateDataObject)를 놓기 처리기에 넣어 "칸 위 놓기 → 복사로 답함 · 원본 옆 webp 생김 · 원본 그대로", "가운데 놓기 → 아무것도 안 만듦".
- release exe 를 임시 설정 폴더로 띄워: 첫 실행에 설정 창이 뜸(켬 · Shift), [원형 메뉴 미리 보기] 가 그려짐, [프로그램 끝내기] 로 끝남.

## 확인 못 한 것
- 이 프로그램으로 **실제 탐색기 끌기** (컴뚝딱 판으로는 대표님이 써 봄).
- 트레이 아이콘 왼쪽 클릭·메뉴, 윈도우 켤 때 같이 켜기 실제 재부팅.
- 설치 파일(NSIS) — 아직 안 만들었다.

## 다음에 할 일 (대표님 확인 필요한 것은 ★)
1. ★ **이름 · 아이콘** — 지금은 임시 이름 "시프트 메뉴", 아이콘은 컴뚝딱 것을 빌려 씀.
2. ★ **배포 방법** — GitHub 저장소(새로)·설치 파일·자동 업데이트. 업데이트 서명 키는 컴뚝딱과 **따로** 만든다(키 파일은 대표님 PC 의 `~/.komttukttak-keys` 같은 자리에, 절대 출력·커밋 금지). 설치 파일을 만들려면 `@tauri-apps/cli`(npx tauri build) 를 붙인다.
3. ★ **메뉴에 넣을 기능** — 컴뚝딱 세션에서 대표님께 드린 제안 (아직 안 고르심):
   - ① 다듬기: 용량 줄이기(카톡·메일용 1MB 이하) · 크기 맞추기(인스타·유튜브·스마트스토어) · 누끼(AI) · 위치 정보(GPS) 지우기
   - ② 묶기(여러 개 끌 때만): 여러 장 → 한 장(이어붙이기·콜라주) · GIF · PDF 한 권 · ZIP
   - ③ 섞기(연금술): 서로 다른 파일 둘을 같이 끌면 조합 결과 (사진+사진=비교, 사진+로고=워터마크, 사진+txt=밈) + **조합 도감**("새 조합 발견! 3/12")
   - ④ 놀기: 사진 퍼즐(9조각) · 오늘의 파일 운세 · 픽셀아트 · 파일 무게 재기
   - 칸이 많아지면 **두 겹 고리**(안쪽 묶음 · 바깥쪽 기능) 제안.
   - 컴뚝딱(D:\computer)에 이미 있는 코드: 크기 맞추기 `imagery/resize.rs` · 이어붙이기 `stitch.rs` · GIF `gif_slot.rs` · AI 누끼 `ai_cutout.rs`(모델 185MB 처음 쓸 때 받기) · 워터마크 `watermark.rs` · 용량 맞추기 `convert::save_within`.
4. 고리 칸 판정·그림 배치는 `ring.rs hit()` 와 `qconv.js` 가 같아야 한다 (12시에서 시계 방향, 반지름 62~178).

## 컴뚝딱에서 겪은 함정 (다시 밟지 말 것)
- **wry 놓기 처리기는 Drop 때 DROPEFFECT 를 정하지 않는다** → Shift 끌기에서 원본이 지워질 수 있다. 그래서 놓기 창을 직접 만들었다.
- Tauri `show()` 는 창을 활성화할 수 있다 → 메뉴는 `ShowWindow(SW_SHOWNOACTIVATE)` 로 띄운다(끌던 탐색기 포커스 유지).
- 메뉴 위에서 칸을 옮길 때마다 통째로 다시 그리면 깜빡인다 → 한 번만 그리고 칸 색·가운데 글만 고친다 (대표님 지적).
- 테스트 exe 는 매니페스트가 없어 comctl32 v6 함수에서 안 뜬다 → build.rs 가 debug 에서 comctl32 지연 로드.
- `cargo` 가 옛 경로(`C:\Users\…\computer`)를 가리키는 tauri 빌드 캐시로 실패하면 `cargo clean -p tauri` (release 는 `--release`).
- 시험용 exe 를 띄울 때는 `APPDATA`·`LOCALAPPDATA` 를 임시 폴더로 — 대표님 설정을 건드리지 않게. 예 (PowerShell):
  ```
  $psi = New-Object System.Diagnostics.ProcessStartInfo "D:\shift-menu\src-tauri\target\release\shift-menu.exe"
  $psi.UseShellExecute = $false
  $psi.EnvironmentVariables["APPDATA"] = "<임시>\roaming"; $psi.EnvironmentVariables["LOCALAPPDATA"] = "<임시>\local"
  [System.Diagnostics.Process]::Start($psi)
  ```
  화면 버튼은 UI Automation(InvokePattern)으로 누르면 마우스를 안 움직인다.
