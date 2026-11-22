"""Export the approved raster origami identity (requires Pillow, NumPy and SciPy).

Regenerates every size and format under this folder from `source/`, and then copies
the files the product consumes (Windows icon, Linux icons, Microsoft Store tile
images and the installer wizard images) into `packaging/`.
"""

import shutil
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
SIZES = (16, 24, 32, 44, 48, 50, 64, 71, 128, 150, 192, 256, 310, 512, 1024)
LINUX_SIZES = (16, 24, 32, 48, 64, 128, 256, 512)
# Smaller pieces of visible artwork are specks left by background removal.
MIN_FRAGMENT_PIXELS = 200
# Opacity (out of 255) below which a pixel is indistinguishable from transparent.
MIN_ALPHA = 8
# Off-white for the wordmark on dark backgrounds.
LIGHT_WORDMARK = (0xF3, 0xEF, 0xEA)


def despeckle(image):
    """Drop isolated specks the background removal left behind (the folds are large)."""
    rgba = np.array(image)
    # A haze of almost invisible pixels would join specks to the artwork.
    rgba[..., 3] = np.where(rgba[..., 3] < MIN_ALPHA, 0, rgba[..., 3])
    visible = rgba[..., 3] > 0
    labels, count = ndimage.label(visible, structure=np.ones((3, 3)))
    if count:
        sizes = ndimage.sum(visible, labels, range(1, count + 1))
        keep = np.zeros(count + 1, dtype=bool)
        keep[1:] = sizes >= MIN_FRAGMENT_PIXELS
        rgba[..., 3] = np.where(keep[labels], rgba[..., 3], 0)
    return Image.fromarray(rgba, "RGBA")


def artwork(name):
    image = despeckle(Image.open(ROOT / "source" / name).convert("RGBA"))
    # Use the visible artwork for framing; preserve the source alpha unchanged.
    bounds = image.getchannel("A").point(lambda a: 255 if a >= 128 else 0).getbbox()
    if bounds is None:
        raise ValueError(f"Empty artwork: {name}")
    return image.crop(bounds)


def on_dark(logo):
    """The stacked logo for dark backgrounds: the charcoal wordmark turned off-white.

    The orange paper keeps its colour: only low-chroma dark pixels are recoloured.
    """
    rgba = np.array(logo)
    rgb = rgba[..., :3].astype(int)
    chroma = rgb.max(axis=2) - rgb.min(axis=2)
    charcoal = (chroma < 40) & (rgb.max(axis=2) < 120)
    rgba[charcoal, :3] = LIGHT_WORDMARK
    return Image.fromarray(rgba, "RGBA")


def canvas(image, width, height, margin=0.06, background=(0, 0, 0, 0)):
    fitted = image.copy()
    fitted.thumbnail((round(width * (1 - 2 * margin)), round(height * (1 - 2 * margin))), Image.Resampling.LANCZOS)
    result = Image.new("RGBA", (width, height), background)
    result.alpha_composite(fitted, ((width - fitted.width) // 2, (height - fitted.height) // 2))
    return result


def export_packages(symbol, logo):
    """Copy what the product consumes: Windows/Linux icons, Store tiles, wizard images."""
    packaging = REPO / "packaging"
    shutil.copyfile(ROOT / "windows" / "notechest.ico", packaging / "windows" / "notechest.ico")
    for size in LINUX_SIZES:
        target = packaging / "linux" / "icons" / f"{size}x{size}"
        target.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(
            ROOT / "linux" / f"{size}x{size}" / "ar.fittyar.NoteChest.png",
            target / "ar.fittyar.NoteChest.png",
        )
    msix = packaging / "windows" / "msix" / "Assets"
    msix.mkdir(parents=True, exist_ok=True)
    tiles = {
        "StoreLogo.png": 50,
        "Square44x44Logo.png": 44,
        "Square44x44Logo.targetsize-256_altform-unplated.png": 256,
        "Square71x71Logo.png": 71,
        "Square150x150Logo.png": 150,
        "Square310x310Logo.png": 310,
    }
    for name, size in tiles.items():
        canvas(symbol, size, size, margin=0.06 if size >= 150 else 0.04).save(msix / name)
    canvas(symbol, 310, 150, margin=0.12).save(msix / "Wide310x150Logo.png")
    canvas(symbol, 620, 300, margin=0.18).save(msix / "SplashScreen.png")
    # The wizard panel of the installer is stretched to fit, so these keep its shape.
    white = (255, 255, 255, 255)
    canvas(logo, 328, 628, margin=0.1, background=white).convert("RGB").save(
        packaging / "windows" / "wizard-large.png", optimize=True
    )
    canvas(symbol, 110, 116, margin=0.08, background=white).convert("RGB").save(
        packaging / "windows" / "wizard-small.png", optimize=True
    )


def main():
    symbol = artwork("nc-origami.png")
    logo = artwork("notechest-stacked.png")
    light = on_dark(logo)
    for folder in ("png", "windows", "linux", "web"):
        (ROOT / folder).mkdir(exist_ok=True)
    symbol.save(ROOT / "nc-origami.png", optimize=True)
    logo.save(ROOT / "notechest-stacked.png", optimize=True)
    light.save(ROOT / "notechest-stacked-light.png", optimize=True)
    symbol.save(ROOT / "web" / "nc-origami.webp", lossless=True)
    logo.save(ROOT / "web" / "notechest-stacked.webp", lossless=True)
    light.save(ROOT / "web" / "notechest-stacked-light.webp", lossless=True)
    for size in SIZES:
        icon = canvas(symbol, size, size)
        icon.save(ROOT / "png" / f"notechest-{size}.png", optimize=True)
        if size in LINUX_SIZES:
            target = ROOT / "linux" / f"{size}x{size}"
            target.mkdir(exist_ok=True)
            icon.save(target / "ar.fittyar.NoteChest.png", optimize=True)
    icon = canvas(symbol, 256, 256)
    ico_sizes = [(s, s) for s in (16, 24, 32, 48, 64, 128, 256)]
    icon.save(ROOT / "windows" / "notechest.ico", sizes=ico_sizes)
    icon.save(ROOT / "web" / "favicon.ico", sizes=ico_sizes)
    canvas(symbol, 180, 180).save(ROOT / "web" / "apple-touch-icon.png")
    for size in (192, 512):
        canvas(symbol, size, size).save(ROOT / "web" / f"icon-{size}.png")
    for path in ROOT.rglob("*.png"):
        if "source" in path.parts:
            continue
        with Image.open(path) as image:
            if image.mode != "RGBA" or image.getchannel("A").getextrema()[0] != 0:
                raise ValueError(f"Missing transparency: {path}")
    with Image.open(ROOT / "windows" / "notechest.ico") as image:
        if image.ico.sizes() != set(ico_sizes):
            raise ValueError("Missing ICO frames")
    export_packages(symbol, logo)
    print(
        "Exported transparent PNG/WebP (with a dark-background logo), Linux icons and 7-size "
        "Windows/favicon ICOs, and refreshed the files under packaging/; verified alpha and ICO frames."
    )


if __name__ == "__main__":
    main()
