# 인수인계 — MAGO (2026-10-20)

## 어디서 왔나
- 컴뚝딱 v0.3.45 ~ v0.3.51 에서 만든 '끌어서 바로 바꾸기'(탐색기에서 Shift 끌기 → 원형 메뉴 → 원본 옆에 형식 바꾸기)를 떼어 왔다.
- 대표님 결정: 컴뚝딱과 **별개의 프로그램으로 따로 배포**한다. 컴뚝딱에서는 이 기능을 뺐다 (v0.3.52). 이름은 **MAGO** (10-20).
- 대표님이 컴뚝딱 판으로 실제 탐색기에서 끌어 써 봤다 — 2026-10-19 23:04·23:05 에 png → webp 가 원본 옆에 만들어진 기록이 컴뚝딱 로그에 있다.

## 지금 되는 것 (v0.1.0)
- 트레이에서 기다리는 프로그램. 처음 켜면 설정 창이 뜨고, 그 뒤로는 트레이 아이콘을 누르면 뜬다. 창 X = 트레이로 숨기기, 트레이 메뉴·설정 창 [프로그램 끝내기] = 끝내기.
- 한 번만 실행 (두 번째로 켜면 떠 있는 쪽 설정 창을 앞으로).
- 탐색기에서 **Shift 를 누르고 있는 동안** 원형 메뉴가 뜬다. 끄는 도중에 눌러도 되고, 떼면 0.25초 뒤 닫힌다. Esc · 가운데 놓기 = 취소.
- 칸 (파일 종류마다 다르다):
  - 그림: PNG · JPG · WEBP · **정사각(SNS 1080×1080, 가운데를 채워 자름 → `이름_정사각.jpg`)** · **1MB 이하 JPG(카톡·메일 → `이름_1MB.jpg`)** — BMP · GIF 는 설정에서 켠다.
  - 소리: wav ↔ mp3. PDF: jpg · png (쪽마다 / 한 장으로).
- **쓰레기통 캐릭터** (고리 오른쪽 아래, 가운데에서 (162,162) px):
  - 커서가 멀면 숨쉬기(idle) → 가까워지면 입 벌림(wait) → 더 가까우면 별눈(wait_heart) → 위에 오면 덥석(feed_near). 커서 쪽으로 ±7° 기운다. 4.5초 가만히 있으면 잔다(sleep + zzz).
  - 놓으면: 끌던 파일이 음식이 되어 입으로 날아감 → 먹기·씹기 번갈아(눌렸다 튀어 오름) → 만족(하트) + 말풍선 "냠! N개 먹었어요". 파일은 **윈도우 휴지통**으로 (되살릴 수 있다).
  - 숨은 메뉴 조건을 채우면: 배부름 → 끙(부들부들) → 응가 → 뚜껑이 빛나며 + 배지 → 고리가 닫히면 **쪽지 방**(화면이 어두워지고 작은 불빛 아래 쪽지, [자세히 보기] = 빨려 들어가며 링크 열기).
- **숨은 메뉴 수**: 고리 가운데 맨 아래에 작게 "숨은 메뉴 N개"(다 찾으면 "숨은 메뉴 다 찾음"). 설정 창 「숨은 메뉴」 카드에 찾은 것 목록 · [다시 보기] · 먹인 횟수.
- **관리자 내용 업데이트**: `content/content.json` 을 고쳐 GitHub main 에 올리면, 켜져 있는 MAGO 가 켤 때와 6시간마다 받아 간다(`ui/content-sync.js` → Rust `content_set`). `version` 을 올려야 바뀐 것으로 본다. 자세히는 `content/README.md`.
- **프로그램 자동 업데이트**: GitHub 릴리스의 latest.json (서명 확인) — 켤 때와 6시간마다 확인, 받아 두었다가 **고리를 안 쓰는 때** 조용히 설치하고 다시 켠다 (`updater.rs`).
- 설정 파일: `%APPDATA%\com.mago.app\prefs.json` (`quickConvert` 고리 설정 · `mago` 먹인 횟수·찾은 것 · `contentCache` 받은 내용). 로그: `%APPDATA%\com.mago.app\logs\app.log` (`[MAGO]` · `[숨은 메뉴]` · `[내용]` · `[업데이트]`).

