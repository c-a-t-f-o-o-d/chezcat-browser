# ChezCat

An experimental Windows x64 browser with an original Rust engine: networking,
JavaScript, DOM, layout, rendering and selected media decoding. It does not embed
Chromium, Gecko, WebKit or an external JavaScript/codec runtime.

The development candidate is **0.80.0-alpha.1**, whose milestone is explicit
offline CSAPUL 1.1 acceptance. It also includes the completed finite navigation,
media, background-image and event slices listed below. This is an experimental
browser without a security certification. Web-platform support is partial. The
renderer and interpreter run in the browser process with worker threads, without
an OS renderer sandbox.

The production Cargo manifest has no third-party dependencies. Rust's standard
library, Windows APIs, data, assets and development tools have their own roles
and licensing obligations; see [third-party notices](THIRD_PARTY_NOTICES.md).

## Scope

- Full offline native license viewing and explicit acceptance, with source and
  headless commands, personal/organization scopes and minimal external receipts.
  Decline or invalid acceptance blocks browsing and source builds.
- HTTP(S), original TLS/certificate handling, memory-only session cookies,
  redirects and supported CSP/resource policies.
- Address-bar search with Google by default, Bing, DuckDuckGo and None; explicit
  URL/file intent is kept separate from search and navigation errors.
- HTML/DOM, a bounded JavaScript subset, events, CSS layout and software painting.
- Genuine Event, UIEvent and finite MouseEvent construction, owner-realm
  prototypes and branded fields. Event/UIEvent dispatch, cancellation and paths
  are real; Mouse dispatch/native pointer payloads, KeyboardEvent/CustomEvent
  constructors and shadow-DOM retargeting remain unsupported.
- Settings, bookmarks, optional browser history, downloads and local diagnostics.
- Selected local IVF/VP8 video, PCM audio and constrained Matroska A/V.
  Visible fixed-canvas version-zero I/P video supports single-vector interframes
  and dependency-aware seeking; SPLITMV and hidden frames remain unsupported.
  One authored HTML video supports native controls, source replacement and real
  JavaScript load(), pause(), currentTime seeking and play() with pending promises
  tied to decoded output. Supported Matroska adds source-bound PCM and
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
is limited, and playback is unsupported.

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
