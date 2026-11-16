# Original compatibility workflows

These development fixtures define three finite, JavaScript-enabled workflows.

- W1: original vanilla Unicode form search, Promise/timer completion, actual result link, one Back action and a different query.
- W2: unmodified jQuery 1.12.4 and Bootstrap 3.4.1 dropdown selection, trusted input, asynchronous text update and a real GET form result.
- W3: unmodified Vue 2.7.16 runtime-only with an original render function, name input, two counter clicks and Promise/timer/nextTick completion. Foundation 6.9.0 CSS is scored independently at 1280×800 and 640×800, 100% zoom.

All example queries are synthetic. The three workflows have four viewport configurations; results do not imply support for other versions, Vue 3, an entire framework or a public site. Load, initialization, rendering, input, async and history are separate observations. The first ordered script failure is retained separately from later potentially cascading messages.

The original HTML/CSS/JavaScript fixtures use no external artwork or fonts. The four-color 4×4 image is generated from explicit RGB bytes by the driver. Fixture hashes and exact unmodified development-input identities are in `fixtures.json` and `inputs.json`. Keep the distributions and retained MIT notices in an external development cache using the documented layout. They are not runtime dependencies and are not copied into the installed application. The driver does not download, install, rewrite or execute package install scripts.

Run only with an existing executable, its actual retained build record and an existing explicit exact-term license receipt:

```text
python tools/check_compatibility_workflows.py --run --inputs EXTERNAL_CACHE --exe EXECUTABLE --build-record BUILD_JSON
```

Without `--run`, the driver prints its plan. The build record must contain a 40-character `source_commit` and exact 64-character `executable_sha256`; it may retain `dirty`, `cargo_version` and `source_map_sha256`. Keep dirty inputs identified as dirty. The driver checks executable bytes and records the build record's digest; it does not infer compiled source from a later checkout or provide a signed attestation. It checks existing acceptance and never records consent.

The App/input process runs with JavaScript enabled, `--no-profile`, a fresh owned working directory, a finite watchdog and an owned IPv4 loopback server. The fixture CSP allows only same-origin external scripts/styles, with no inline/eval grant or object content. The server is closed and joined after each case. Source/layout/canvas stamps, generated pixels, exact control pixels and response MIME/hash/bytes are retained in fresh `results/verification` output. Generated reports and logs are local evidence, not source fixtures or release payloads.

Optional initial reference capture requires all three explicit `--reference-exe`, `--reference-version` and `--reference-sha256` values for an installed headless Edge/Chromium. It uses fresh owned profiles, no physical input, no debugging endpoint and no web-security/sandbox bypass. It captures initial DOM and PNG only. Reference interaction remains Not run; reference pixels never substitute for ChezCat output. Generic system fonts and ChezCat bundled fonts are not presumed identical.

This scorecard is not part of the normal regression suite until its integration and actual execution are validated. Failing or unrun stages remain visible; no polyfill, vendor substitution or suppressed script failure changes their result.