## 파일 지도
| 파일 | 하는 일 |
|---|---|
| `src-tauri/src/lib.rs` | 트레이·설정 창·한 번만 실행·로그·플러그인 |
| `src-tauri/src/ring.rs` | **핵심.** 끌기+키 감지(watch) · 고리 창 · 놓기 받는 둥근 창(drop_win, OLE — 고리 원 + 쓰레기통 원) · 칸/쓰레기통 판정 · 바꾸기 · 먹이기(feed) |
| `src-tauri/src/actions.rs` | 정사각 · 1MB · 휴지통(SHFileOperation, 되살리기 가능) |
| `src-tauri/src/mix.rs` | 여러 종류 묶음 — PDF 하나로(lopdf 로 잇기) · 파일마다 PDF · JPG 한 장으로 · 쪽마다 JPG |
| `src-tauri/src/office.rs` | Word · 엑셀 · 한글 → PDF (이 PC 의 그 프로그램을 숨겨서 COM 으로). 없으면 docx · xlsx · hwpx 는 글만 꺼내서 |
| `src-tauri/src/content.rs` | 숨은 메뉴 내용(번들 사본 · 받은 것 · 검사) · 찾은 것 · 쪽지 방 창 · 링크 열기(내용 파일의 https 만) |
| `src-tauri/src/updater.rs` | 자동 업데이트 |
| `src-tauri/src/web.rs` | 브라우저에서 끈 그림 · 링크 — 다운로드 폴더에 받기 · 바로가기 · 카톡(클립보드 + 카톡 열기) |
| `src-tauri/src/convert/` | 바꾸기 엔진 (컴뚝딱에서 그대로): ext_convert · convert · pdf_render |
| `ui/qconv.*` · `styles/qconv.css` | 고리 + 쓰레기통 그림 (마우스가 통과하는 창 — 어느 칸·쓰레기통 위인지는 Rust 가 알려 준다) |
| `ui/secret.*` · `styles/secret.css` | 쪽지 방 |
| `ui/index.html` · `settings.js` · `content-sync.js` | 설정 창 · 내용 받기 |
| `ui/mago/*.webp` | 캐릭터 그림 23장 (대표님이 준 시트를 AI 누끼로 따서 높이 240·같은 바닥선으로 맞춤) |
| `content/content.json` | 관리자가 고치는 숨은 메뉴 |
| `scripts/release-build.ps1` · `make-latest.py` | 배포 |

## 배포
1. `src-tauri/tauri.conf.json` 의 `version` 올리기 (업데이트는 tauri.conf 판을 본다)
2. `powershell -ExecutionPolicy Bypass -File scripts\release-build.ps1` — 서명키 `%USERPROFILE%\.mago-keys\`(mago.key · PASSWORD.txt)를 이 창 안에서만 쓴다. **출력·커밋 금지, 다른 곳에 백업.**
3. `python scripts/make-latest.py 0.1.1 "바뀐 점"` → `release/v0.1.1/` 에 mago-setup.exe · .sig · latest.json
4. `gh release create v0.1.1 release/v0.1.1/mago-setup.exe release/v0.1.1/mago-setup.exe.sig release/v0.1.1/latest.json -R 1215kkm/mago-menu -t "MAGO 0.1.1" -n "바뀐 점"`
5. `https://github.com/1215kkm/mago-menu/releases/latest/download/latest.json` 이 새 판인지 확인

## 확인한 것
- `cargo test --lib` 46개 통과 — 탐색기와 같은 셸 끌기 데이터로 "칸 위 놓기 → 복사로 답함 · 원본 옆 webp · 원본 그대로", "가운데 → 아무것도 안 만듦", "쓰레기통 자리 → 바꾸지 않고 먹이러 감". 정사각(1600×900 → 900×900, 가운데) · 1MB(잡음 3000×2000 → 1MB 이하) · 숨은 메뉴 여는 조건 · 나쁜 내용 파일 거부.
- 고리·쓰레기통 9가지 상태와 쪽지 방, 설정 창을 가짜 데이터로 브라우저에 띄워 화면 확인.

