"""Turn the raw screenshots into the website assets: WebP slides and thumbnails.

The social card (assets/social.png) is the brand logo on ivory #FCF4EB, built from
assets/brand/notechest/notechest-stacked.png on main; it is not generated here.

Usage: python publish.py <shots-dir> <site-assets-dir>
"""

import os
import sys

from PIL import Image

src, out = sys.argv[1:3]
NAMES = ["editor", "travel", "recipe", "languages", "search", "palette",
         "history", "export", "private", "themes", "mcp", "spanish"]

os.makedirs(os.path.join(out, "screens"), exist_ok=True)
for i, n in enumerate(NAMES):
    im = Image.open(os.path.join(src, n + ".png")).convert("RGB")
    im.save(os.path.join(out, "screens", f"{i + 1:02d}-{n}.webp"), "WEBP", quality=88, method=6)
    im.resize((320, 200), Image.LANCZOS).save(
        os.path.join(out, "screens", f"{i + 1:02d}-{n}-thumb.webp"), "WEBP", quality=80, method=6)

print("published", len(NAMES), "screenshots")
