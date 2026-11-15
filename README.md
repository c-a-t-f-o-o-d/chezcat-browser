# ChezCat

An experimental Windows x64 browser with an original Rust engine: networking,
JavaScript, DOM, layout, rendering and selected media decoding. It does not embed
Chromium, Gecko, WebKit or an external JavaScript/codec runtime.

The current milestone is **0.79.0-alpha.2**, preserving the completed animation milestone. This is an experimental browser, without a
security certification. Web-platform support is partial. The renderer and interpreter run
in the browser process with worker threads, without an OS renderer sandbox.

The production Cargo manifest has no third-party dependencies. Rust's standard
library, Windows APIs, data, assets and development tools have their own roles
and licensing obligations; see [third-party notices](THIRD_PARTY_NOTICES.md).

## Scope

- HTTP(S), original TLS/certificate handling, memory-only session cookies,
  redirects and supported CSP/resource policies.
- Address-bar search with Google by default, Bing, DuckDuckGo and None; explicit
  URL/file intent is kept separate from search and navigation errors.
- HTML/DOM, a bounded JavaScript subset, events, CSS layout and software painting.
- Settings, bookmarks, optional browser history, downloads and local diagnostics.
- Selected local IVF/VP8-intra video, PCM audio and constrained Matroska A/V.
  One authored HTML video supports native controls, source replacement and real
  JavaScript load(). Script play/pause/seek and network media remain incomplete.
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
.\Build.cmd
python tools/verify_offline.py
```

Build.cmd writes ChezCat.exe beside the project. Run it with the accompanying
pages, assets and trust data. Python/Rust are development tools, not application
runtime dependencies. See [building and testing](docs/BUILDING.md).

## Development and reporting

See [architecture](docs/ARCHITECTURE.md), [roadmap](docs/ROADMAP.md),
[feature contracts](docs/FEATURE_CONTRACTS.md), [release process](docs/RELEASING.md),
[changes](CHANGELOG.md), [contribution policy](CONTRIBUTING.md) and
[security policy](SECURITY.md). Routine CI uses local fixtures; device, native
presentation and public-site checks are separate opt-in evidence.

## License

ChezCat is source-available under the ChezCat Source-Available Personal Use License (CSAPUL), Version 1.0 for the releases expressly designated under that license. It permits free personal use and ordinary internal workplace use, including private/internal customization, compilation, headless operation and test automation for one's own permitted tasks. Organizations may deploy permitted builds internally and engage contractors for that internal work. Preserve the original notices and clearly label unofficial modifications.

General redistribution, commercial products and ChezCat-specific commercial services are not granted, except for the license's expressly defined exceptions or a separate written agreement. GitHub's independently applicable platform rights remain separate. Officially endorsed branches and nonmonetized external hosted services require specific written approval; invitation-only access is not an exception. Monetized reviews and demonstrations and limited attributed explanatory excerpts are permitted. Third-party materials retain their own terms.

Licensing and contractual notices: legal@bitmadd.com. Sensitive vulnerabilities use the verified private route in [SECURITY.md](SECURITY.md), not public issues.

The full [LICENSE](LICENSE), its [release scope](LICENSE_SCOPE.md) and any separately signed approval control over this summary. Code contributions require the separate signed contribution-assignment process. ChezCat is experimental, provided as is, and is not offered as a security-audited daily driver.
