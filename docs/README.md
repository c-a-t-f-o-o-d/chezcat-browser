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
release, and a model test is not a live-site outcome. Follow the exact source or
build identity attached to each result.

The verified downloadable baseline is
[v0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4),
from tagged source `122d0b7de5988493481dc4bed168d01969eef488`. Download and runtime
checks are recorded alongside earlier releases in the [release history](RELEASING.md#published-baseline).

The `v0.82.0-alpha.2` tag identifies source
[`14134ab`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/14134aba8ac2ab001c8ec13db77f68fa220b1400),
including integrated changes through PR #430 and release preparation in PR #431.
Its [change summary](../CHANGELOG.md#0820-alpha2) describes the source scope;
[Windows release qualification and verified delivery](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/runs/44971512445)
stopped at the normal native-test stage. No alpha.2 packages were published.
The subsequent [alpha.3 qualification](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/runs/45091960420)
at `ff5740d` also failed the Windows normal suite.
Its build and the four hosted capacity failures are recorded in the
[release history](RELEASING.md#published-baseline). The alpha.4 source includes
the adapter and capacity corrections. F00 acceptance covers requirements and
the fixed baseline. F01, F02 and F04 remain partial, and the required browser
groups await their harness. The [coverage register](HTML_FOUNDATION_COVERAGE.json)
tracks these distinctions.

### Current reading path

| Question | Read next |
| :--- | :--- |
| **What should become usable next?** | The [HTML/XHTML foundation plan](HTML_FOUNDATION_PLAN.md), followed by the YouTube shell/search/navigation/playback milestones. |
| **What has changed in the engine?** | [Capabilities](CAPABILITIES.md) for current contracts and limits; the [changelog](../CHANGELOG.md#0820-alpha4) for the release-line summary. |
| **What has actually worked on a page?** | The [latest homepage checkpoint](COMPATIBILITY.md#latest-reviewed-signed-out-homepage-checkpoint), [recent original fixtures](COMPATIBILITY.md#recent-shared-increments) and [source-bound framework scorecard](COMPATIBILITY.md#current-scorecard). |
| **Which build should I run?** | [Build instructions](BUILDING.md) for local development; [release history](RELEASING.md#published-baseline) for verified downloads and exact source identities. |

The current YouTube observation remains a skeleton. Merged engine changes and
passing original fixtures are useful progress, but they do not establish a
working homepage, search journey or playback.

Fully accepted milestones proceed through the checks in the
[release procedure](RELEASING.md).
