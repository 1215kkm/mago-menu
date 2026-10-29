#!/usr/bin/env bash
# 대표님이 직접 돌리는 올리기 — `bash scripts/publish.sh 0.1.8`
# release/v<판>/ (release-build.ps1 → make-latest.py 로 만든 것) 을 GitHub 릴리스로 올리고, 자동 업데이트가 보는 latest.json 을 다시 읽어 확인한다.
set -euo pipefail
V="${1:?판 번호를 주세요 (예: 0.1.8)}"
cd "$(dirname "$0")/.."
D="release/v$V"
NOTES="$(python -c "import json,sys; print(json.load(open(sys.argv[1], encoding='utf-8'))['notes'])" "$D/latest.json")"
git push origin main
gh release create "v$V" "$D/mago-setup.exe" "$D/mago-setup.exe.sig" "$D/latest.json" -R 1215kkm/mago-menu -t "MAGO $V" -n "$NOTES"
got="$(curl -sL https://github.com/1215kkm/mago-menu/releases/latest/download/latest.json | python -c "import json,sys; print(json.load(sys.stdin)['version'])")"
if [ "$got" = "$V" ]; then echo "올라감 — 자동 업데이트가 보는 판: $got"; else echo "확인 필요 — 자동 업데이트가 보는 판이 $got 예요" >&2; exit 1; fi
