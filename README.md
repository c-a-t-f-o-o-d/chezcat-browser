# ChezCat

An experimental Windows x64 browser with an original Rust engine: networking,
JavaScript, DOM, layout, rendering and selected media decoding. It does not embed
Chromium, Gecko, WebKit or an external JavaScript/codec runtime.

The published [**0.81.0-alpha.2** prerelease](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2) consolidates
the completed finite B2 tabs/windows/link milestone and integrated platform
and compatibility prerequisites since **0.80.0-alpha.1**. Its immutable source,
four downloadable assets, checksums and extracted offline runtime were verified.
The earlier **0.80.0-alpha.1** release remains the
offline CSAPUL 1.1 acceptance milestone, with its supplied tag and assets unchanged.

This is an experimental browser without a security certification. Web-platform
support is partial. The renderer and interpreter run in the browser process with
worker threads, without an OS renderer sandbox.

The production Cargo manifest has no third-party dependencies. Rust's standard
library, Windows APIs, data, assets and development tools have their own roles
and licensing obligations; see [third-party notices](THIRD_PARTY_NOTICES.md).

## Scope

- Full offline native license viewing and explicit acceptance, with source and
  headless commands, personal/organization scopes and minimal external receipts.
  Decline or invalid acceptance blocks browsing and source builds.
- HTTP(S), original TLS/certificate handling, memory-only session cookies,
  redirects and supported CSP/resource policies. Browser-owned trust data includes
  a conservative 98-authority Mozilla NSS subset, with pinned provenance,
  offline generation and explicit custom-store replacement; see
  [HTTPS trust](trust/README.md). This data update adds no external TLS engine.
- Address-bar search with Google by default, Bing, DuckDuckGo and None; explicit
  URL/file intent is kept separate from search and navigation errors.
- HTML/DOM, a bounded JavaScript subset, events, CSS layout and software painting.
- Shared compatibility prerequisites include live Document/Node child lists,
  genuine Comment leaves, fresh text replacement and native array-like apply.
  Conventional select/option/optgroup fragments use the real native form model;
  whole-subtree removal and reconnection preserve identities and edited values,
  retire stale popup/focus/hits and maintain connected form submission order.
  Fresh hidden inputs and parsed hidden-input attributes, values and whole-tree
  moves now retain native form state. Rich select content, existing-control
  interior edits, new explicit select associations, arbitrary input fragments and
  existing input-type transitions remain unsupported.
- Static noncomputed object-literal `__proto__: value` setters preserve genuine
  null/object/function prototype links, ignore other primitives without coercion
  and reject duplicate special setters before execution. Ordinary own
  `__proto__` properties and legacy prototype access remain unsupported. These
  finite prerequisites do not establish framework initialization or interaction.
- Genuine Event, UIEvent, CustomEvent and finite MouseEvent construction,
  owner-realm prototypes and branded fields. CustomEvent retains actual any
  detail; Event/UIEvent/CustomEvent dispatch, cancellation and paths are real.
  A finite trusted anchor click/auxclick seam supplies actual host input and
  cancellation; native primary/middle link routing and browser-owned link context
  commands consume guarded source intents. Synthetic Mouse
  dispatch, broader native pointer payloads, KeyboardEvent construction and
  shadow-DOM retargeting remain unsupported.
- Settings, bookmarks, optional browser history, downloads and local diagnostics.
- Selected local IVF/VP8 video, PCM audio and constrained Matroska A/V.
  Visible fixed-canvas version-zero I/P video supports single-vector interframes
  and dependency-aware seeking; SPLITMV and hidden frames remain unsupported.
  One authored HTML video supports native controls, source replacement and real
  JavaScript load(), pause(), currentTime seeking and play() with pending promises
  tied to decoded output. Owner-realm HTMLMediaElement/HTMLVideoElement interfaces
  expose those existing controls and real state through canonical prototypes;
  illegal construction and unbound playback refusals do not create extra players.
  Boolean controls reflect actual attributes even on unbound elements. Changing
  the bound video's native bar preserves source, clock, frames and play promises.
  VIDEO width/height IDL reflection remains unsupported.
  Supported Matroska adds source-bound PCM and
  acknowledged sample time. Complete HTTP(S) transfers admit explicit IVF or
  Matroska MIME and preserve authored crossorigin/credential checks. Routine
  local and controlled HTTP/TLS A/V checks keep audio output closed. Separate
  opt-in WinMM checks verify acknowledged PCM advancement and video alignment,
  without claiming physical audibility. Unmarked foreign sources,
  dynamic crossOrigin controls, muted Matroska, streaming and multiple players
  remain unsupported.
