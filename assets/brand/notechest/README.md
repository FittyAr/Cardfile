# NoteChest origami identity

Approved NC paper-fold concept, generated with the built-in image generation tool.
This directory contains raster artwork, not vector masters. It is a separate
asset package; the existing application asset pipeline does not consume it yet.

- `nc-origami.png`: symbol only, transparent, trimmed to visible artwork.
- `notechest-stacked.png`: symbol and NoteChest wordmark, transparent, trimmed.
- `source/`: unchanged generated transparent sources and the approved opaque reference.
- `png/`: transparent square icons from 16 through 1024 pixels.
- `windows/notechest.ico`: 16, 24, 32, 48, 64, 128 and 256 pixel frames.
- `linux/<size>/ar.fittyar.NoteChest.png`: desktop icon sizes.
- `web/`: lossless transparent WebP logos, favicon and browser/mobile PNG icons.

The charcoal wordmark is intended for light backgrounds. Small icons are
downsampled from the detailed paper artwork; they are not hand-tuned vectors.
The 1024 pixel icon is an export canvas size, not a claim of additional source detail.

Regenerate format and size conversions with `python assets/brand/notechest/export.py`
(Pillow required). Internal paper shading and source alpha are preserved.

Generation prompts: preserve the approved NC coral/peach origami, crisp paper
folds and matte texture; remove the ivory background to alpha transparency;
export one stacked version with the exact NoteChest wordmark and one symbol-only
version. The symbol-only final edit removed the wordmark from the transparent
stacked logo while preserving the monogram position and design.
