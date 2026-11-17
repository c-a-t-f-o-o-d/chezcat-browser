<div align="center">

# 🐱 ChezCat Browser — Development Roadmap

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.81.0-alpha.2&color=0e7490)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2)
[![Development focus](https://img.shields.io/static/v1?label=Focus&message=Web%20compatibility&color=6d28d9)](#active--upcoming-milestones)
[![Main verification](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)

*Delivering repeatable web page improvements through shared, spec-aligned browser capabilities.*

[Release horizon](#release-horizon) · [Active work](#active--upcoming-milestones) · [Architecture](#performance--architecture) · [Development protocol](#agent-execution-protocol) · [Backlog](#reference--backlog)

</div>

---

## Release Horizon

**Public source moves frequently. Named releases move only on owner request.**
Milestone IDs below describe work, not reserved versions, deadlines, or promises
of complete framework support. No date or version is assigned to the next release.

| Channel | What it contains | When it changes |
| :--- | :--- | :--- |
| **Published prerelease** | [0.81.0-alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2), source `36adf20`, released November 17, 2026 (UTC) | Explicit owner request; immutable tag and assets |
| **Public `main`** | Integrated development source; PRs [#29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29) and [#30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30) add behavior beyond alpha.2 | Coherent PRs after applicable integration checks |
| **Public feature branches** | Reviewable work in progress with its own check status | Meaningful implementation checkpoints, independently of releases |
| **Development binaries** | Planned CI artifacts identified by source commit, not another GitHub Release | Delivery work `DX-02`; not advertised as available yet |

```mermaid
flowchart LR
    BASE["Published alpha.2"] --> DOM["DOM / string foundation<br/>Merged: PR 29"]
    DOM --> URL["Anchor URL reflection<br/>Merged: PR 30"]
    URL --> INPUT["INPUT fragments + events<br/>Active compatibility work"]
    URL --> CSS["CSS variables + colors<br/>Active visual work"]
    INPUT --> APPS["Useful framework workflows"]
    CSS --> APPS
    APPS --> ISO["Sandboxed content processes<br/>Later readiness gate"]
    APPS --> GPU["Measured GPU acceleration<br/>Later readiness gate"]
```

This is a dependency and progress map, **not a calendar**. CSS and language/DOM
work proceed alongside one another. The diagram does not require isolation to
wait for complete framework or video support.

> [!IMPORTANT]
> **Owner-gated releases.** Completing a feature, merging a PR, or passing CI does
> not trigger a release. An explicit request is queued after the current coherent
> development task. CI/CD owns the requested build, package, and publication steps;
> a failed check blocks promotion or publication, not unrelated coding.

---

## Active & Upcoming Milestones

**Legend:** ✅ **Merged** · 🟡 **Under review** · ◻️ **Next / planned** · 🧊 **Deferred**

Snapshot: November 17, 2026, using public `main` at `9e5be72` and the linked PRs.
PR pages carry changing review and CI status. Branch-only results are not results
for `main` or for the published alpha.2 binary.

### DOM — Shared page behavior

**Goal:** Make standard DOM and language operations work across real library
workloads, without framework-specific substitutes.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| ✅ Merged | `DOM-01` | **String concat, hidden-input state, and genuine Node roots** | [PR #29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29) integrated the finite contracts. This is not whole-framework compatibility. |
| ✅ Merged | `DOM-02` | **Anchor HTTP(S) URL reflection** | [PR #30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30) integrated scoped live URL values. Wider URL semantics remain bounded by the contract. |
| 🟡 Under review | `DOM-03` | **Native text INPUT fragments and retained editor state** | [PR #32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32): real editing/reset, dirty/default values, neighboring controls, ownership, and atomic refusals. Branch results report modern jQuery startup passing; integration remains separate. |
| ◻️ Next | `DOM-04` | **Native Event own-property behavior** | Reproduce the trusted-input failure reported by PR #32; implement normal property lookup without weakening branded access. Recheck the unchanged modern jQuery input/timer workflow. |
| ◻️ Next | `DOM-05` | **DocumentFragment and subsequent legacy DOM boundaries** | Reduce the next failure reported by PR #32, then verify fragment insertion, tree identity, form state, and rollback. Preserve the unchanged legacy workload and any next blocker. |

### CSS — Visible layout and color correctness

**Goal:** Improve the actual pixels and hit targets of representative pages, not
only their initial HTML paint or absence of script errors.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| 🟡 Under review | `CSS-01` | **Inherited custom properties in padding and borders** | [PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31) reports a real App reduction changing from 80×30 to 104×46, matching an opaque literal control's pixels, hits, and reflow. Hosted verification still gates integration; no full Bootstrap styling claim. |
| ◻️ Investigate | `CSS-02` | **Alpha colors, including `rgba()`** | Establish the current parser/compositing boundary with an original reduction. Verify computed values, alpha composition, clipping, and painted output against a reference; do not assume every spelling is absent or supported. |
| ◻️ Ongoing | `CSS-03` | **Image, font, sizing, and input-layout defects** | Rank reproducible visible failures by shared impact; keep real dimensions, resource outcomes, paint, and hit testing consistent. Select the next finite scope from evidence. |

### JS & Frameworks — From startup to useful interaction

**Goal:** Progress from loading a library to exercising an unchanged application.
A passing startup probe is a step, not a working application.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| ◻️ Planned | `JS-01` | **Object-prototype access used by the Vue workload** | Reduce the recorded Vue 2.7.16 boundary and implement shared semantics with receiver, realm, ordering, and access checks. Recheck mount, input, and asynchronous updates separately. |
| ◻️ Planned | `JS-02` | **Class syntax and semantics used by modern applications** | Define a coherent parser/runtime slice from the recorded Bootstrap 5.3.9 failure. Test behavior, not merely parsing; preserve explicit limits for unfinished class features. |
| ◻️ Dependency-gated | `FW-01` | **One modern Vue or React application** | Pin an official distribution and compatible dependencies. First record a genuine baseline; completion requires mounting, a user-driven state change, visible DOM/pixel output, and relevant asynchronous behavior. |
| ◻️ Continuing | `FW-02` | **Modern and legacy cohort coverage** | Keep existing legacy samples and separate modern jQuery/Bootstrap cases. Add materially different generations when evidence makes them useful; never infer support for every version from one sample. |

Exact versions, build identities, stage results, and first failures belong in
[COMPATIBILITY.md](COMPATIBILITY.md) and [compatibility.json](compatibility.json).
Raw source line numbers remain in those evidence records, not in permanent
milestone names. Security defects and demonstrated shared dependencies can change
implementation order.

---

## Performance & Architecture

Keep the original Rust implementation and improve it incrementally. Do not start
a language rewrite, generic framework, or speculative parallel engine.

| State | ID | Milestone | Entry condition & finite outcome |
| :--- | :--- | :--- | :--- |
| ◻️ Incremental | `PERF-01` | **Representative performance baselines** | Reuse successful original fixtures. Record parse/script/style/layout/paint time, input responsiveness, and retained memory across repeated navigation, with exact build/configuration and repeatable conditions. Failed or incomplete execution is not a speed win. |
| 🧊 Later | `ISO-01` | **Sandboxed content processes** | Begin after representative load/render/input/navigation and selected media paths are repeatable, before positioning the browser for sensitive everyday use. Separate privileged browser services from restricted content; validate bounded messages, access refusals, crash containment, recovery, and resource limits. Multiple processes alone are not a sandbox. |
| 🧊 Later | `GPU-01` | **GPU compositing/presentation with software fallback** | Start from measured rendering costs. Keep original rendering commands, add a bounded platform backend, verify equivalent behavior, device-loss recovery, and a “Use hardware acceleration when available” setting with active/fallback diagnostics. Expand rasterization only with evidence. |
| 🧊 Separate decision | `GPU-02` | **Accelerated video presentation / decoding boundary** | Presenting our decoded frames is distinct from delegating decoding. No OS-codec fallback or replacement decoder is authorized by the graphics milestone; hardware decoding needs an explicit originality/dependency decision. |

Current reality remains [one browser process with worker threads and software
rendering](ARCHITECTURE.md). Isolation and GPU acceleration are **planned, not
implemented**. They are not prerequisites for the active compatibility fixes.
Neither depends on supporting every website first.

---

## Delivery — Frequent Source, Pipeline-Owned Builds

**Policy is agreed; the automation work below is not yet implemented.** Existing
branch protections and required checks remain in force during the transition.

| State | ID | Deliverable | Exit criteria |
| :--- | :--- | :--- | :--- |
| ◻️ Planned | `DX-01` | **Asynchronous CI/CD handoff** | Focused local checks precede a clean public source checkpoint. Hosted jobs own canonical integration/release checks and publish concise results. No routine agent polling or duplicate local delivery audit. The maintainer triages hosted failures; failing changes cannot promote or publish. |
| ◻️ Planned | `DX-02` | **Commit-identified development artifacts** | Successful trusted CI exposes a complete, clearly labeled development payload with its real source identity, required notices/data, no private material, and bounded retention. It is not a release, tag, or version bump. |
| ◻️ Proposed | `DX-03` | **Lightweight public `dev` integration channel** | Adopt only with explicit triggers, protected promotion, trusted identities, and no duplicate validation for unchanged inputs. Keep `main` current with promoted known-good work. Do not create another manually supervised release stage. |

For requested releases, freeze the selected revision and hand it to the existing
pipeline. Continue independent coding without mutating that snapshot. Pipeline
failure leaves delivery incomplete; it never justifies claiming a release passed.
See [RELEASING.md](RELEASING.md) for the existing checks and the handoff boundary.

---

## Agent Execution Protocol

> [!NOTE]
> **One practical development cycle:** reproduce a concrete failure, implement
> the shared fix, run focused affected and security-negative checks, inspect the
> changed workflow, then commit and submit it through the applicable PR/CI gates.
> Full regression belongs at integration boundaries—not after every small edit.

Public feature branches should receive coherent, reviewable progress frequently.
A named release is not required for a commit, PR, CI run, or development build.
Keep changed source distinct from merged source, and merged source distinct from
published binaries. A completed PR is a checkpoint, not the end of the task queue.

Documentation has one owner per concern:

| Document | Responsibility |
| :--- | :--- |
| **This roadmap** | Order, milestones, dependencies, and backlog |
| [CAPABILITIES.md](CAPABILITIES.md) | Implemented behavior and limits |
| [COMPATIBILITY.md](COMPATIBILITY.md) + [catalogue](compatibility.json) | Versioned, measured application outcomes |
| [FEATURE_CONTRACTS.md](FEATURE_CONTRACTS.md) | Lasting behavior and acceptance requirements |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current system boundaries |
| [RELEASING.md](RELEASING.md) | Owner-requested delivery procedure |

Update only documents whose facts change in the same coherent implementation PR.
Do not turn routine synchronization into a fresh audit of every document. Keep
production original Rust/std-only, preserve security boundaries, and retain
unmodified upstream workloads with appropriate notices. Test counts alone do
not establish browser compatibility.

---

## Reference & Backlog

<details>
<summary><strong>✅ Delivered foundations</strong> — published baseline and later source progress</summary>

| Delivery | Area | Finite scope |
| :--- | :--- | :--- |
| **alpha.2** | `B2` navigation | Native tabs/windows, guarded link gestures and context actions, independent page state |
| **alpha.2** | `A1` identity | About/build identity and offline license access; explicit acceptance remains its own contract |
| **alpha.2** | `CP1–CP5` prerequisites | Collection/Node, Comment/RegExp, responsive CSS, conventional select, and literal-prototype foundations |
| **alpha.2** | `T1 / B1` foundations | Scoped TLS/trust, dual-stack transport, stylesheet/resource and viewport behavior; broader coverage remains partial |
| **alpha.2** | Selected media/interfaces | Existing finite VP8/PCM/Matroska, Media/Video and event slices within their documented limits |
| **Post-alpha.2 source** | `DOM-01 / DOM-02` | PR #29 string/hidden-input/Node-root work and PR #30 anchor URL reflection; not retroactively part of alpha.2 |

Published assets remain immutable. Read the [capability matrix](CAPABILITIES.md)
for exclusions and the [changelog](../CHANGELOG.md) for source history.

</details>

<details>
<summary><strong>🧊 Deferred backlog</strong> — unscheduled, with no reserved release numbers</summary>

| Area | State & trigger |
| :--- | :--- |
| **Windows installation and updates** | Preserve one installer, About/license access, optional startup update check, “Check now,” and explicitly approved upgrade contracts. Portable ZIP remains the baseline; schedule after current browser foundations. |
| **Broader media and service playback** | Preserve working finite media slices. Streaming, wider codecs, ordinary YouTube playback, and broader media interfaces remain separate unproved work. |
| **Extended web platform** | Wider Unicode, ES modules, networking APIs, storage, workers, frames, accessibility, DOM interfaces, and font/layout coverage are selected by demonstrated shared dependencies. |
| **Additional element APIs** | VIDEO dimensions, Element.toggleAttribute, NamedNodeMap mutation, and other interfaces remain distinct contracts rather than implied framework support. |
| **Agent status automation** | Deferred; no new screenshot watcher, heartbeat service, or restart supervisor in the compatibility phase. |

</details>

*Dates document observations, not delivery promises. Named release versions are
chosen only for an owner-requested, eligible snapshot.*