## 확인 못 한 것 (대표님 손이 필요)
- **실제 탐색기 끌기**로 쓰레기통에 먹이기 · 쪽지 방 · [자세히 보기].
- 트레이 아이콘, 윈도우 켤 때 같이 켜기 실제 재부팅, 설치 파일로 설치 → 다음 판 자동 업데이트.

## 캐릭터 그림 — 더 부드럽게 하려면 (대표님께 드린 목록)
지금은 시트의 15자세를 겹쳐 넘기기(0.09초)로 잇는다. 아래를 같은 캐릭터·같은 크기(투명 PNG, 높이 240 안팎, 발 위치 같게)로 받으면 끊김이 줄어든다:
`blink_1~2`(눈 깜빡) · `mouth_open_1~3`(idle → wait 사이) · `chew_1~4`(씹기 한 바퀴) · `drool_1~3`(침 방울) · `poop_1~4`(끙 → 응가) · `lid_open_1~3`(뚜껑 열림) · `fx_zzz`·`icon_plus`(배경 없는 것 — 지금은 CSS 로 그림) · `feed_near` 손 없는 판.

## ✔ 0.1.1 에 넣음 (10-20) — 윈도우 켤 때 같이 켜기 **기본 켬** (남은 것 1~3 끝: 시험 추가 · `src-tauri/windows/hooks.nsh` 지우기 훅 · run_key 글)
대표님: "윈도우 켤 때 같이 켜기 기본 켬으로 해줘".
- 해 둔 것 (커밋 전): `src-tauri/src/lib.rs` 에 `apply_autostart_default()` — prefs `autostartDefaulted` 가 없으면 한 번 `run_key::enable()` 하고 표시. 사용자가 `autostart_set` 으로 고르면 표시를 남겨 다시 덮지 않음. cargo `target\release|debug` 의 시험용 exe 는 등록 안 함(`is_dev_build`). 0.1.0 사용자도 0.1.1 로 바뀌며 한 번 켜짐.
- 남은 것:
  1. `is_dev_build` 단위 시험 추가, `cargo test --lib` 통과 확인.
  2. 지우기(언인스톨) 때 `HKCU\...\Run` 의 `MAGO` 값도 지우기 — NSIS 훅(`bundle.windows.nsis.installerHooks`, `NSIS_HOOK_POSTUNINSTALL` 에서 `DeleteRegValue`, 업데이트 중(`$UpdateMode = 1`)에는 지우지 않게).
  3. `run_key.rs` 의 남은 '컴뚝딱' 글(주석·오류 문구 "컴뚝딱 실행 파일 경로…")을 MAGO 로.
  4. README · 이 문서 정리, version 0.1.1 → 「배포」 순서대로 빌드·릴리스 → latest.json 확인.
  5. 대표님께 한국어로 보고 (확인한 것 / 못 한 것).

## ✔ 0.1.1 — 10-20 대표님 요청 3가지
- **쓰레기통이 왼쪽(고리 쪽)을 본다** — 그림은 그대로, `styles/qconv.css` `.mago .pose` 에 `scaleX(-1)`. 음식이 날아가는 입 자리도 왼쪽으로 (`qconv.js` throwFood).
- **먹은 용량** — 말풍선 "냠! 4개 · 12.5MB 먹었어요 / 지금까지 19개 · 45.1MB · 휴지통에 있어요". prefs `mago.files`·`mago.bytes` 로 쌓는다(`content.rs on_feed`). 설정 창 숨은 메뉴 카드에도.
  "4개를 넣어도 1개" — 탐색기와 같은 셸 끌기 데이터 4개로 시험해 보니 놓기 창은 4개를 다 받는다(`a_shell_drag_of_four_files_hands_over_all_four`). 한 번에 하나씩 네 번 넣은 것일 수도 있어 쌓인 수를 같이 보인다. **대표님 PC 에서 4개 한꺼번에 먹여 확인 필요.**
