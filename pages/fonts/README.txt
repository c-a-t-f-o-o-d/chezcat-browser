ChezCat Geometry — original font test artwork

chezcat-geometry.ttf and chezcat-geometry.woff are generated from the original
geometric outlines in src/font_tests.rs. They contain only a few test glyphs:
A, V, o, an accented composite mapped to é, a diamond mapped to U+1F600, space,
and the missing-glyph box. No installed or third-party font bytes are included.

To reproduce from the repository root (PowerShell):
  $env:CHEZCAT_FONT_FIXTURES = (Join-Path $PWD 'pages/fonts')
  cargo test font::tests::original_ttf_and_woff_share_unicode_advances_and_rasters

The fixture deliberately has no TrueType hinting program. ChezCat uses its own
quadratic-outline rasterizer; it never executes font hinting bytecode.
