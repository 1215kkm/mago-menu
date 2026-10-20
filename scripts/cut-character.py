"""고양이 시트(5×3)를 자세별로 잘라 누끼 → 높이 240 · 같은 바닥선 webp (ui/mago/cat/)."""
import sys, os
import numpy as np
from PIL import Image
from rembg import remove, new_session
from scipy import ndimage

src, out, dbg = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(out, exist_ok=True)
sheet = Image.open(src).convert('RGB')
NAMES = [['idle', 'wait', 'wait_heart', 'feed_near', 'eating'],
         ['satisfied', 'more_please', 'chewing', 'full', 'poop_start'],
         ['poop_done', 'menu_open', 'crown', 'love', 'sleep']]
COLS = [0, 307, 614, 921, 1228, 1536]
ROWS = [(0, 312), (348, 624), (655, 909)]
cuts = {}
sess = new_session(sys.argv[4] if len(sys.argv) > 4 else 'isnet-general-use')
for r, (y0, y1) in enumerate(ROWS):
    for c in range(5):
        name = NAMES[r][c]
        cell = sheet.crop((COLS[c], y0, COLS[c + 1], y1))
        cut = remove(cell, session=sess).convert('RGBA')
        a = np.array(cut)
        alpha = a[:, :, 3].copy()
        # 자세마다 시트에 같이 그려진 군더더기: 먹는 중 위의 닭강정(손에 들린 것), 메뉴 열림의 + 말풍선(화면이 따로 그린다)
        if name == 'eating':
            alpha[:110, :75] = 0
        if name == 'menu_open':
            reg = alpha[:100, 215:]
            reg[reg < 235] = 0
        # 검은 고양이 귀 안쪽이 배경으로 잘못 빠진 곳(구멍·반투명)을 몸 안쪽이면 꽉 채운다
        body = ndimage.binary_fill_holes(alpha > 30)
        inner = ndimage.binary_erosion(body, iterations=2)
        alpha[inner] = 255
        # 떨어진 반짝이·빛은 버리고, 큰 덩어리(몸)와 그 근처 큰 조각(똥·왕관·하트)만 남긴다
        lab, n = ndimage.label(alpha > 40)
        if n:
            sizes = ndimage.sum(np.ones_like(alpha), lab, range(1, n + 1))
            big = sizes.max()
            keep = np.zeros(n + 1, bool)
            for i, s in enumerate(sizes, 1):
                keep[i] = s >= big * 0.04
            a[:, :, 3] = np.where(keep[lab], alpha, 0)
        cut = Image.fromarray(a)
        cut.save(f'{dbg}/{name}.png')
        cuts[name] = cut

# 모든 자세를 같은 배율로 (기본 자세 키 → 240), 바닥선을 맞춰 높이 240 판에 놓는다.
# 왕관처럼 더 높은 자세만 판에 맞게 줄인다.
def box(im):
    return im.getchannel('A').point(lambda v: 255 if v > 8 else 0).getbbox()
s = 240 / (box(cuts['idle'])[3] - box(cuts['idle'])[1])
for name, cut in cuts.items():
    cut = cut.crop(box(cut))
    w, h = cut.size
    k = min(s, 240 / h)
    im = cut.resize((max(1, round(w * k)), max(1, round(h * k))), Image.LANCZOS)
    board = Image.new('RGBA', (im.width, 240), (0, 0, 0, 0))
    board.paste(im, (0, 240 - im.height), im)
    board.save(f'{out}/{name}.webp', 'WEBP', quality=88, method=6)
    print(name, board.size, round(k, 3))