- **움직이는 GIF** (`src-tauri/src/anim.rs`) — GIF 칸을 기본으로 켬(설정 판 4, 옛 설정엔 한 번 더함). 여러 장 → 이름 순(숫자는 숫자로)으로 이어 하나 `첫이름_움직임.gif`. 한 장 → 칸 사이가 빈 줄(투명·바탕색)이거나 가로가 세로의 정확한 배수면 칸을 나눠 움직임, 아니면 예전처럼 보통 GIF. 긴 변 800px, 300장까지, 빠르기는 설정 「GIF 한 장」 0.06/0.1/0.2/0.5초.

## ✔ 0.1.2 — 10-20 대표님 "고양이 케릭터 추가해서 기본으로 해주고 설정에서 선택할수 있게"
- 대표님이 준 검은 고양이 시트(쓰레기통과 같은 15자세, 같은 방향)를 `scripts/cut-character.py` 로 잘라 `ui/mago/cat/*.webp` (rembg 누끼 → 귀 안쪽 구멍 메움 → 같은 배율·같은 바닥선, 높이 240). 음식 · 하트 · 반짝이는 쓰레기통 것을 같이 쓴다.
- 설정 `character` (`cat` 기본 · `trash`) — 칸이 없는 옛 설정도 고양이로 읽힘. 설정 창 「캐릭터」 줄(그림 붙은 두 칸). 고리는 `qconv:view.character` 를 받아 숨어 있을 때만 바꾼다(먹는 도중 안 바뀜). 입 자리는 캐릭터마다(`qconv.js` CHARS.mouth).
- `content/content.json` 의 쪽지 글은 아직 "쓰레기통"이라 부른다 — 관리자 내용이라 그대로 둠.

## ✔ 0.1.4 — 10-20 대표님 "txt 잡고 끌면 pdf, 한글, word, 한영일중 번역, 엑셀 … 누끼버튼 … gif 설명"
- **글(TXT·TSV) 칸** (`src-tauri/src/text.rs`): PDF · 한글(HWPX) · Word(DOCX) · 엑셀(XLSX, 줄마다 행 · 탭마다 칸, 숫자는 숫자 · 앞자리 0 은 글). UTF-8 / UTF-16 / 메모장 ANSI(CP949) 읽음. 20MB 까지.
  - 한글: python-hwpx(Apache-2.0)의 빈 문서 틀 `src-tauri/assets/hwpx/` 첫 문단 자리부터 줄마다 문단. 탭은 빈칸 넷.
  - PDF: `pdfprint.rs` — 숨은 WebView2 창에 이스케이프한 HTML(임시 파일)을 띄우고 PrintToPdf (A4, 맑은 고딕 등 쓴 글자만 들어감).
