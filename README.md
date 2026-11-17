<div align="center">

<img src="assets/chezcat-icon.png" width="128" height="128" alt="ChezCat browser icon">

# ChezCat Browser

### An original browser engine. Written in Rust. Built one shared capability at a time.

*From network bytes to page pixels—networking, JavaScript, DOM, layout,
rendering, and selected media decoding.*

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.81.0-alpha.2&color=0e7490&style=flat-square)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)
[![Windows x64](https://img.shields.io/static/v1?label=Platform&message=Windows%20x64&color=2563eb&style=flat-square)](docs/BUILDING.md)
[![Production Cargo dependencies](https://img.shields.io/static/v1?label=Production%20Cargo%20dependencies&message=0&color=6d28d9&style=flat-square)](docs/ARCHITECTURE.md)
[![Main verification](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)
[![License](https://img.shields.io/static/v1?label=License&message=CSAPUL%201.1&color=64748b&style=flat-square)](LICENSE)

**[📦 Download](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)** · **[🗺️ Roadmap](docs/ROADMAP.md)** · **[🔨 Build](#build)** · **[🧪 Compatibility](docs/COMPATIBILITY.md)** · **[📖 Documentation](#documentation)**

</div>

---

> [!WARNING]
> **Experimental browser—not yet a daily driver.** Modern websites can render
> incorrectly or fail to interact. Content runs in the browser process with worker
> threads, without an OS renderer sandbox. Passing CI is not a security
> certification or a claim of complete website support.

## Why ChezCat?

ChezCat is a Windows x64 browser built around its **own engine**, rather than an
embedded Chromium, Gecko, WebKit, or external JavaScript/codec runtime.

| 🦀 Original implementation | 🌐 Shared web capabilities | 🔬 Observable progress |
| :--- | :--- | :--- |
| Original Rust with **zero third-party production Cargo dependencies**. | Standards-aligned language, DOM, CSS, and resource behavior—not website-specific replacements. | Real page output, input, and asynchronous behavior, with exact limits and remaining failures recorded. |

Rust's standard library, Windows APIs, reviewed data, assets, and development
tools still have their own roles and licensing obligations. See
[third-party notices](THIRD_PARTY_NOTICES.md).

## What is here

| Area | Current foundation |
| :--- | :--- |
| 🖥️ **Native browser shell** | Tabs, multiple windows, link context actions, address search, bookmarks, settings, optional history, downloads, and About/build identity. |
| 🧩 **Web platform** | Original HTML/DOM, a bounded JavaScript interpreter, events, CSS layout, and software painting. Compatibility is growing through shared fixes. |
| 🔐 **Networking and trust** | Original HTTP(S), TLS/certificate handling, memory-only session cookies, redirects, and explicit resource/CSP boundaries. |
| 🎞️ **Selected media** | Original constrained VP8, PCM audio, and Matroska paths with finite playback interfaces—not general streaming or working YouTube playback. |
| 🧪 **Compatibility workloads** | Original regressions plus separate, unmodified legacy and modern framework fixtures. Loading, startup, rendering, input, and async results remain distinct. |

These are **implemented slices, not complete standards support**. The
[capability matrix](docs/CAPABILITIES.md) owns the detailed limits; the
[compatibility report](docs/COMPATIBILITY.md) records measured application outcomes.

---

## Choose your build

| You want… | Start here | What you are getting |
| :--- | :--- | :--- |
| **A fixed downloadable snapshot** | [📦 0.81.0-alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2) | A published prerelease with runtime/source ZIPs, notices, checksums, and its own source identity. |
| **Newer integrated development** | Public `main` + [build instructions](#build) | Source changes that may not exist in the published executable. Check the current CI result. |
| **Work under review** | [🔎 Open pull requests](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pulls) | Feature-branch work with its own results and remaining checks; not necessarily in `main`. |

> [!IMPORTANT]
> **New source does not update an old download.** Published alpha.2 assets remain
> unchanged. Named releases are created only on explicit owner request; ordinary
> commits, PRs, CI runs, and development builds do not require a release.

Commit-identified development downloads and a possible `dev` channel remain
roadmap items, not advertised delivery options. The CI badge reports workflow
status—not how well every website works.

## Development highlights

*Snapshot: November 17, 2026 · public source through `e97dc85`.*

| State | Improvement | Evidence and boundary |
| :--- | :--- | :--- |
| ✅ **Merged** | **Shared string and DOM primitives** | [PR #29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29): `String.prototype.concat`, hidden-input state, and genuine `Node.getRootNode()` behavior. |
| ✅ **Merged** | **Anchor URL reflection** | [PR #30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30): scoped live HTTP(S) URL values used by ordinary page code. |
| ✅ **Merged** | **Text-input fragments without lost edits** | [PR #32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32): staged native input creation, retained edited/default values, Reset behavior, and focus retirement. |
| 🟡 **Under review** | **CSS variables in padding and borders** | [PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31): inherited custom properties and measured box/pixel improvements; integration remains gated by verification. |

The pinned **jQuery 4.0.0 fixture now passes startup** in the PR #32 evidence;
trusted input still encounters a native Event-property gap. Legacy jQuery reaches
a separate `DocumentFragment` boundary. **Startup progress is not complete
framework compatibility.** Linked PRs and compatibility records carry the precise
scope and newer results.

### Next: useful pages, then measured speed and stronger isolation

| Now | Next foundations | Later, when ready |
| :--- | :--- | :--- |
| Shared DOM/events, JavaScript semantics, CSS variables/colors, and image/font/layout correctness. | Broader representative framework interactions and lightweight performance baselines. | Sandboxed content processes and measured GPU rendering with a software fallback. |

**[Explore the full roadmap →](docs/ROADMAP.md)** for milestone IDs, dependencies,
completion evidence, and deferred work. Isolation and GPU acceleration are planned,
not implemented; they do not block the current compatibility work.

---

## Inside the engine

```mermaid
flowchart LR
    INPUT["Network / local bytes"] --> PARSE["HTML + CSS parsing"]
    PARSE --> DOM["Document + styles"]
    JS["Original JavaScript + DOM APIs"] <--> DOM
    DOM --> LAYOUT["Style + layout"]
    LAYOUT --> PAINT["Software painting"]
    PAINT --> WINDOW["Native Windows presentation"]
```

*A conceptual view, not a complete scheduling or security diagram.* Browser-owned
services and document-owned work keep explicit lifetime, origin, and access
boundaries. The implementation currently uses **one Windows process with worker
threads**. See [architecture and boundaries](docs/ARCHITECTURE.md).

## Scope

<details>
<summary><strong>🖥️ Browser, navigation, and resources</strong></summary>

Native tabs/windows and guarded current, foreground/background-tab, and new-window
link actions are implemented. Address search supports Google by default, Bing,
DuckDuckGo, or None; explicit URL/file intent remains distinct. Settings,
bookmarks, optional history, downloads, and diagnostics are available within their
contracts. Named contexts, script-opened windows, and wider accessibility remain gaps.

Trust data contains a pinned, conservative **98-authority Mozilla NSS subset**
with provenance and explicit custom-store replacement. It is public data, not an
external TLS engine or a guarantee that every HTTPS site will connect. See
[HTTPS trust](trust/README.md) and [security](SECURITY.md).

</details>

<details>
<summary><strong>🧩 JavaScript, DOM, events, and rendering</strong></summary>

Finite shared primitives include live Document/Node child lists, genuine Comment
leaves, text replacement, native array-like apply, generic string concatenation,
and genuine roots for supported connected/detached trees. Conventional select
fragments and supported hidden/text-input operations preserve native form state.
Text input fragments retain edited/default values; unsupported input kinds and
ownership cases still reject without publishing partial state.

Static noncomputed object-literal prototype setters are supported; wider
prototype behavior and class semantics remain incomplete. Shadow DOM, arbitrary
input-type transitions, modules, and broader host APIs remain gaps.
Event/UIEvent/CustomEvent and finite MouseEvent interfaces preserve branded state;
broader synthetic dispatch and keyboard/pointer behavior remain limited.

The renderer is software-based. Responsive layout improvements are finite, and
passing a layout fixture does not establish full framework styling. Google Search,
Images, and Maps are not fully supported. Consult the
[capability matrix](docs/CAPABILITIES.md) and [compatibility results](docs/COMPATIBILITY.md).

</details>

<details>
<summary><strong>🎞️ Media and current playback limits</strong></summary>

Selected local IVF/VP8, PCM audio, and constrained Matroska A/V use original
decoders. Finite version-zero single-vector interframes, source replacement,
seeking, native controls, and selected script playback operations are supported
under their contracts. Supported network media uses bounded complete responses
with explicit source and credential policies.

Wider interframe modes, hidden frames, streaming, multiple players, dynamic
crossOrigin controls, and full HTMLMediaElement behavior remain outside the
implemented scope. **Ordinary YouTube playback is unsupported.** Controlled
output checks do not establish physical audibility or general device compatibility.

</details>

---

## Build

### 1 · Prepare Windows x64

Use Microsoft C++ build tools/Windows SDK, **Rust 1.99.1 MSVC**, and **Python 3.14.8**
for verification scripts. From your source checkout:

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
python tools/accept_license.py --show
```

### 2 · Read the terms, then build

The following records an affirmative **personal-scope** acceptance. Run it only
after reading and deciding to accept the exact terms:

```powershell
python tools/accept_license.py --accept-license 34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749 --license-scope personal
.\Build.cmd
```

Use `--license-scope organization` only with the required organizational authority.
Missing or changed-text acceptance blocks builds and ordinary execution; help and
license viewing remain available.

### 3 · Run with the matching resources

`Build.cmd` writes **`ChezCat.exe`** beside the project. Keep its accompanying
pages, assets, trust data, and notices together. Rust and Python are development
tools, not application runtime dependencies.

<details>
<summary><strong>🧪 Verification and development checks</strong></summary>

Use focused affected tests while editing. The complete integration check is:

```powershell
python tools/verify_offline.py
```

Existing local and hosted integration gates remain in place. The development
agent investigates failed checks and fixes the affected work; failures are not
silently ignored or passed back to the maintainer by default. No new asynchronous
handoff or branch-promotion system is enabled by this documentation.

Routine CI uses local fixtures. Physical desktop, audio-device, and public-site
observations are separate evidence. See [building and testing](docs/BUILDING.md)
and the [release procedure](docs/RELEASING.md).

</details>

## Documentation

| Explore | What you will find |
| :--- | :--- |
| 🗺️ [**Roadmap**](docs/ROADMAP.md) | Merged work, current milestones, dependencies, and future architecture. |
| 📋 [**Capabilities**](docs/CAPABILITIES.md) | Implemented behavior, explicit limits, and unsupported areas. |
| 🧪 [**Compatibility**](docs/COMPATIBILITY.md) | Versioned framework/workflow observations and first remaining blockers. |
| 🏗️ [**Architecture**](docs/ARCHITECTURE.md) | Component ownership, execution, rendering, and security boundaries. |
| 📐 [**Feature contracts**](docs/FEATURE_CONTRACTS.md) | Lasting behavior and acceptance requirements. |
| 🔧 [**Build guide**](docs/BUILDING.md) | Toolchain, source builds, and verification. |
| 📦 [**Release procedure**](docs/RELEASING.md) | Delivery for explicitly requested releases. |
| 📝 [**Changelog**](CHANGELOG.md) | Recorded source history. |

## Development and reporting

Development uses coherent public commits and PRs without waiting for a release.
A completed PR is a checkpoint, not the end of the development queue. Keep the
current verification-and-repair workflow; broader CI handoff changes are deferred.

For bug reports and contributions, read [CONTRIBUTING.md](CONTRIBUTING.md). For
security concerns, follow [SECURITY.md](SECURITY.md).

<details>
<summary><strong>🔎 Connection-only diagnostics</strong></summary>

Use `--diagnose URL --report FILE`; optional `--diagnose-allow-host HOST` entries
restrict destinations before DNS. These reports run no page scripts or
subresources and do not establish page usability. See the
[T1 contract](docs/FEATURE_CONTRACTS.md).

</details>

---

## Licensing

ChezCat is **source-available under [CSAPUL 1.1](LICENSE)** for the releases
designated in [LICENSE_SCOPE.md](LICENSE_SCOPE.md). Personal and ordinary internal
workplace use, source review, and permitted private modifications are allowed
under its terms. General redistribution, hosted access, and independent commercial
offerings remain restricted; the exact license controls.

**This is not an OSI-approved open-source license.** Independent third-party terms
and valid grants for earlier releases remain unchanged.

<details>
<summary><strong>License identity and related policies</strong></summary>

License SHA-256: `34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749`.

[License acceptance](docs/LICENSE_ACCEPTANCE.md) · [Third-party notices](THIRD_PARTY_NOTICES.md) · [Branding](docs/BRANDING.md) · [Changes from 1.0](licensing/CHANGES-1.1.md)

</details>

<div align="center">

**An original engine. Measured progress.**

[Roadmap](docs/ROADMAP.md) · [Build guide](docs/BUILDING.md) · [Compatibility](docs/COMPATIBILITY.md)

</div>
