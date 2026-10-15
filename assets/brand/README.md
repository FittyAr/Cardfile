# Cardfile brand assets

The master artwork is SVG. Every logo and icon raster in the repository is rendered from these
files by `scripts/render_brand.py`, so edit the SVGs and re-render. Do not edit the PNGs or ICOs.

## Files

| File | What it is |
|------|------------|
| `icon.svg` | App icon: a coral rounded-square tile with a stack of three index cards; the back card has a tab. 256 × 256 grid. Used for every size of 48 px and up. |
| `icon-small.svg` | Simplified icon for 16, 24 and 32 px: two cards and no shadow, with edges on whole pixels at 16 px. 32 × 32 grid. |
| `icon-mono.svg` | Single-colour symbol with no tile, for monochrome uses (tray, print, embossing). Paints with `currentColor`. |
| `wordmark.svg` | "Cardfile" set in Inter Display SemiBold and converted to outlines (no font needed at render time). |
| `logo.svg` | Horizontal lockup: icon + wordmark. **Generated** from `icon.svg` + `wordmark.svg`. |
| `logo-stacked.svg` | Stacked lockup (icon above wordmark), used for `assets/logo_trasparente.png`. **Generated**. |
| `preview.png` | Review sheet: the icon at 16/32/64/128/256 px and the logo, on light and dark backgrounds. |

Every file reads on both light and dark backgrounds. The wordmark uses two corals rather than a
dark ink so a single raster works on both UI themes.

## Palette

| Role | Hex |
|------|-----|
| Coral (tile, "file", card label) | `#F26B4A` |
| Deep coral (wordmark "Card") | `#C8442A` |
| Tab | `#B93D24` |
| Back card | `#F8C1A9` |
| Middle card | `#FCDCCC` |
| Front card (warm white) | `#FFF9F5` |
| Card lines | `#F6B59D` |
| Shadow (blurred, 32 % opacity) | `#9C3019` |
| Mono symbol (default) | `#1F1A17` |
| Background of the opaque `assets/logo.png` | `#FFF6F1` |

## Rendered outputs

| Output | Size | Source |
|--------|------|--------|
| `assets/logo_trasparente.png` | 578 × 639 | `logo-stacked.svg` (login, settings, wizard, README, Inno Setup wizard image) |
| `assets/logo_trasparente_1.png` | 458 × 462 | `icon.svg` (toolbar icon, Inno Setup small wizard image) |
| `assets/logo_trasparente_2.png` | 563 × 129 | `wordmark.svg` |
| `assets/logo.png` | 1254 × 1254, opaque | `logo-stacked.svg` on `#FFF6F1` |
| `assets/icon.ico`, `packaging/windows/cardfile.ico` | 16, 24, 32, 48, 64, 128, 256 | `icon-small.svg` (≤ 32) + `icon.svg` |
| `packaging/linux/icons/<n>x<n>/ar.fittyar.Cardfile.png` | 16, 24, 32, 48, 64, 128, 256, 512 | same |
| `packaging/windows/msix/Assets/*.png` | Store sizes (44, 50, 71, 150, 256, 310, 310 × 150, 620 × 300) | `icon.svg` |

## Re-rendering

The tools are build-time only and are not dependencies of the app.

```bash
python -m venv .venv-brand
.venv-brand/bin/pip install resvg-py pillow fonttools uharfbuzz   # Windows: .venv-brand\Scripts\pip
.venv-brand/bin/python scripts/render_brand.py
```

To rebuild the wordmark outlines as well (only needed when the lettering changes), download Inter
from <https://github.com/rsms/inter/releases> and pass the font from `extras/ttf/`:

```bash
.venv-brand/bin/python scripts/render_brand.py --outline-font Inter-4.1/extras/ttf/InterDisplay-SemiBold.ttf
```

The current files were produced with Inter 4.1, resvg-py 0.5.0 (resvg 0.48), uharfbuzz 0.56,
fontTools 4.67 and Pillow 12.3.

Inter is licensed under the SIL Open Font License 1.1 (see `docs/CREDITS.md`).
