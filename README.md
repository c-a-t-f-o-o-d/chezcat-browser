<div align="center">

# 🐱 ChezCat Browser

**An original browser engine, written in Rust.**

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.81.0-alpha.2&color=0e7490)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)
[![Windows x64](https://img.shields.io/static/v1?label=Platform&message=Windows%20x64&color=2563eb)](docs/BUILDING.md)
[![Main verification](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)
[![License](https://img.shields.io/static/v1?label=License&message=CSAPUL%201.1&color=64748b)](LICENSE)

*From network bytes to page pixels—original networking, JavaScript, DOM, layout,
rendering, and selected media decoding.*

[Download alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2) · [Roadmap](docs/ROADMAP.md) · [Capabilities](docs/CAPABILITIES.md) · [Compatibility](docs/COMPATIBILITY.md) · [Build](#build) · [Security](SECURITY.md)

</div>

---

> [!WARNING]
> **Experimental, not a daily-driver security claim.** Web-platform support is
> partial. The renderer and interpreter currently share the browser process with
> worker threads; there is no OS renderer sandbox. Many modern pages do not yet
> render or interact correctly. Passing CI does not establish complete site support.

## Why ChezCat?

ChezCat explores building a small Windows browser from its own implementation,
not embedding Chromium, Gecko, WebKit, or an external JavaScript/codec runtime.
The production Cargo manifest has **zero third-party dependencies**. Rust's
standard library, Windows APIs, reviewed data, assets, and development tools have
separate roles and licensing obligations; see [third-party notices](THIRD_PARTY_NOTICES.md).

| Foundation | What is here | What to expect |
| :--- | :--- | :--- |
| **Browser shell** | Native tabs/windows, address search, bookmarks, settings, history, downloads, and About/build identity | Useful finite workflows; wider accessibility and script-opened/named contexts remain gaps |
| **Original web platform** | HTML/DOM, a bounded JavaScript interpreter, CSS layout, and software painting | Shared standards-aligned improvements, not library-specific replacements |
| **Networking and trust** | Original HTTP(S)/TLS/certificate handling, session cookies, redirects, and guarded resources | Explicit trust/origin/CSP boundaries; broader protocol and site coverage remain partial |
| **Selected media** | Original constrained VP8/PCM/Matroska paths and finite Media/Video interfaces | Not general streaming, all codecs, or working YouTube playback |
| **Measured compatibility** | Original regressions and pinned legacy/modern framework applications | Load, initialization, rendering, input, and async results are reported separately |

## Where Development Is Going

**Make representative pages work, then make them faster and more isolated.**

The current focus is shared DOM/input behavior, JavaScript semantics, CSS variables
and colors, and image/font/layout correctness. The [roadmap](docs/ROADMAP.md) shows
merged work, review-stage changes, and the next measured dependencies. It also
records performance baselines, sandboxed content processes, and GPU acceleration
with software fallback as future work—not features available today.

### Published build vs. development source

| Use | Choose | Meaning |
| :--- | :--- | :--- |
| A fixed downloadable snapshot | [**0.81.0-alpha.2**](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2) | Published prerelease with its own source revision, notices, checksums, and runtime/source ZIPs |
| Integrated development progress | Public `main` | Newer source may contain changes absent from the latest release; inspect its current CI result |
| Work under review | [Open pull requests](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pulls) | Feature-branch results are not a promise that `main` or the published binary includes them |

PRs [#29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29) and
[#30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30) integrated string/DOM
prerequisites and scoped anchor URL reflection **after** alpha.2. They do not make
the existing alpha.2 downloads newer. Completing a milestone does not trigger a
release; new named releases require an explicit owner request.

The main-branch badge reports GitHub Actions status, not full browser compatibility.
Development binary artifacts and a lightweight `dev` promotion channel are planned
in the roadmap; this page does not claim either is operational.

## Scope

The [capability matrix](docs/CAPABILITIES.md) is the authority for implemented
behavior and limits. The [compatibility report](docs/COMPATIBILITY.md) records exact
library versions and measured application outcomes. Neither an HTTP 200 response,
a successful parse, nor a green unit-test count establishes a working website.

<details>
<summary><strong>Explore the finite implementation scope</strong></summary>

**Browser and resources.** Native tab/window navigation and guarded current,
foreground/background-tab and new-window link actions are implemented. Address
search supports Google by default, Bing, DuckDuckGo, or None; explicit URL/file
intent remains distinct. The browser includes settings, bookmarks, optional
history, downloads, and diagnostics. Browser-owned trust data contains a pinned,
conservative 98-authority Mozilla NSS subset with provenance and explicit
custom-store replacement; see [HTTPS trust](trust/README.md). It is data, not an
external TLS engine or a promise of universal TLS compatibility.

**DOM and JavaScript.** Finite prerequisites include live Document/Node child
lists, genuine Comment leaves, text replacement, native array-like apply, generic
String.prototype.concat, and genuine Node.getRootNode behavior for supported
connected/detached trees. Hidden-input state and conventional select fragments
preserve native form ownership within their contracts. Static noncomputed
object-literal prototype setters are supported; wider prototype behavior remains
partial. Scoped anchor HTTP(S) reflection is integrated in development source.
Shadow DOM, arbitrary input fragments/type transitions, and broader host APIs
remain gaps or work under review; see the current compatibility evidence.

**Events and layout.** Genuine Event/UIEvent/CustomEvent and finite MouseEvent
interfaces preserve branded state and ownership. Supported trusted link events
and native context commands use source-bound intents. Synthetic mouse dispatch,
broader pointer/keyboard interfaces, and shadow retargeting remain limited.
The current renderer is software-based. Responsive layout improvements are finite;
a passing layout fixture does not imply complete framework styling.

**Media.** Supported local IVF/VP8, PCM audio, and constrained Matroska A/V paths
use original decoders. Finite version-zero single-vector interframes, source
replacement, seeking, native controls, and selected script playback operations
are supported under their contracts. Supported network media uses bounded complete
responses, with explicit source/credential policies. Wider interframe modes,
hidden frames, streaming, multiple players, dynamic crossOrigin controls, and
complete HTMLMediaElement behavior remain outside the implemented scope.
Ordinary YouTube playback is unsupported. Controlled audio/output checks do not
establish physical audibility or general device compatibility.

**Evidence and acceptance.** Original model, App/input/offscreen, native desktop,
and live-service observations have different scopes. Google Search, Images, and
Maps are not fully supported. Current build/source identity and declared limits
matter more than a broad supported/unsupported label.

</details>

## Build

Use **Windows x64**, Microsoft C++ build tools/Windows SDK, **Rust 1.99.1 MSVC**,
and **Python 3.14.8** for verification scripts. From an approved source checkout:

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
python tools/accept_license.py --show
```

Read the terms. The following is an affirmative **personal-scope** acceptance
command; run it only after deciding to accept:

```powershell
python tools/accept_license.py --accept-license 34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749 --license-scope personal
.\Build.cmd
```

Use `--license-scope organization` only with the required organizational authority.
A missing or changed-text receipt blocks builds and ordinary execution; help and
license viewing remain available. `Build.cmd` writes `ChezCat.exe` beside the
project. Run it with the accompanying pages, assets, and trust data. Rust and
Python are development tools, not application runtime dependencies.

For the complete integration check:

```powershell
python tools/verify_offline.py
```

Use focused affected tests during ordinary edits. See [building and testing](docs/BUILDING.md)
for prerequisites and the distinction between development checks and delivery.

## Development and Reporting

| Start here | What it answers |
| :--- | :--- |
| [Roadmap](docs/ROADMAP.md) | What is merged, active, next, or deferred? |
| [Capabilities](docs/CAPABILITIES.md) | What behavior is implemented, and what are its limits? |
| [Compatibility](docs/COMPATIBILITY.md) | What actually happened in the pinned application workflows? |
| [Architecture](docs/ARCHITECTURE.md) | Where are ownership, execution, rendering, and security boundaries? |
| [Feature contracts](docs/FEATURE_CONTRACTS.md) | Which behavior and acceptance requirements must be preserved? |
| [Release process](docs/RELEASING.md) | How are explicitly requested builds validated and published? |
| [Changelog](CHANGELOG.md) | What changed in the source? |

Routine hosted checks use local fixtures. Native presentation, physical audio,
and public-site observations remain separate evidence. Development uses coherent
public commits and PRs without waiting for a release. See the roadmap's delivery
work for the planned CI handoff and development-artifact improvements.

For bug reports and contributions, read [CONTRIBUTING.md](CONTRIBUTING.md); for
security concerns, use [SECURITY.md](SECURITY.md). Document-only connection reports
use `--diagnose URL --report FILE`; optional `--diagnose-allow-host HOST` entries
restrict destinations before DNS. Those reports run no page scripts/subresources
and do not establish page usability. See the [T1 contract](docs/FEATURE_CONTRACTS.md).

## Licensing

ChezCat is **source-available under [CSAPUL 1.1](LICENSE)** for the releases
designated in [LICENSE_SCOPE.md](LICENSE_SCOPE.md). Personal and ordinary internal
workplace use, source review, and permitted private modifications are allowed
under its terms. General redistribution, hosted access, and independent commercial
offerings remain restricted; the exact license controls.

**This is not an OSI-approved open-source license.** Independent third-party terms
and valid grants for earlier releases remain unchanged.

License SHA-256: `34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749`.

[License acceptance](docs/LICENSE_ACCEPTANCE.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [Branding](docs/BRANDING.md) · [Changes from 1.0](licensing/CHANGES-1.1.md)
