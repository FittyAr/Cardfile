"""Turn the raw screenshots into the website assets: WebP slides, thumbnails and the social card.

Usage: python publish.py <shots-dir> <site-assets-dir> <cardfile-repo>
"""

import os
import sys

from PIL import Image, ImageDraw, ImageFilter, ImageFont

src, out, repo = sys.argv[1:4]
NAMES = ["editor", "travel", "recipe", "languages", "search", "palette",
         "history", "export", "private", "themes", "mcp", "spanish"]

os.makedirs(os.path.join(out, "screens"), exist_ok=True)
for i, n in enumerate(NAMES):
    im = Image.open(os.path.join(src, n + ".png")).convert("RGB")
    im.save(os.path.join(out, "screens", f"{i + 1:02d}-{n}.webp"), "WEBP", quality=88, method=6)
    im.resize((320, 200), Image.LANCZOS).save(
        os.path.join(out, "screens", f"{i + 1:02d}-{n}-thumb.webp"), "WEBP", quality=80, method=6)

# Social card, 1200 x 630.
W, H = 1200, 630
fonts = os.path.join(repo, "assets", "fonts")
bold = lambda size: ImageFont.truetype(os.path.join(fonts, "Inter-Bold.ttf"), size)
regular = ImageFont.truetype(os.path.join(fonts, "Inter-Regular.ttf"), 26)

bg = Image.new("RGB", (W, H), (10, 10, 13))
d = ImageDraw.Draw(bg)
for r in range(520, 0, -4):
    a = int(40 * (1 - r / 520))
    d.ellipse([W - 260 - r, -120 - r, W - 260 + r, -120 + r], fill=(10 + a * 3 // 2, 10 + a // 2, 13 + a // 3))
shot = Image.open(os.path.join(src, "editor.png")).convert("RGB")
shot = shot.resize((740, int(740 * shot.height / shot.width)), Image.LANCZOS)
mask = Image.new("L", shot.size, 0)
ImageDraw.Draw(mask).rounded_rectangle([0, 0, *shot.size], 14, fill=255)
shadow = Image.new("L", (W, H), 0)
ImageDraw.Draw(shadow).rounded_rectangle([520, 190, 520 + shot.width, 190 + shot.height], 14, fill=170)
bg.paste((0, 0, 0), mask=shadow.filter(ImageFilter.GaussianBlur(24)))
bg.paste(shot, (500, 170), mask)
icon = Image.open(os.path.join(repo, "assets", "logo_trasparente_1.png")).convert("RGBA").resize((96, 96), Image.LANCZOS)
bg.paste(icon, (64, 70), icon)
d = ImageDraw.Draw(bg)
d.text((64, 200), "Cardfile", font=bold(64), fill=(255, 249, 245))
d.text((64, 285), "Your notes,", font=bold(44), fill=(248, 193, 169))
d.text((64, 338), "one card at a time.", font=bold(44), fill=(242, 107, 74))
d.text((64, 420), "Native · Fast · Private", font=regular, fill=(160, 160, 170))
d.text((64, 458), "Windows & Linux · Rust", font=regular, fill=(160, 160, 170))
bg.save(os.path.join(out, "social.png"), optimize=True)
print("published", len(NAMES), "screenshots")
