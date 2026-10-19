# MAGO 릴리스 묶기 — `python scripts/make-latest.py 0.1.0 "바뀐 점 한 줄"`
# 설치 파일 · .sig 를 release/v<판>/ 에 mago-setup.exe 로 모으고 latest.json 을 새로 쓴 뒤 다시 읽어 확인한다.
# 판은 tauri.conf.json 의 version 과 같아야 한다 (자동 업데이트가 이 둘을 비교한다).
import base64, datetime, io, json, os, shutil, sys

V, notes = sys.argv[1], sys.argv[2]
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
conf = json.load(io.open(os.path.join(ROOT, 'src-tauri', 'tauri.conf.json'), encoding='utf-8'))
assert conf['version'] == V, f"tauri.conf.json 판 {conf['version']} ≠ {V}"
pub = conf['plugins']['updater']['pubkey']
want = base64.b64decode(base64.b64decode(pub).decode().splitlines()[1])[2:10][::-1].hex().upper()
nsis = os.path.join(ROOT, 'src-tauri', 'target', 'release', 'bundle', 'nsis')
exe = os.path.join(nsis, f"{conf['productName']}_{V}_x64-setup.exe")
d = os.path.join(ROOT, 'release', f'v{V}')
os.makedirs(d, exist_ok=True)
shutil.copyfile(exe, os.path.join(d, 'mago-setup.exe'))
shutil.copyfile(exe + '.sig', os.path.join(d, 'mago-setup.exe.sig'))
sig = io.open(os.path.join(d, 'mago-setup.exe.sig'), encoding='utf-8').read().strip()
line = base64.b64decode(sig).decode('utf-8', 'replace').splitlines()[1]
kid = base64.b64decode(line)[2:10][::-1].hex().upper()
assert kid == want, f'서명 키가 다르다: {kid} ≠ {want}'
url = f'https://github.com/1215kkm/mago/releases/download/v{V}/mago-setup.exe'
out = {
    'version': V,
    'notes': notes,
    'pub_date': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
    'platforms': {'windows-x86_64': {'signature': sig, 'url': url}},
}
json.dump(out, io.open(os.path.join(d, 'latest.json'), 'w', encoding='utf-8'), ensure_ascii=False, indent=2)
back = json.load(io.open(os.path.join(d, 'latest.json'), encoding='utf-8'))
assert back['version'] == V and back['platforms']['windows-x86_64']['url'] == url
print('ok', V, kid, os.path.getsize(os.path.join(d, 'mago-setup.exe')) // 1024, 'KB')