- **누끼 칸** (`cutout.rs`): Windows ML 로 이 PC 에서 IS-Net(isnet-general-use). 처음 한 번 179MB 를 GitHub(rembg 릴리스)에서 WinHTTP 로 받아 `%LOCALAPPDATA%\com.mago.app\models\` · SHA-256 확인. 몸 안에 갇힌 작은 구멍(눈 흰자 · 귀 안)은 채움(그림의 2% 미만). → `이름_누끼.png`. 설정 판 5 — 옛 설정에 누끼를 한 번 더함.
- **칸 설명 말풍선**: `Seg.tip` — 마우스가 올라간 칸에 긴 설명이 있으면 캐릭터 왼쪽 아래(고리 아래 빈자리)에 말풍선. GIF: "같은 크기 이미지 여러 장, 또는 일정한 간격의 캐릭터들이 있는 이미지가 필요해요".
- 시험용 exe 만 `mago.exe --시험바꾸기 <칸> <파일>` — 끌기 없이 그 칸 일을 한 번 하고 로그에 남기고 끝남 (PDF · 누끼를 실제 프로그램 안에서 확인하는 데 씀. 설치한 exe 는 무시).
- **번역(한·영·일·중)은 아직** — 글을 PC 밖 번역 서비스로 보내야 해서(규칙: 파일은 PC 밖으로 안 보냄) 대표님 결정 대기.
- 확인: Word·엑셀은 이 PC 의 Office 2010 으로 숨겨 열어 내용 확인. HWPX 는 python-hwpx 로 다시 읽어 확인 — **한글 프로그램으로 여는 건 대표님 확인 필요**(이 PC 한글 2010 은 HWPX 를 못 열 수 있음).

## 0.1.5 — 10-24 대표님 "jpg, txt, pdf, word, 한글파일, xlsx 를 모두 잡고 시프트드래그하면 모두 한장의 pdf, jpg 로 저장할건지, 쪽마다 저장할건지 … 순서는 선택한 파일순서대로 … 선택한 파일 목록이 번호랑 같이 보이게"
- **묶음 칸** (`mix.rs`, 설정 `mix` — 칸이 없는 옛 설정은 넷 다): 종류가 섞였거나 Word·엑셀·한글이면 고리에 `PDF 하나로` · `PDF 파일마다` · `JPG 한 장으로` · `JPG 쪽마다`. 소리가 섞이면 "소리는 소리끼리". 같은 종류끼리는 예전 칸 그대로.
  - PDF 하나로 → 1번 파일 옆 `1번이름 외 N개.pdf`. PDF 는 쪽을 그대로 잇고(물려받는 MediaBox·Resources 는 쪽에 적어 둠), 그림은 A4(모양 따라 가로·세로)에 여백 1cm · 긴 변 2400px. 잇지 못하는 PDF 는 쪽을 그림으로 그려 넣음.
  - JPG 한 장으로 → `1번이름 외 N개.jpg` (PDF 쪽 폭에 맞춤, 그림은 줄이기만, 높이 60000px·1억 화소 넘으면 전체를 줄임). 쪽마다 → 새 폴더 `1번이름 외 N개` 에 `01_이름.jpg` · `02_보고서_1쪽.jpg` ….
  - 파일마다 PDF → 각 원본 옆 `이름.pdf` (이미 PDF 는 그대로).
  - 한 파일이 실패해도 나머지로 만든다. 실패 이유는 캐릭터 말풍선("3번: …").
- **문서 → PDF** (`office.rs`): Word(`Word.Application`) · 엑셀(`Excel.Application`) · 한글(`HWPFrame.HwpObject`)을 화면에 안 띄우고 COM(IDispatch)으로 열어 PDF 로 저장. 읽기 전용 · 매크로 강제 끔 · 암호 문서는 엉터리 암호로 실패시킴 · 이미 켜 둔 Word·엑셀은 설정을 되돌리고 다른 문서가 열려 있으면 안 끔 · 3분 넘으면 포기.
  - 함정 1: Word 2010 은 15자 넘는 암호를 넘기면 암호 없는 문서도 "명령을 정상적으로 종료할 수 없습니다" 로 못 연다 → 9자 `mago-nopw`.
  - 함정 2: 빈 글 BSTR 이 null 이면 한글 SaveAs 가 "서버에서 예외 오류" → 빈 글은 `SysAllocString("")`.
  - 한글 2010(이 PC)은 .hwpx 를 못 연다(`Open(…, "HWPX")` 가 false) → 글만 꺼내 PDF.
  - 프로그램이 없으면 docx(word/document.xml) · xlsx(시트 순서·공유 글자) · hwpx(Contents/section*.xml)에서 글만 꺼내 PDF(엑셀은 표로). .doc · .xls · .hwp 는 그 프로그램이 있어야 함.
- **번호 목록** (`qconv.js` drawList · `Plan.files`): 묶음일 때만 고리 위 빈자리에 `놓으면 이 순서대로 · N개` + 번호·이름(확장자는 늘 보임). 다섯 개 넘으면 두 줄, 열 개 넘으면 9개 + "외 N개". 바꾸는 동안 지금 파일에 색, 마친 번호는 초록.
- **순서** (`ring.rs picked_order`): 탐색기의 끌기 데이터는 **잡은 파일부터, 앞서 고른 것을 거꾸로 돈다** — C→A→D 로 고르고 D 를 잡으면 [D, A, C], C 를 잡으면 [C, D, A], A 를 잡으면 [A, C, D] (`drop_win::tests::explorer_drag_data_order`: 탐색기 창을 셸로 골라 IShellView::GetItemObject(SVGIO_SELECTION) — 끌기 때 쓰는 것과 같은 데이터). 그래서 뒤집는다 → **마지막에 고른 파일을 잡고 끌면 고른 순서 그대로**, 다른 파일을 잡으면 한 칸 돈 순서(목록에 보임). **진짜 마우스로는 대표님 확인 필요.**
- 시험용 exe `--시험바꾸기 mix-pdf <파일1> <파일2> …` — 받은 순서 그대로 (picked_order 를 거치지 않음).

## 0.1.6 (10-27 올림) — 10-25 대표님 "화질 떨어지면 안돼 크기조절할 사람은 파일변형후 완성파일 미리 보여줘 … 이미지들은 현재가 원본크기인걸 표시 … 원본보다 커지면 화질이 떨어질수 있다 알려주고 그대로 저장할건지, 조절부터 할건지 선택 … 원본이 너무 커도 파일폭은 넘어가면 안돼"
- 고리의 **JPG 한 장으로** 는 바로 저장하지 않고 **미리 보기 창**(`sheet.rs` · `ui/sheet.html`)을 연다. 고리 가운데는 "미리 보기 · 창에서 저장해요".
  - 종이 = 저장될 JPG 그대로(그림은 Rust 가 data: 주소로). 칸마다 이름표: 원본 크기(초록) · 파일 폭에 맞춰 원본의 N%(파랑) · 원본의 N% · 흐려질 수 있어요(주황) · 문서 쪽(흰색). 이름표에 마우스를 올리면 걷힌다.
  - 그림 크기 규칙 (`mix::Sheet`): 기본은 **원본 크기**, 파일 폭(PDF 쪽 폭, 그림만이면 가장 넓은 그림·2000px 까지)보다 넓은 그림만 폭에 맞춰 줄임. 사람이 바꾸는 폭은 1px ~ 파일 폭 (`Sheet::clamp`). PDF 쪽은 못 바꿈.
  - [그대로 저장] · [크기 조절](두 칸 스위치) → 그림 아래 양쪽 모서리 동그라미를 끌기 · 두 번 누르면 원본 크기 · [원본 크기] [파일 폭에 맞추기] · [처음대로]. 원본보다 키운 그림이 있으면 저장 전에 [그래도 저장] [다시 조절].
  - 저장 뒤 [폴더에서 보기] [닫기]. 다시 저장하면 같은 파일을 고친다. **✕ 로 닫으면 보이는 그대로 저장**(sheet_set 으로 받아 둔 폭), [저장 안 함] 은 버림. 저장을 기다리는 동안은 업데이트 설치를 미룬다(`in_use`).
- 시험: `--시험바꾸기 mix-jpg-view <파일…>` — 창을 화면 밖(-6000)에 앞으로 안 나오게 띄우고, `MAGO_CDP_PORT=9333` 이면 창 안을 원격 디버깅(CDP)으로 본다(찍기·누르기 — 마우스 안 움직임). 창이 닫히고 저장이 끝나면 exe 가 끝난다. `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 로 포트를 열면 Tauri 의 기본 인자와 부딪혀 바로 죽는다 → 창 빌더의 `additional_browser_args` + 따로 `data_directory`.
- 확인: 실제 exe 로 그림 3·TXT·PDF 5개 → 미리 보기 → 5번(300px)을 207% 로 끌기 → 확인 질문 → 저장 1239×5077 (화면 셈과 같음) · ✕ 닫기 = 보이던 그대로 저장 · [저장 안 함] = 파일 안 생김.

