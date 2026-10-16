#!/usr/bin/env python3
"""Render every Cardfile brand raster from the master SVGs in assets/brand/.

Build-time tool only; nothing here is a dependency of the app crates.

Requirements (all permissively licensed):
    pip install resvg-py pillow                 # rendering (resvg: Apache-2.0/MIT)
    pip install fonttools uharfbuzz             # only for `--outline-font`

Usage:
    python scripts/render_brand.py
        Compose logo.svg / logo-stacked.svg from icon.svg + wordmark.svg, then
        regenerate every PNG/ICO in assets/ and packaging/ plus
        assets/brand/preview.png.

    python scripts/render_brand.py --outline-font path/to/InterDisplay-SemiBold.ttf
        First rebuild assets/brand/wordmark.svg by shaping "Cardfile" with
        HarfBuzz and converting the glyphs to paths, then render as above.
        Inter is at https://github.com/rsms/inter/releases (extras/ttf/).
"""

from __future__ import annotations

import argparse
import io
import pathlib
import re

from PIL import Image, ImageDraw, ImageFont

import resvg_py

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRAND = ROOT / "assets" / "brand"
APP_ID = "ar.fittyar.Cardfile"

# Palette (keep in sync with assets/brand/README.md).
CORAL = "#F26B4A"  # tile, wordmark "file", card label
CORAL_DEEP = "#C8442A"  # wordmark "Card"
WARM_WHITE = "#FFF6F1"  # background of the opaque logo.png

WORDMARK_TEXT = ("Card", "file")
WORDMARK_COLORS = (CORAL_DEEP, CORAL)

ICON_GRID = 256  # icon.svg viewBox size
SMALL_MAX = 32  # sizes up to this use icon-small.svg


# --------------------------------------------------------------------------- render


def render(svg: str | pathlib.Path, width: int, height: int | None = None) -> Image.Image:
    """Rasterize an SVG file or string to an RGBA image of exactly width x height.

    The SVG is scaled to fit (keeping its aspect ratio) and centred.
    """
    height = height or width
    text = svg.read_text(encoding="utf-8") if isinstance(svg, pathlib.Path) else svg
    vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', text).group(1).split()]
    scale = min(width / vb[2], height / vb[3])
    w, h = round(vb[2] * scale), round(vb[3] * scale)
    png = resvg_py.svg_to_bytes(svg_string=text, width=w, height=h)
    img = Image.open(io.BytesIO(bytes(png))).convert("RGBA")
    if img.size == (width, height):
        return img
    canvas = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    canvas.alpha_composite(img, ((width - img.width) // 2, (height - img.height) // 2))
    return canvas


def icon(size: int, pad: float = 0.0) -> Image.Image:
    """App icon at `size` px, optionally with transparent padding (fraction per side)."""
    inner = round(size * (1 - 2 * pad))
    src = BRAND / ("icon-small.svg" if inner <= SMALL_MAX else "icon.svg")
    img = render(src, inner)
    if inner == size:
        return img
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    off = (size - inner) // 2
    canvas.alpha_composite(img, (off, off))
    return canvas


def on_canvas(img: Image.Image, w: int, h: int) -> Image.Image:
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    canvas.alpha_composite(img, ((w - img.width) // 2, (h - img.height) // 2))
    return canvas


def save(img: Image.Image, rel: str) -> None:
    path = ROOT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)
    print(f"wrote {rel}  {img.width}x{img.height}")


def save_ico(rel: str) -> None:
    sizes = [16, 24, 32, 48, 64, 128, 256]
    frames = [icon(s) for s in sizes]
    path = ROOT / rel
    # Pillow picks the frame whose size matches each entry of `sizes`, so the
    # hand-tuned small icon is used for 16/24/32 instead of a downscale.
    frames[-1].save(path, format="ICO", sizes=[(s, s) for s in sizes], append_images=frames[:-1])
    print(f"wrote {rel}  {sizes}")


# --------------------------------------------------------------------------- SVG composition


def svg_inner(path: pathlib.Path) -> tuple[list[float], str]:
    """Return (viewBox, inner markup) of an SVG file, without <title> or comments."""
    text = path.read_text(encoding="utf-8")
    vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', text).group(1).split()]
    inner = text[text.index(">", text.index("<svg")) + 1 : text.rindex("</svg>")]
    inner = re.sub(r"<!--.*?-->", "", inner, flags=re.S)
    inner = re.sub(r"<title>.*?</title>", "", inner, flags=re.S)
    inner = "\n".join(line for line in inner.splitlines() if line.strip())
    return vb, inner


def nested(path: pathlib.Path, x: float, y: float, w: float, h: float) -> str:
    vb, inner = svg_inner(path)
    vbs = " ".join(f"{v:g}" for v in vb)
    return f'<svg x="{x:g}" y="{y:g}" width="{w:g}" height="{h:g}" viewBox="{vbs}">\n{inner}\n</svg>'


def compose_logos() -> None:
    """Build logo.svg (horizontal) and logo-stacked.svg from icon.svg + wordmark.svg."""
    wm_vb, _ = svg_inner(BRAND / "wordmark.svg")
    wm_ratio = wm_vb[2] / wm_vb[3]
    header = (
        "<!-- Generated by scripts/render_brand.py from icon.svg and wordmark.svg; "
        "edit those and re-run instead of editing this file. -->"
    )

    # Horizontal lockup: the tile's own 12/256 margin is part of the spacing.
    s = ICON_GRID
    wm_h = s * 0.36
    wm_w = wm_h * wm_ratio
    gap = s * 0.06
    width = s + gap + wm_w
    logo = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0f}" height="{s}" '
        f'viewBox="0 0 {width:.2f} {s}">\n<title>Cardfile</title>\n{header}\n'
        + nested(BRAND / "icon.svg", 0, 0, s, s)
        + "\n"
        + nested(BRAND / "wordmark.svg", s + gap, (s - wm_h) / 2, wm_w, wm_h)
        + "\n</svg>\n"
    )
    (BRAND / "logo.svg").write_text(logo, encoding="utf-8", newline="\n")
    print("wrote assets/brand/logo.svg")

    # Stacked lockup (icon above wordmark), aspect of the legacy 578x639 PNG.
    W, H = 578.0, 639.0
    wm_w = W * 0.92
    wm_h = wm_w / wm_ratio
    gap = 26.0
    icon_s = H - wm_h - gap - 8
    top = (H - (icon_s + gap + wm_h)) / 2
    stacked = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W:.0f}" height="{H:.0f}" '
        f'viewBox="0 0 {W:g} {H:g}">\n<title>Cardfile</title>\n{header}\n'
        + nested(BRAND / "icon.svg", (W - icon_s) / 2, top, icon_s, icon_s)
        + "\n"
        + nested(BRAND / "wordmark.svg", (W - wm_w) / 2, top + icon_s + gap, wm_w, wm_h)
        + "\n</svg>\n"
    )
    (BRAND / "logo-stacked.svg").write_text(stacked, encoding="utf-8", newline="\n")
    print("wrote assets/brand/logo-stacked.svg")


