# Original conditional viewport fixtures

Original HTML/CSS without external assets. Base and conditional stylesheets are
served from one owned loopback origin. The independent image control is generated
from the runner's original RGB component formulas: 4x4 source pixels at100% zoom
or5x5 source pixels at125%, with a4CSSpx image box. It therefore remains a real
1:1physical-pixel reference, without copying an image resizing implementation.

The fixed green base is independent of conditional rules. At full CSS viewport
1024px with primary hover, the two navigation links disappear and a32CSSpx green
target appears. At1023px, links remain and the red target is96CSSpx. Between1023
and1024 neither width condition matches, so the160CSSpx red fallback remains.
Print and unknown-feature branches must stay inactive. Full media width includes
the reserved scrollbar; available layout line width is a separate quantity.

The dynamic case inserts the same original conditional stylesheet through one
real button activation and checks current Page geometry in its genuine onload.
These fixtures claim no float layout, touch-device detection, conditional font
loading, public-site usability or CSSOM surface.