## 0.1.7 (10-28 올림) — 10-28 대표님 "유튜브 영상위에서 영상을 시프트 드래그 하면 다운받는 메뉴도 … 인터넷에서 뭔가를 잡고 시프트 드래그 하면 다운받게 이미지나 영상, 링크파일등등 다운받거나, 카톡으로 보내거나"
- **인터넷에서 끌어 받기** (`web.rs`): 브라우저(크롬 · 엣지 · 웨일 · 파이어폭스)에서 Shift 를 누른 채 끌면 같은 고리.
  - 그림: 저장(받은 그대로) · JPG · PNG(이미 그 형식이면 뺌, avif · svg 는 저장만) · 카톡. 파일 링크(.pdf .zip .mp4 .hwp …): 받기 · 바로가기 · 카톡(파일). 그 밖의 링크 · 유튜브: 바로가기(.url) · 카톡(링크).
  - 받은 것은 **다운로드 폴더**, 같은 이름이면 `(2)`, 인터넷에서 온 표시(Zone.Identifier) 를 붙인다. 받기는 WinHTTP(윈도우 프록시) · 서버가 준 이름(Content-Disposition) · 웹 페이지(text/html)는 "파일이 아니라 웹 페이지예요" 로 거절. 401/403 은 "로그인해야 받을 수 있는 곳".
  - 카톡: 카톡 PC 에 다른 프로그램이 보내는 길이 없다 → 파일(CF_HDROP) 또는 링크 글을 클립보드에 넣고 `KakaoTalk.exe` 를 실행(설치 위치는 지우기 목록 DisplayIcon). 말풍선 "보낼 대화방에서 Ctrl+V".
  - 끌기 데이터: `UniformResourceLocatorW` · `text/x-moz-url` · 주소 하나뿐인 글 · `FileGroupDescriptorW`+`FileContents`(그림 내용 — 놓을 때만 읽음). 크롬이 링크와 같이 주는 "제목.url" 은 제목으로만. 파이어폭스가 임시 폴더에 써 둔 그림 파일 하나 + 주소면 웹으로. 탐색기의 진짜 파일은 늘 파일로.
  - 웹에서 온 것은 쓰레기통이 쉰다(지울 파일이 없다). 파일도 주소도 없는 끌기(글자만)는 고리를 닫는다.
  - 고리 띄우기: 끌기 그림(SysDragImage)이 보이거나, **브라우저에서 시작했고 OLE 끌기 중**(그 스레드의 마우스를 잡은 창 = `CLIPBRDWNDCLASS` — 10-28 DoDragDrop 실험으로 확인). 브라우저에서 글자를 고르며 끄는 동안은 브라우저 창이 잡아서 안 뜬다.