# --------------------------------------------------------------------------- wordmark outlines


def outline_wordmark(font_path: pathlib.Path) -> None:
    """Shape the wordmark with HarfBuzz and write its glyph outlines to wordmark.svg."""
    import uharfbuzz as hb
    from fontTools.pens.boundsPen import BoundsPen
    from fontTools.pens.svgPathPen import SVGPathPen
    from fontTools.pens.transformPen import TransformPen
    from fontTools.ttLib import TTFont

    data = font_path.read_bytes()
    tt = TTFont(io.BytesIO(data))
    glyphs = tt.getGlyphSet()
    order = tt.getGlyphOrder()
    face = hb.Face(data)
    font = hb.Font(face)
    upem = face.upem
    tracking = -0.01 * upem  # slight negative tracking suits a display wordmark

    text = "".join(WORDMARK_TEXT)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(font, buf, {"kern": True, "liga": False})

    # Lay out glyphs (font units, y up), remembering which run each belongs to.
    placed = []
    x = 0.0
    split = len(WORDMARK_TEXT[0])
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        run = 0 if info.cluster < split else 1
        placed.append((order[info.codepoint], x + pos.x_offset, pos.y_offset, run))
        x += pos.x_advance + tracking

    bounds = BoundsPen(glyphs)
    for name, gx, gy, _ in placed:
        glyphs[name].draw(TransformPen(bounds, (1, 0, 0, 1, gx, gy)))
    xmin, ymin, xmax, ymax = bounds.bounds

    # Scale so the outline box is 100 units tall; flip y for SVG.
    k = 100.0 / (ymax - ymin)
    runs = ["", ""]
    for name, gx, gy, run in placed:
        pen = SVGPathPen(glyphs, ntos=lambda v: f"{round(v, 2):g}")
        glyphs[name].draw(TransformPen(pen, (k, 0, 0, -k, (gx - xmin) * k, (ymax - gy) * k)))
        runs[run] += pen.getCommands()
    w = (xmax - xmin) * k
    family = tt["name"].getDebugName(4)
    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0f}" height="100" viewBox="0 0 {w:.2f} 100">
  <title>Cardfile</title>
  <!-- "Cardfile" set in {family} (SIL Open Font License 1.1, rsms/inter),
       converted to outlines by scripts/render_brand.py. Both colours read on
       light and dark backgrounds. -->
  <path fill="{WORDMARK_COLORS[0]}" d="{runs[0]}"/>
  <path fill="{WORDMARK_COLORS[1]}" d="{runs[1]}"/>
