#!/usr/bin/env python3
"""Generate platform icons from assets/logo_trasparente_1.png (requires Pillow).

Run: python scripts/gen-icons.py
"""
import pathlib

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = Image.open(ROOT / "assets" / "logo_trasparente_1.png").convert("RGBA")
APP_ID = "ar.fittyar.Cardfile"


def square(size, pad=0.0):
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    inner = round(size * (1 - 2 * pad))
    icon = SRC.resize((inner, inner), Image.LANCZOS)
    off = (size - inner) // 2
    canvas.paste(icon, (off, off), icon)
    return canvas


def wide(w, h):
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    inner = round(h * 0.8)
    icon = SRC.resize((inner, inner), Image.LANCZOS)
    canvas.paste(icon, ((w - inner) // 2, (h - inner) // 2), icon)
    return canvas


def save(img, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path)
    print("wrote", path.relative_to(ROOT))


for size in (16, 24, 32, 48, 64, 128, 256, 512):
    save(square(size), ROOT / "packaging" / "linux" / "icons" / f"{size}x{size}" / f"{APP_ID}.png")

msix = ROOT / "packaging" / "windows" / "msix" / "Assets"
save(square(44), msix / "Square44x44Logo.png")
save(square(256), msix / "Square44x44Logo.targetsize-256_altform-unplated.png")
save(square(71, 0.1), msix / "Square71x71Logo.png")
save(square(150, 0.12), msix / "Square150x150Logo.png")
save(square(310, 0.12), msix / "Square310x310Logo.png")
save(wide(310, 150), msix / "Wide310x150Logo.png")
save(square(50), msix / "StoreLogo.png")
save(wide(620, 300), msix / "SplashScreen.png")

# Multi-resolution Windows icon.
ico = ROOT / "packaging" / "windows" / "cardfile.ico"
square(256).save(ico, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print("wrote", ico.relative_to(ROOT))
