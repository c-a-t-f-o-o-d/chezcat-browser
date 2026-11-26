<div align="center">

# ChezCat documentation

*Find the behavior, evidence, workflow, or policy you need.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

## Start here

| Read | For |
| :--- | :--- |
| 🗺️ **[Roadmap](ROADMAP.md)** | Development priorities, milestones and future architecture |
| 🧱 **[HTML/XHTML foundation](HTML_FOUNDATION_PLAN.md)** | Active standards plan, package gates and [requirement register](HTML_FOUNDATION_COVERAGE.json) |
| **[CSS foundation](CSS_FOUNDATION_PLAN.md)** | Planned after HTML/XHTML F14: [source coverage and work packages](CSS_FOUNDATION_COVERAGE.json), [standards inventory](CSS_STANDARDS_INVENTORY.json) and acceptance gates |
| 📋 **[Capabilities](CAPABILITIES.md)** | Implemented slices and explicit gaps |
| 🧪 **[Compatibility](COMPATIBILITY.md)** | Actual pinned application outcomes, separated by stage |
| 🏗️ **[Architecture](ARCHITECTURE.md)** | Ownership, processes, resources and rendering boundaries |
| 🔨 **[Build guide](BUILDING.md)** | Prerequisites, acceptance, local builds and verification |
| 📦 **[Development and releases](RELEASING.md)** | Focused development checks, milestone acceptance and release verification |
| 📐 **[Feature contracts](FEATURE_CONTRACTS.md)** | Navigable index into the complete preserved acceptance requirements |

## Policies and provenance

| Read | Scope |
| :--- | :--- |
| [License acceptance](LICENSE_ACCEPTANCE.md) | Explicit offline choice and minimal local receipts |
| [Contributing](../CONTRIBUTING.md) | Reports, evaluation and maintained-inclusion requirements |
| [Contributor privacy](CONTRIBUTOR_PRIVACY.md) | Private legal records and public credit boundaries |
| [Security policy](../SECURITY.md) | Sensitive reports and experimental-browser limitations |
| [Branding](BRANDING.md) | Attribution, modified builds and permitted demonstrations |
| [Official branches and approvals](ENDORSEMENT_POLICY.md) | Requests, required scope and approval records |
| [Third-party notices](../THIRD_PARTY_NOTICES.md) | Separate licenses, data, runtimes and development inputs |
| [HTTPS trust](../trust/README.md) | Explicit trust stores and reviewed offline data refresh |
| [Public-suffix data](../data/public_suffix/README.md) | Pinned data, algorithm and freshness limitations |

## Source, evidence, and delivery

The README introduces the product. These pages serve different purposes: a planned
milestone is not implemented support, a source merge is not a new downloadable
release, and a component test is not a live-site outcome. Follow the exact source or
build identity attached to each result.

The verified downloadable baseline is
[v0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4),
from tagged source `122d0b7de5988493481dc4bed168d01969eef488`. Its Windows build,
release checks, four published assets and extracted runtime passed verification.
The [current release record](RELEASING.md#current-verified-release) links the
workflow and exact asset hashes; [release history](RELEASING.md#release-history)
preserves earlier deliveries and candidates.

**The document foundation is the active development priority.** F00 acceptance
covers the requirements and fixed baseline. F01, F02, F04 and F05 remain partial.
All 21 required browser files have been attempted; the
[coverage register](HTML_FOUNDATION_COVERAGE.json) records their measured results
and identifies files still missing a terminal report.

### Current reading path

| Question | Read next |
| :--- | :--- |
| **What should become usable next?** | The active [HTML/XHTML foundation](HTML_FOUNDATION_PLAN.md), then the [CSS program C00–C32](CSS_FOUNDATION_PLAN.md). The [P1–P6 capability packages](ROADMAP.md#current-product-sequence) cover shared DOM/components, layout/input, requests, history, resources/scrolling and original media, with explicit dependencies and acceptance evidence. |
| **What has changed in the engine?** | [Capabilities](CAPABILITIES.md) for current contracts and limits; the [changelog](../CHANGELOG.md#0820-alpha4) for the release-line summary. |
| **What has actually worked on a page?** | The [latest recorded homepage checkpoint](COMPATIBILITY.md#latest-reviewed-signed-out-homepage-checkpoint), [recorded original fixtures](COMPATIBILITY.md#recent-shared-increments) and [source-bound framework scorecard](COMPATIBILITY.md#current-scorecard). |
| **Which build should I run?** | [Build instructions](BUILDING.md) for local development; the [current release record](RELEASING.md#current-verified-release) for verified downloads and exact source identity. |

Named sites are representative compatibility tests; product milestones follow the
shared capability contracts and integration gates. The latest recorded YouTube
observation remains a skeleton on its measured source. Merged engine changes and
passing original fixtures are useful progress, but they do not establish a working
homepage, search journey or playback.

Fully accepted milestones proceed through the checks in the
[release procedure](RELEASING.md).