</svg>
"""
    (BRAND / "wordmark.svg").write_text(svg, encoding="utf-8", newline="\n")
    print(f"wrote assets/brand/wordmark.svg  ({family})")


# --------------------------------------------------------------------------- preview sheet


def preview() -> None:
    sizes = [16, 32, 64, 128, 256]
    pad = 32
    row_h = 256 + 2 * pad
    logo_h = 128
    width = pad + sum(s + pad for s in sizes) + 160
    themes = [("light", (250, 247, 245, 255), (40, 40, 40)), ("dark", (28, 28, 30, 255), (220, 220, 220))]
    height = len(themes) * (row_h + logo_h + 2 * pad)
    sheet = Image.new("RGBA", (width, height))
    draw = ImageDraw.Draw(sheet)
    try:
        font = ImageFont.load_default(size=14)
    except TypeError:  # Pillow < 10.1
        font = ImageFont.load_default()
    y0 = 0
    for name, bg, fg in themes:
        band_h = row_h + logo_h + 2 * pad
        draw.rectangle([0, y0, width, y0 + band_h], fill=bg)
        x = pad
        for s in sizes:
            sheet.alpha_composite(icon(s), (x, y0 + pad + 256 - s))
            draw.text((x, y0 + pad + 256 + 6), f"{s}px", fill=fg, font=font)
            x += s + pad
        # 16 px icon magnified 8x (nearest neighbour) to check pixel fit.
        zoom = icon(16).resize((128, 128), Image.NEAREST)
        sheet.alpha_composite(zoom, (x, y0 + pad + 128))
        draw.text((x, y0 + pad + 256 + 6), "16px x8", fill=fg, font=font)
        ly = y0 + row_h + pad // 2
        vb, _ = svg_inner(BRAND / "logo.svg")
        logo = render(BRAND / "logo.svg", round(logo_h * vb[2] / vb[3]), logo_h)
        sheet.alpha_composite(logo, (pad, ly))
        mono = render(BRAND / "icon-mono.svg", logo_h)
        if name == "dark":
            # Show the symbol recoloured, as a monochrome consumer would.
            mono = render(
                (BRAND / "icon-mono.svg").read_text(encoding="utf-8").replace('color="#1F1A17"', 'color="#F2EDEA"'),
                logo_h,
            )
        sheet.alpha_composite(mono, (width - pad - logo_h, ly))
        draw.text((pad, y0 + 8), name, fill=fg, font=font)
        y0 += band_h
    out = BRAND / "preview.png"
    sheet.convert("RGB").save(out, optimize=True)
    print(f"wrote assets/brand/preview.png  {sheet.width}x{sheet.height}")


# --------------------------------------------------------------------------- main


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--outline-font", type=pathlib.Path, help="rebuild wordmark.svg from this TTF/OTF")
    args = ap.parse_args()

    if args.outline_font:
        outline_wordmark(args.outline_font)
    compose_logos()

    stacked = BRAND / "logo-stacked.svg"
    wordmark = BRAND / "wordmark.svg"

    # In-app and README logos (assets/, referenced by cardfile-ui and installer.iss).
    save(render(stacked, 578, 639), "assets/logo_trasparente.png")
    save(on_canvas(render(BRAND / "icon.svg", 458), 458, 462), "assets/logo_trasparente_1.png")
    save(render(wordmark, 563, 129), "assets/logo_trasparente_2.png")
    opaque = Image.new("RGBA", (1254, 1254), WARM_WHITE)
    opaque.alpha_composite(render(stacked, 900, 996), ((1254 - 900) // 2, (1254 - 996) // 2))
    save(opaque.convert("RGB"), "assets/logo.png")

    # Linux hicolor icons.
    for s in (16, 24, 32, 48, 64, 128, 256, 512):
        save(icon(s), f"packaging/linux/icons/{s}x{s}/{APP_ID}.png")

    # Windows executable / installer icon.
    save_ico("packaging/windows/cardfile.ico")

    # MSIX package assets (paddings match the platform's tile conventions).
    msix = "packaging/windows/msix/Assets"
    save(icon(44), f"{msix}/Square44x44Logo.png")
    save(icon(256), f"{msix}/Square44x44Logo.targetsize-256_altform-unplated.png")
    save(icon(71, 0.1), f"{msix}/Square71x71Logo.png")
    save(icon(150, 0.12), f"{msix}/Square150x150Logo.png")
    save(icon(310, 0.12), f"{msix}/Square310x310Logo.png")
    save(on_canvas(icon(120), 310, 150), f"{msix}/Wide310x150Logo.png")
    save(icon(50), f"{msix}/StoreLogo.png")
    save(on_canvas(icon(240), 620, 300), f"{msix}/SplashScreen.png")

    preview()


if __name__ == "__main__":
    main()