- Replaceable main-window animation operations that schedule and cancel real
  callbacks. Child-frame schedulers are unsupported.
- Original local regression fixtures and App/input/offscreen interaction tests.

These are scoped foundations, not complete standards support. The
[capability matrix](docs/CAPABILITIES.md) separates implementation from evidence.
Google Search, Images and Maps are not fully supported. YouTube page compatibility
is limited, and playback is unsupported. Further F6/YouTube work is deferred.
The completed finite B2 source milestone adds a browser-owned tab strip and
shortcuts, real windows and tab-pinned inspectors. Original pure UI and
hidden-window controls cover ownership and lifetime. Native links support current,
foreground/background tabs and new windows with source-bound context commands.
Normal/optimized, hosted and scoped desktop acceptance identifies the released
source checkpoint. Named browsing contexts, script-opened windows and wider
accessibility remain gaps. Remaining TLS gaps stay explicit; perfect TLS coverage
is not required before useful page-usability work. The trust-data subset does not
establish new live-service compatibility.

## Build

Use Windows x64, Microsoft C++ build tools/Windows SDK, Rust **1.99.1 MSVC** and
Python **3.14.8** for the verification scripts. From an approved source checkout:

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
python tools/accept_license.py --show
python tools/accept_license.py --accept-license 34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749 --license-scope personal
.\Build.cmd
python tools/verify_offline.py
```

The acceptance command is an affirmative personal-scope choice; use
`--license-scope organization` only with the required organizational authority.
Read the terms before accepting. A missing or changed-text receipt blocks builds
and ordinary application execution; help and license viewing remain available.

Build.cmd writes ChezCat.exe beside the project. Run it with the accompanying
pages, assets and trust data. Python/Rust are development tools, not application
runtime dependencies. See [building and testing](docs/BUILDING.md).

## Development and reporting

See [architecture](docs/ARCHITECTURE.md), [roadmap](docs/ROADMAP.md),
[feature contracts](docs/FEATURE_CONTRACTS.md), [release process](docs/RELEASING.md),
[changes](CHANGELOG.md), [contribution policy](CONTRIBUTING.md) and
[security policy](SECURITY.md). Routine CI uses local fixtures; device, native
presentation and public-site checks are separate opt-in evidence.
The roadmap orders development, the capability matrix records implemented limits,
compatibility records measured workflows, and feature contracts define lasting
behavior and acceptance. Development continues through normal PR/CI integration;
new releases require an explicit owner request.

Document-only connection reports use `--diagnose URL --report FILE`. Optional
`--diagnose-allow-host HOST` entries restrict each destination before DNS; the
existing default redirect policy remains. Reports run no page scripts or
subresources and do not establish access or page usability. See the
[T1 diagnostic contract](docs/FEATURE_CONTRACTS.md).

## Licensing

ChezCat is source-available under [CSAPUL 1.1](LICENSE) for the releases designated
in [LICENSE_SCOPE.md](LICENSE_SCOPE.md). Personal and ordinary internal workplace
use, source review and permitted private modifications are allowed under its terms.
General redistribution, hosted access and independent commercial offerings remain
restricted; the exact license controls. This is not an OSI-approved open-source
license. Third-party terms and valid grants for earlier releases remain independent.

License SHA-256: `34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749`.

See [license acceptance](docs/LICENSE_ACCEPTANCE.md), [contributing](CONTRIBUTING.md),
[branding](docs/BRANDING.md) and [changes from 1.0](licensing/CHANGES-1.1.md).