- **유튜브 영상 받기는 안 넣음** — 유튜브 약관이 받기를 막고(공개 배포 프로그램은 신고 대상이 될 수 있음) yt-dlp(약 18MB)와 ffmpeg 를 처음에 받아야 한다. ★ 대표님 결정.
- **PDF 하나로 미리 보기** (10-28 대표님 "모두 진행해"): 고리의 PDF 하나로도 JPG 한 장과 같은 미리 보기 창(`sheet.rs` · `mix::Sheet.pdf`). 칸마다 쪽 하나 — 그림은 A4(모양 따라 가로·세로) 가운데, 기본은 원본 크기(1px = 1pt, 예전 PDF 하나로와 같음), 키우기는 여백 1cm 안까지(`Part.max`). 문서 쪽은 그대로. 원본보다 키우면 [그래도 저장] · [다시 조절].
  - 함정: 창의 CSP 가 `style="…"` 속성을 막는다 — 비율(aspect-ratio)은 `el.style` 로 넣어야 한다 (속성으로 넣은 것은 0.1.6 JPG 한 장에서도 그동안 무시되고 있었다 · 그림 크기로 맞아 보였을 뿐).
  - 확인: 실제 exe(`--시험바꾸기 mix-pdf-view`, 화면 밖 · CDP)로 그림 3 · TXT · PDF → 5쪽 A4 미리 보기 → 1번을 [쪽에 맞추기](원본의 175%) → 확인 질문 → 저장된 PDF 의 그림 폭 539 · 785 · 300pt.
