<div align="center">

# ChezCat icon

*The project icon, original controls, and reproducible asset preparation.*

[Home](../README.md) · [Branding](../docs/BRANDING.md) · [Build guide](../docs/BUILDING.md)

</div>

---

The ChezCat icon is a C-cat monogram: a cream C with two feline ears and a teal tail terminal on a midnight navy rounded tile.

## Included assets

- `chezcat-icon.png`: the project icon on a 512 by 512 transparent canvas for the home page and reuse.
- `chezcat.ico`: Windows icon containing 16, 24, 32, 48, 64, 128 and 256 pixel images.
- The home-page copy at `pages/images/chezcat-icon.png` matches the normalized asset. Keeping it within the page content root preserves local-file resource boundaries.

The home page uses navy `#0b153c`, cream `#fff3dc` and teal `#42c7bc` to accompany the icon.

## Browser control icons

Build 0.23 adds 21 original geometric control icons in `src/ui.rs`. Home uses a
cream house outline and teal door; Refresh uses a cream arc and teal arrowhead.
The remaining navigation, menu, bookmark, history, settings and find glyphs use
the same rounded strokes and colors. These are our drawing code, with no stock
artwork, icon font, imported symbol set or additional runtime dependency.

## Format preparation

The checked-in PNG and ICO are the inputs for the home page and Windows resources. For an approved artwork update, an optional ImageMagick conversion trims empty surrounding space, scales and pads the image, and prepares the required icon sizes:

```text
magick path/to/artwork.png -trim +repage -resize 464x464 -background none -gravity center -extent 512x512 -depth 8 -interlace none -define png:color-type=6 -strip assets/chezcat-icon.png
magick assets/chezcat-icon.png -define icon:auto-resize=256,128,64,48,32,24,16 assets/chezcat.ico
```

Copy the normalized PNG to `pages/images/chezcat-icon.png` after any artwork update. Run `tools/build_icon_resources.py` after changing the ICO or `resources/chezcat.rc`; it regenerates the MSVC `.res` and GNU x64 `.o` files. Normal Cargo builds link those supplied resources and need none of these asset-authoring tools.
