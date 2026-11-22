"""Compose the Microsoft Store images from the illustrations (requires Pillow and NumPy).

Writes into `docs/store/logos/`: the text-free hero art, and the box and poster art with the
NoteChest logo on top. Run: `python assets/brand/notechest/compose_store.py`.
"""

from pathlib import Path
import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
B = REPO / "assets/brand/notechest"
OUT = REPO / "docs/store/logos"
OUT.mkdir(parents=True, exist_ok=True)
LANCZOS = Image.Resampling.LANCZOS

logo = Image.open(B / "notechest-stacked.png").convert("RGBA")  # charcoal wordmark


def with_logo(plate, size, logo_width, top):
    """Scale the plate to size x size (or size x 1.5 size) and put the logo on its top third."""
    plate = plate.convert("RGBA")
    k = logo_width / logo.width
    mark = logo.resize((logo_width, round(logo.height * k)), LANCZOS)
    plate.alpha_composite(mark, ((plate.width - mark.width) // 2, top))
    return plate


# 1. Hero art, 16:9, no text. The chest base reaches 70% of the height in the source, and the
# Store may cover the bottom third with a gradient, so extend the background (it is a soft,
# even ivory) until the chest ends above 66%.
hero = Image.open(B / "illustrations/hero-note-chest-01.png").convert("RGB")
h = 1010
w = round(h * 16 / 9)
arr = np.array(hero)
pad_y = h - arr.shape[0]
pad_x = (w - arr.shape[1]) // 2
arr = np.pad(arr, ((0, pad_y), (pad_x, w - arr.shape[1] - pad_x), (0, 0)), mode="edge")
Image.fromarray(arr).resize((1920, 1080), LANCZOS).save(OUT / "hero-art-1920x1080.png", optimize=True)

# 2. Box art, 1:1: title in the upper two thirds, calm bottom third.
square = Image.open(B / "illustrations/listing-note-chest-square-01.png").convert("RGB")
big = square.resize((2160, 2160), LANCZOS)
box = with_logo(big, 2160, 520, 150).convert("RGB")
box.save(OUT / "box-art-2160x2160.png", optimize=True)
box.resize((1080, 1080), LANCZOS).save(OUT / "box-art-1080x1080.png", optimize=True)

# 3. Poster art, 2:3.
portrait = Image.open(B / "illustrations/listing-note-chest-portrait-01.png").convert("RGB")
big = portrait.resize((1440, 2160), LANCZOS)
poster = with_logo(big, 1440, 560, 160).convert("RGB")
poster.save(OUT / "poster-art-1440x2160.png", optimize=True)
poster.resize((720, 1080), LANCZOS).save(OUT / "poster-art-720x1080.png", optimize=True)

for p in sorted(OUT.glob("*.png")):
    im = Image.open(p)
    print(f"{p.name:28} {im.size[0]}x{im.size[1]}  {p.stat().st_size // 1024} KB  {im.mode}")