- 확인 못 함: **실제 브라우저에서 끌기**(끌기 데이터 형식 · 고리가 뜨는지) · 카톡 열기 · 카톡에 Ctrl+V.
- 10-29 더함 (0.1.7 이 아직 안 올라가서 같은 판에): 대표님 "저장한후에 폴더가 바로 열리는걸 기본으로 해주고 저장폴더는 설정에서 바꿀수 있게 해줘 설정창에 버전번호가 안나오고 있어" · "피그마나 포토샵으로 바로 열리도록 … 워드같은 다른프로그램에도, 카톡으로 보내기까지"
  - 설정 `webDir`(빈 칸 = 다운로드 폴더 · 상대 경로는 버림 · 못 쓰면 다운로드로) · `webOpen`(기본 켬 — 저장한 뒤 `explorer /select,"…"`) · `web`(보일 칸 — `web::OPTS`). 설정 창에 「인터넷에서 끌어 받기」 카드 · 머리에 판 번호(`app_info`).
  - 새 칸: 포토샵(받은 파일로 실행 — CS6 가 못 여는 webp 등은 PNG 로 바꿔서) · 피그마 · Word(파일로 열 길이 없어 **그림을 클립보드에(PNG + CF_DIB) 담고 실행** → 그 안에서 Ctrl+V) · 다른 프로그램으로(윈도우 「연결 프로그램」 창 `SHOpenWithDialog`, 기본 앱은 안 바꿈). 포토샵 · Word 는 App Paths, 피그마는 `%LOCALAPPDATA%\Figma\Figma.exe` — **깔린 것만 고리에 보이고**, 설정에서는 "이 PC 에 없음".
  - 폴더 고르기는 `IFileOpenDialog`(FOS_PICKFOLDERS) 를 따로 띄운 STA 스레드에서.
  - 시험: `--시험바꾸기 settings-view` = 설정 창을 화면 밖에 3분 띄운다. `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=…` 로 창 안을 본다.

## 다음에 할 일 (★ = 대표님 결정)
1. ★ 숨은 메뉴 더 넣기 — 대표님 메모의 나머지(1분 탈출구 · 타임캡슐 · 파일 조합 해금 · 30초 미니게임 · 소리의 방 · 파일 감별사)를 어떤 순서로.
2. ★ "응가 → 추가 메뉴" 의 추가 메뉴를 쪽지 말고 진짜 기능(예: 미니게임)으로 열지.
3. 메뉴 칸이 많아지면 두 겹 고리.

## 컴뚝딱에서 겪은 함정 (다시 밟지 말 것)
- **wry 놓기 처리기는 Drop 때 DROPEFFECT 를 정하지 않는다** → Shift 끌기에서 원본이 지워질 수 있다. 그래서 놓기 창을 직접 만들었다.
- Tauri `show()` 는 창을 활성화할 수 있다 → 고리는 `ShowWindow(SW_SHOWNOACTIVATE)` 로 띄운다.
- 고리 위에서 칸을 옮길 때마다 통째로 다시 그리면 깜빡인다 → 한 번만 그리고 칸 색·가운데 글만 고친다 (대표님 지적).
- 테스트 exe 는 매니페스트가 없어 comctl32 v6 함수에서 안 뜬다 → build.rs 가 debug 에서 comctl32 지연 로드.
- `cargo` 가 옛 경로를 가리키는 tauri 빌드 캐시로 실패하면 `cargo clean -p tauri` (release 는 `--release`).
- 시험용 exe 를 띄울 때는 `APPDATA`·`LOCALAPPDATA` 를 임시 폴더로 — 대표님 설정을 건드리지 않게. 예 (PowerShell):
  ```
  $psi = New-Object System.Diagnostics.ProcessStartInfo "D:\shift-menu\src-tauri\target\release\mago.exe"
  $psi.UseShellExecute = $false
  $psi.EnvironmentVariables["APPDATA"] = "<임시>\roaming"; $psi.EnvironmentVariables["LOCALAPPDATA"] = "<임시>\local"
  [System.Diagnostics.Process]::Start($psi)
  ```
  화면 버튼은 UI Automation(InvokePattern)으로 누르면 마우스를 안 움직인다.
- 화면을 브라우저로 볼 때 Windows 의 `python -m http.server` 는 .js 를 text/plain 으로 줘서 모듈이 안 뜬다 → 확장자별 형식을 정해 주는 작은 서버를 쓴다.
