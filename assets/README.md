# ChezCat icon

The selected direction is the C-cat monogram: a cream C with two feline ears and a teal tail terminal on a midnight navy rounded tile. This is the third concept selected by the user for ChezCat.

## Included assets

- `chezcat-icon-source.png`: the untouched selected generated artwork, 1254 by 1254 pixels, with alpha.
- `chezcat-icon.png`: the same artwork normalized to a 512 by 512 transparent canvas for the home page and reuse.
- `chezcat.ico`: Windows icon containing 16, 24, 32, 48, 64, 128 and 256 pixel images.
- The home-page copy at `pages/images/chezcat-icon.png` matches the normalized asset. Keeping it within the page content root preserves local-file resource boundaries.

The icon was created using the built-in image generation tool from an original project brief. No input artwork, stock icon, existing logo or branded character was supplied. These files contain only the chosen direction.

The home page uses navy `#0b153c`, cream `#fff3dc` and teal `#42c7bc` to accompany the icon.

## Browser control icons

Build 0.23 adds 21 original geometric control icons in `src/ui.rs`. Home uses a
cream house outline and teal door; Refresh uses a cream arc and teal arrowhead.
The remaining navigation, menu, bookmark, history, settings and find glyphs use
the same rounded strokes and colors. These are our drawing code, with no stock
artwork, icon font, imported symbol set or additional runtime dependency.

## Format preparation

Only empty surrounding space was trimmed before scaling and padding; the selected design was not redrawn. From the project folder, ImageMagick can reproduce the prepared formats:

```text
magick assets/chezcat-icon-source.png -trim +repage -resize 464x464 -background none -gravity center -extent 512x512 -depth 8 -interlace none -define png:color-type=6 -strip assets/chezcat-icon.png
magick assets/chezcat-icon.png -define icon:auto-resize=256,128,64,48,32,24,16 assets/chezcat.ico
```

Copy the normalized PNG to `pages/images/chezcat-icon.png` after any artwork update. Run `tools/build_icon_resources.py` after changing the ICO or `resources/chezcat.rc`; it regenerates the MSVC `.res` and GNU x64 `.o` files. Normal Cargo builds link those supplied resources and need none of these asset-authoring tools.
