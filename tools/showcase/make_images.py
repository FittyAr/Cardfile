"""Generate the flat illustrations used by the showcase seed (no third-party artwork)."""

import math
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)


def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(img)
    for y in range(h):
        d.line([(0, y), (w, y)], fill=lerp(top, bottom, y / h))
    return img


# Lisbon at sunset: sky, sun, hills of tiled houses, river.
W, H = 1200, 640
img = gradient(W, H, (255, 183, 130), (246, 108, 92))
d = ImageDraw.Draw(img)
d.ellipse([760, 180, 980, 400], fill=(255, 230, 170))
rng = random.Random(7)
for layer, (base, color) in enumerate([(360, (196, 82, 84)), (420, (150, 60, 78)), (480, (104, 44, 70))]):
    x = -20
    while x < W:
        bw = rng.randint(50, 110)
        bh = rng.randint(40, 140) - layer * 10
        top = base - bh + int(30 * math.sin(x / 180 + layer))
        d.rectangle([x, top, x + bw, H], fill=color)
        d.polygon([(x - 6, top), (x + bw // 2, top - 26), (x + bw + 6, top)], fill=lerp(color, (90, 30, 40), 0.35))
        for wy in range(top + 14, base + 40, 26):
            for wx in range(x + 10, x + bw - 14, 22):
                if rng.random() < 0.45:
                    d.rectangle([wx, wy, wx + 8, wy + 12], fill=(255, 214, 150))
        x += bw + rng.randint(0, 8)
d.rectangle([0, 540, W, H], fill=(74, 52, 92))
for i in range(40):
    y = rng.randint(548, 630)
    x = rng.randint(0, W)
    d.line([(x, y), (x + rng.randint(30, 90), y)], fill=(255, 190, 140), width=2)
# Tram
d.rounded_rectangle([140, 470, 330, 540], 10, fill=(250, 204, 21))
for wx in range(156, 320, 34):
    d.rectangle([wx, 482, wx + 24, 506], fill=(255, 245, 220))
d.line([(235, 470), (260, 430)], fill=(40, 30, 40), width=3)
img.save(out / "lisbon.jpg", quality=88)

# Sourdough loaf on a board.
W, H = 1000, 620
img = gradient(W, H, (246, 236, 222), (226, 206, 184))
d = ImageDraw.Draw(img)
d.rounded_rectangle([120, 400, 880, 470], 30, fill=(168, 110, 64))
d.rounded_rectangle([120, 400, 880, 420], 12, fill=(190, 132, 82))
shadow = Image.new("L", (W, H), 0)
ImageDraw.Draw(shadow).ellipse([230, 360, 770, 430], fill=120)
img.paste((120, 80, 50), mask=shadow.filter(ImageFilter.GaussianBlur(16)))
d = ImageDraw.Draw(img)
d.ellipse([240, 170, 760, 420], fill=(176, 104, 48))
d.ellipse([260, 180, 740, 400], fill=(204, 134, 70))
for i, x in enumerate(range(330, 700, 90)):
    d.arc([x - 40, 200, x + 80, 330], 200, 330, fill=(246, 214, 160), width=10)
for _ in range(220):
    x, y = random.randint(280, 720), random.randint(200, 390)
    d.point((x, y), fill=(250, 232, 200))
img.save(out / "sourdough.jpg", quality=88)

# Simple launch chart (sign-ups per week).
W, H = 1000, 460
img = Image.new("RGB", (W, H), (255, 249, 245))
d = ImageDraw.Draw(img)
vals = [120, 180, 260, 240, 380, 520, 610, 780]
maxv = 800
for i in range(5):
    y = 400 - i * 90
    d.line([(70, y), (960, y)], fill=(240, 222, 212), width=2)
bw = 80
for i, v in enumerate(vals):
    x = 100 + i * 108
    h = int(v / maxv * 360)
    d.rounded_rectangle([x, 400 - h, x + bw, 400], 10, fill=(242, 107, 74) if i == len(vals) - 1 else (248, 193, 169))
img.save(out / "chart.png")
print("ok")
