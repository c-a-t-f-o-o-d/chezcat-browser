# Current library development fixtures

Two separate original apps measure unmodified jQuery 4.0.0 and Bootstrap 5.3.9.
They complement the existing legacy W2 fixture; they do not replace it or imply
whole-library support. Bootstrap runs independently without jQuery, using its
classic bundle with Popper 2.11.8. These versions were checked against official
release pages and tagged package metadata on 2026-11-16:

- https://releases.jquery.com/
- https://blog.jquery.com/2026/01/17/jquery-4-0-0/
- https://blog.getbootstrap.com/2026/11/11/bootstrap-5-3-9/
- https://github.com/twbs/bootstrap/blob/v5.3.9/package-lock.json

The runner never downloads, installs or accepts a license. `inputs.json` pins
external development-only assets and exact MIT notices. Preserve the upstream
distributions unchanged outside source/runtime directories. Keep their notices
here; neither libraries nor this development server belong in runtime bundles.

`python tools/check_modern_compatibility_workflows.py` prints the finite plan.
`--check-inputs --inputs <external-modern-cache>` checks pins without running an
executable. `--run --inputs <external-modern-cache> --build-record <actual-build-json>`
requires an exact executable digest and existing real acceptance. `--exe` selects
the corresponding executable. Outputs must be fresh children of the configured
verification storage (`CHEZCAT_VERIFICATION_ROOT` or the normal default).

The actual App driver supplies trusted input and paired source/layout/pixel
snapshots. The jQuery app applies a field value after a real click and uses an
original timer callback to update text through jQuery; it does not establish
AJAX/Deferred support. The Bootstrap app opens a real dropdown, chooses its item,
and measures its two responsive columns at 1280 and 640. No library is patched,
polyfilled, replaced, or driven through a reference browser.

Loading, demonstrated parsing, initialization, initial rendering and interaction
are reported separately. A syntax failure is distinguished only when the engine
explicitly identifies parsing; reaching the initialization marker establishes the
stated initial path, not every dormant function. An earlier failure leaves later
stages unproved. Initial rendering never substitutes for library initialization.
The first causal error is retained separately from later messages. Results stay
private; pinning these fixtures is not a compatibility pass.
