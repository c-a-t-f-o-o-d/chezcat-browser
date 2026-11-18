<div align="center">

# 🐱 ChezCat Browser — Development Roadmap

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.81.0-alpha.3&color=0e7490)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.3)
[![Development focus](https://img.shields.io/static/v1?label=Focus&message=Web%20compatibility&color=6d28d9)](#active--upcoming-milestones)
[![Main verification](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)

*Delivering repeatable web page improvements through shared, spec-aligned browser capabilities.*

[Release horizon](#release-horizon) · [Active work](#active--upcoming-milestones) · [Architecture](#performance--architecture) · [Development protocol](#agent-execution-protocol) · [Backlog](#reference--backlog)

</div>

---

## Release Horizon

**Public source moves frequently. Named releases move only on owner request.**
Milestone IDs below describe work, not reserved versions, deadlines, or promises
of complete framework support. The authorized preparation target is `v0.82.0-alpha.1`.

| Channel | What it contains | When it changes |
| :--- | :--- | :--- |
| **Previous prerelease** | [0.81.0-alpha.3](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.3), source `57cc1d5` | Explicit owner request and immutable tag and assets |
| **Public `main`** | Integration baseline `6089546` includes the iframe interface, radio clones and implicit grid rows. Earlier PRs [#29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29), [#30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30), and [#32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32) add behavior beyond alpha.2 | Coherent PRs after applicable integration checks |
| **Public feature branches** | Reviewable work in progress with its own check status | Meaningful implementation checkpoints, independently of releases |
| **Development binaries** | Planned CI artifacts identified by source commit, not another GitHub Release | Delivery work `DX-02`; not advertised as available yet |

```mermaid
flowchart LR
    BASE["Previous release: alpha.3"] --> DOM["DOM / language prerequisites<br/>Integrated through PR 114"]
    DOM --> URL["HTML interfaces and attributes"]
    URL --> INPUT["Native radio clones and iframe identity<br/>Next: raw HTML attributes and contentEditable"]
    URL --> CSS["CSS variables and implicit grid rows"]
    INPUT --> APPS["Measured Vue, jQuery and Bootstrap apps"]
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
> development task. Keep the existing verification workflow: the agent investigates
> failed checks and fixes affected work. A failed gate blocks promotion or
> publication; unrelated authorized coding can continue.

---

## Active & Upcoming Milestones

**Legend:** ✅ **Merged** · 🟡 **Under review** · ◻️ **Next / planned** · 🧊 **Deferred**

Current integration baseline: `6089546`, November 18, 2026. The two active compatibility blockers are the raw `onfocusin` attribute on a DIV in legacy jQuery and the `contentEditable` read before ReactDOM mounts. Single-column grid rows are integrated. Per-item inline alignment remains a layout follow-up. [Current source-bound scorecard](COMPATIBILITY.md)

Earlier snapshot: November 17, 2026, using public `main` at `e97dc85` and the linked PRs.
DOM-06 includes measurements on `3456e67`, `5c89eea` and `5c22afd`. These changes
are included in the combined `main` recheck on `25654fa`.
The later selector candidate `02cd0f8` is integrated in `main` at `991cba3`.
Computed-style candidate `63a2259` is integrated in `main` at `5c21400` through PR #62.
PR #64/#65 measurements on `a82267e` and `7b8a5fb` are integrated by `main` `bf0c737`.
PR #67/#68 add Slottable and HTML-container object behavior. The `13d7dae`
interaction captures identify the feature candidate. Candidate and integrated
`main` `5309a68` CI each pass 2,695 normal tests and 25 journeys.
PR #69/#71 integrate computed `position` and layout-backed `offsetParent`.
Measured builds `c74503c` and `96666d3` retain open-menu and trusted Amber-choice
evidence at 1280/640. The menu remains visible after selection.
Those captures remain historical. Integrated source
`ec7db2f61cddb78c457b2966b72e1e362b89e66a` includes PR #74/#75/#76 and passes the
unchanged dropdown's trusted open, Amber selection and close at both widths,
with paired DOM/pixels, responsive cards and zero script errors. A separate
original native App probe on this source reaches missing `WeakMap` at 11:92.
PR pages carry changing review and CI status. Branch-only results are not results
for `main` or for the published alpha.2 binary.

### DOM — Shared page behavior

**Goal:** Make standard DOM and language operations work across real library
workloads, without framework-specific substitutes.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| ✅&nbsp;Merged | <code>DOM&#8209;01</code> | **String concat, hidden-input state, and genuine Node roots** | [PR #29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29) integrated the finite contracts. This is not whole-framework compatibility. |
| ✅&nbsp;Merged | <code>DOM&#8209;02</code> | **Anchor HTTP(S) URL reflection** | [PR #30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30) integrated scoped live URL values. Wider URL semantics remain bounded by the contract. |
| ✅&nbsp;Merged | <code>DOM&#8209;03</code> | **Native text INPUT fragments and retained editor state** | [PR #32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32) integrated real editing/reset, dirty/default values, neighboring controls, ownership, atomic refusals, and focus retirement. Its pinned modern jQuery fixture passed startup on that source. PR #34 later cleared the interaction boundary. |
| ✅&nbsp;Merged | <code>DOM&#8209;04</code> | **Native Event own-property behavior** | PR #34 clears the trusted-input failure reported by PR #32. The unchanged jQuery 4.0.0 input and timer workflow passes on `067ac5e`. |
| ◻️&nbsp;Continuing | <code>DOM&#8209;05</code> | **DocumentFragment and native form controls** | Legacy jQuery creates and appends its checked radio on `bfc6b26`. That build stopped at 4509:23. PR #115 adds native radio cloning. On `7608f4c` the next W2 failure is the `onfocusin` attribute write at 4743:4. [Radio inputs](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/113) |
| ✅&nbsp;Merged | <code>DOM&#8209;06</code> | **Element selectors, siblings, Slottable and NodeList iteration** | Bootstrap initializes at 1280/640. [PR #60](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/60) clears its dropdown selector boundary on measured `02cd0f8`, integrated at `991cba3`. [PR #67](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/67) adds guarded Element/Text `assignedSlot` reads for ordinary trees. Measured `a6a2619` then reaches `node.toString()` at 1736:9. Shadow DOM remains unsupported. |
| ✅&nbsp;Merged | <code>DOM&#8209;07</code> | **Ordinary DOM object stringification** | [PR #64](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/64)'s BUTTON/UL slice advanced `a82267e` to computed overflow. [PR #68](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/68) extends HTML-container interfaces. On `13d7dae` at 1280/640, the menu visibly opens and its trusted amber-choice callback runs. The menu stays open. The first microtask fault is computed `position` at 1990:5. |
| ✅&nbsp;Merged | <code>DOM&#8209;08</code> | **Genuine Node containment** | [PR #76](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/76) adds inclusive physical Node ancestry with Attr and receiver/origin distinctions. Integrated `ec7db2f` completes the pinned Bootstrap open/choice/close workflow at 1280/640. |

### CSS — Visible layout and color correctness

**Goal:** Improve the actual pixels and hit targets of representative pages, not
only their initial HTML paint or absence of script errors.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| ✅&nbsp;Merged | <code>CSS&#8209;01</code> | **Inherited custom properties in padding and borders** | [PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31) reports a real App reduction changing from 80×30 to 104×46, matching an opaque literal control's pixels, hits, and reflow. The measured scope covers the original opaque literal control. |
| ✅&nbsp;Merged slice | <code>CSS&#8209;02</code> | **Alpha colors, including `rgba()`** | PR #85 integrates translucent uniform rounded borders. On `8ebf051`, 144 exterior corner samples are white and the straight edges retain their original pixels at both widths. |
| ◻️&nbsp;Ongoing | <code>CSS&#8209;03</code> | **Image, font, sizing, and input-layout defects** | Rank reproducible visible failures by shared impact; keep real dimensions, resource outcomes, paint, and hit testing consistent. Select the next finite scope from evidence. |
| ✅&nbsp;Merged | <code>CSS&#8209;04</code> | **Native button padding** | [PR #54](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/54) integrates padding for button, submit and reset controls in `main` at `25654fa`. The loaded Bootstrap toggle measures 191×41 px at both widths on that main build. |
| ✅&nbsp;Merged | <code>CSS&#8209;05</code> | **Live computed custom-property reads** | [PR #62](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/62) integrates at `5c21400`. Measured feature build `63a2259` passes 2,657 normal tests and 25 journeys locally and in CI. The unchanged Bootstrap sample advances to Popper's `node.toString()` boundary on its first trusted click at both widths. |
| ✅&nbsp;Merged | <code>CSS&#8209;06</code> | **Computed overflow and rectangular axis clipping** | [PR #65](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/65) integrates at `bf0c737`. Measured `7b8a5fb` advances the initialized Bootstrap sample to `assignedSlot` on its first trusted click at both widths. The menu remains closed. Element scrolling and scrollbar widgets remain unimplemented. |
| ✅&nbsp;Merged | <code>CSS&#8209;07</code> | **Computed position and layout-backed offset parents** | [PR #69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/69) advances measured `c74503c` to `offsetParent` at 1994:12. [PR #71](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/71) advances measured `96666d3` to `scrollLeft` at 3154:19. Both runs open the menu and execute the trusted Amber choice at 1280/640. The menu remains visible and input fails. |
| ✅&nbsp;Merged | <code>CSS&#8209;08</code> | **Viewport and supported non-scrollable offsets** | [PR #72](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/72) integrates at `13f5cd4`. Qualified feature `e7be7cd` passes 2,732 normal tests and 25 journeys locally and in CI. Its Bootstrap captures at 1280/640 show the open menu and trusted Amber choice, with the menu still visible and first fault `offsetLeft` at 1927:10. Nested element scrolling remains outside this slice. |
| ✅&nbsp;Merged | <code>CSS&#8209;09</code> | **Relative offset coordinates** | [PR #74](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/74) adds measured `offsetLeft`/`offsetTop` with border/padding origins and scroll independence. Untransformed offset geometry stays separate from visual translations. |
| ✅&nbsp;Merged | <code>CSS&#8209;10</code> | **Bounded translation rendering** | [PR #75](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/75) connects translations to visual rectangles, paint/hits/clips and absolute/fixed containing blocks. With PR #76, integrated `ec7db2f` has true open/close DOM/pixel evidence and responsive cards at both widths. Wider transforms remain outside this slice. |

### JS & Frameworks — From startup to useful interaction

**Goal:** Progress from loading a library to exercising an unchanged application.
A passing startup probe is a step, not a working application.

| State | ID | Deliverable & scope | Completion evidence |
| :--- | :--- | :--- | :--- |
| ✅&nbsp;Verified | <code>JS&#8209;01</code> | **Object-prototype access used by the Vue workload** | On `1561f4d`, Vue 2.7.16 mounts, updates from input and two clicks, and completes its asynchronous update at both W3 widths. |
| ✅&nbsp;Merged | <code>JS&#8209;02</code> | **Class inheritance from native HTML interfaces** | The Vue 3.5.45 runtime loads and exports on `45799d9`. The original probe verifies both class prototype links and the typed failure for unregistered construction. [PR #84](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/84) |
| ✅&nbsp;Merged | <code>JS&#8209;03</code> | **Weak collections** | On `acd5703`, the original WeakMap probe reaches count 4 and completes its timer. On `4bd4c71`, Vue passes its earlier missing WeakSet at 5:5880. [PR #78](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/78) integrated at `5c8fe78` and [PR #81](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/81) at `8f24b29`. |
| ✅&nbsp;Verified | <code>FW&#8209;01</code> | **One modern Vue or React application** | On integrated `f0dc9ef`, Vue 3.5.45 mounts, displays the edited name and count after two clicks, and completes the next-tick and timer update. [PR #96](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/96) |
| ✅&nbsp;Verified | | **Async functions and arrows** | On integrated `c669dd9`, both original pages displayed pending, fulfilled and rejected outcomes across real Promise jobs. [PR #100](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/100) |
| ◻️&nbsp;Continuing | <code>FW&#8209;02</code> | **Modern and legacy cohort coverage** | Keep existing legacy samples and separate modern jQuery/Bootstrap cases. Add materially different generations when evidence makes them useful; never infer support for every version from one sample. |

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
| ◻️&nbsp;Incremental | <code>PERF&#8209;01</code> | **Representative performance baselines** | Reuse successful original fixtures. Record parse/script/style/layout/paint time, input responsiveness, and retained memory across repeated navigation, with exact build/configuration and repeatable conditions. Failed or incomplete execution is not a speed win. |
| 🧊&nbsp;Later | <code>ISO&#8209;01</code> | **Sandboxed content processes** | Begin after representative load/render/input/navigation and selected media paths are repeatable, before positioning the browser for sensitive everyday use. Separate privileged browser services from restricted content; validate bounded messages, access refusals, crash containment, recovery, and resource limits. Multiple processes alone are not a sandbox. |
| 🧊&nbsp;Later | <code>GPU&#8209;01</code> | **GPU compositing/presentation with software fallback** | Start from measured rendering costs. Keep original rendering commands, add a bounded platform backend, verify equivalent behavior, device-loss recovery, and a “Use hardware acceleration when available” setting with active/fallback diagnostics. Expand rasterization only with evidence. |
| 🧊&nbsp;Separate&nbsp;decision | <code>GPU&#8209;02</code> | **Accelerated video presentation / decoding boundary** | Presenting our decoded frames is distinct from delegating decoding. No OS-codec fallback or replacement decoder is authorized by the graphics milestone; hardware decoding needs an explicit originality/dependency decision. |

Current reality remains [one browser process with worker threads and software
rendering](ARCHITECTURE.md). Isolation and GPU acceleration are **planned, not
implemented**. They are not prerequisites for the active compatibility fixes.
Neither depends on supporting every website first.

---

## Delivery — Frequent Source, Verified Builds

**Keep the current CI approach.** The development agent reviews failures and fixes
the affected work through the existing local, PR, and hosted checks. The proposed
asynchronous handoff is deferred; no workflow, branch-protection, or promotion
change is enabled by this documentation. Development source continues to advance
independently of owner-requested releases.

| State | ID | Deliverable | Exit criteria |
| :--- | :--- | :--- | :--- |
| 🧊&nbsp;Deferred | <code>DX&#8209;01</code> | **Asynchronous CI/CD handoff** | Retain the current agent-owned failure diagnosis and repair. Revisit handoff only by a later owner decision; do not transfer routine failed-build triage to the maintainer or weaken integration gates. |
| ◻️&nbsp;Planned | <code>DX&#8209;02</code> | **Commit-identified development artifacts** | Successful trusted CI exposes a complete, clearly labeled development payload with its real source identity, required notices/data, no private material, and bounded retention. It is not a release, tag, or version bump. |
| ◻️&nbsp;Proposed | <code>DX&#8209;03</code> | **Lightweight public `dev` integration channel** | Adopt only with explicit triggers, protected promotion, trusted identities, and no duplicate validation for unchanged inputs. Keep `main` current with promoted known-good work. Do not create another manually supervised release stage. |

For requested releases, preserve the selected source identity and use the existing
validation and publication procedure. The agent remains responsible for analyzing
and correcting failures. Continue safe independent work without modifying inputs
under verification; do not claim failed delivery succeeded. See
[RELEASING.md](RELEASING.md).

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
| **Post-alpha.2 source** | `DOM-01 / DOM-02 / DOM-03` | PR #29 string/hidden-input/Node-root work, PR #30 anchor URL reflection, and PR #32 native text INPUT fragments; not retroactively part of alpha.2 |

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
