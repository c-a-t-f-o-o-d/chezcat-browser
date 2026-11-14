# ChezCat

An experimental Windows x64 browser with an original Rust engine: networking,
JavaScript, DOM, layout, rendering and selected media decoding. It does not embed
Chromium, Gecko, WebKit or an external JavaScript/codec runtime.

This is the **0.78 publication preview**, not a published release or a security
certification. Web-platform support is partial. The renderer and interpreter run
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
- Original local regression fixtures and App/input/offscreen interaction tests.

These are scoped foundations, not complete standards support. The
[capability matrix](docs/CAPABILITIES.md) separates implementation from evidence.
Google has worked in particular recorded/manual sessions; fresh-session behavior
is still unresolved. Images and Maps have separate gaps. YouTube playback is not
implemented, and a homepage title/HTTP success is not playback or browsing success.

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

## Licensing status

The intended model is **source-available**, under the proposed ChezCat
Source-Available Personal Use License (CSAPUL), Version 1.0. Its intended direction
permits inspection and private personal noncommercial use, modification and
compilation; commercial exploitation and general redistribution require separate
permission, with preserved attribution. Exact wording and unresolved uses still
require owner approval. This preview contains no operative project LICENSE and
makes no new project-wide grant. Third-party, prior, platform and statutory rights
remain independent. CSAPUL is not described as OSI-approved or SPDX-listed.

The license version is unrelated to the browser's pre-1.0 product version.
[History import](HISTORY_IMPORT.md) explains the filtered engineering lineage.
