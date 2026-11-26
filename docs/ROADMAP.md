<div align="center">

# 🐱 ChezCat Browser — Development Roadmap

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.82.0-alpha.4&color=0e7490)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4)
[![Development focus](https://img.shields.io/static/v1?label=Focus&message=Web%20compatibility&color=6d28d9)](#active--upcoming-milestones)
[![Main source privacy and configuration](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)

*An original browser, developed through shared web standards and usable browser capabilities.*

[Releases](#release-horizon) · [Milestones](#active--upcoming-milestones) · [Architecture](#performance--architecture) · [Documentation](#verification-and-documentation)

</div>

---

## Release Horizon

| Channel | Current position |
| :--- | :--- |
| **Published prerelease** | [0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4) is the verified Windows release. It delivers the F00 requirements and test baseline; the HTML/XHTML foundation remains in progress. |
| **Development source** | Public `main` includes later implementation work. See [capabilities and limitations](CAPABILITIES.md) for supported behavior. |
| **Development binaries** | Commit-identified CI downloads are planned. They are not currently advertised as available. |

Release downloads, verification and earlier versions are documented in [Releasing](RELEASING.md#current-verified-release). Milestone IDs describe scope and dependencies, with no fixed delivery dates or reserved version numbers.

## Active & Upcoming Milestones

**Current priority: the HTML/XHTML document foundation.** Work is progressing across document identity, DOM operations, encoding and HTML parsing. The initial requirements/test baseline is verified; the remaining foundation packages have open implementation or integration requirements.

### Active foundation sequence

| Stage | Browser capability |
| :--- | :--- |
| **F00–F03 · Shared foundations** | Requirements baseline, document/namespace model, encoding, MIME handling and standard URLs |
| **F04–F05 · HTML parsing** | Tokenization, tree construction, error recovery, entities, foreign content and templates |
| **F06–F08 · DOM and XHTML** | Native DOM interfaces, fragments, serialization, XML and XHTML |
| **F09–F11 · Document behavior** | HTML elements, forms, scripts, resources, navigation, focus and editing |
| **F12–F13 · Components and presentation** | Native Custom Elements, Shadow DOM, default styles and required text/rendering behavior |
| **F14 · Foundation qualification** | Integrated document behavior and required browser journeys |

This sequence builds the shared document behavior needed by later browser capabilities. Existing partial implementations remain useful while the full contracts are completed; [feature contracts](FEATURE_CONTRACTS.md) summarize the expected product behavior.

The broader CSS foundation program follows F14 and covers syntax, selectors, cascade, values, layout, painting, CSSOM and animation. Independent CSS improvements may proceed earlier; CSS needed by an HTML capability remains a prerequisite of that capability. JavaScript language/runtime coverage and the wider web platform retain their own requirements, including dependencies needed by the document foundation.

### Current product sequence

These integration milestones build on the document foundation and their required CSS and platform capabilities. A milestone qualifies when its required behavior works together in the browser. Individual website and framework results are recorded in [Compatibility](COMPATIBILITY.md).

| Milestone | Current position | Next capability |
| :--- | :--- | :--- |
| **P1 · DOM and components** | Existing partial implementation; broader integration follows the foundation | Dynamic documents and native controls that preserve identity, focus, observers and resource behavior |
| **P2 · Layout, CSSOM and input** | Partial | Responsive layouts, accurate geometry, consistent rendering and usable pointer/keyboard interaction |
| **P3 · Requests and response bodies** | Partial | Fetch, broader response types, streaming and cross-origin request behavior |
| **P4 · History and navigation state** | Partial | Structured history state, traversal events and reliable application navigation |
| **P5 · Resources and scrolling** | Planned integration; image/font decoders and selected geometry already exist | Resource updates, scrolling and hit targets that stay consistent with layout |
| **P6 · Streaming media and playback** | Planned integration; finite VP8/PCM/Matroska playback already exists | Streaming APIs and a verified original audio/video path with synchronized playback and seeking |

Required dependencies determine readiness. Specialized CSS features and separately gated platform programs retain their own acceptance boundaries.

## Performance & Architecture

ChezCat uses an original Rust/std-only implementation with no third-party browser or JavaScript engine. The current architecture uses one browser process, worker threads and software rendering. See [Architecture](ARCHITECTURE.md) for the implemented boundaries.

| Direction | Status |
| :--- | :--- |
| **Performance baselines** | Planned measurements of representative page loading, rendering, input and memory use |
| **Sandboxed content processes** | Planned after representative browser paths are repeatable, before sensitive everyday use |
| **GPU compositing and presentation** | Planned, with software fallback and recovery requirements |
| **Hardware video decoding** | Separate future decision; original software playback remains the active media direction |

Current security boundaries remain required throughout development. Process isolation and GPU acceleration are not yet implemented.

## Delivery — Frequent Source, Verified Builds

Implementation reaches public `main` through reviewed changes. Accepted milestones proceed to release preparation and source, package and download verification. A source update does not by itself create a new verified binary.

Commit-identified development downloads and Windows installation/update support remain planned. Published release assets retain their original source identity and verification record.

## Verification and documentation

This roadmap summarizes direction and major milestones. Detailed supported behavior, standards coverage and measured results have dedicated references:

| Reference | Contents |
| :--- | :--- |
| [Capabilities](CAPABILITIES.md) | Implemented browser behavior and current limits |
| [Standards and fixture inventory](HTML_FOUNDATION_COVERAGE.json) | Pinned standards inputs and inventories used by the test tooling |
| [Compatibility](COMPATIBILITY.md) | Measured website and framework outcomes, tied to their tested versions |
| [Feature contracts](FEATURE_CONTRACTS.md) | Lasting behavior requirements |
| [Releasing](RELEASING.md) and [changelog](../CHANGELOG.md) | Verified builds, release history and source changes |

Partial implementation and historical test results do not establish complete standards support. Compatibility claims apply to the source and workload actually measured.

## Reference & Backlog

Later work includes broader storage, workers and messaging; additional graphics, font and media capabilities; accessibility coverage; and installation/update support. Each area depends on the required browser foundations and its own scope and verification.

*The roadmap describes development direction. Dates and compatibility records document observations, not delivery promises.*
