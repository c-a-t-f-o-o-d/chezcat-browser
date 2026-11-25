<div align="center">

# 🐱 ChezCat Browser — Development Roadmap

[![Published prerelease](https://img.shields.io/static/v1?label=Published&message=0.82.0-alpha.4&color=0e7490)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4)
[![Development focus](https://img.shields.io/static/v1?label=Focus&message=Web%20compatibility&color=6d28d9)](#active--upcoming-milestones)
[![Main source privacy and configuration](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml/badge.svg?branch=main&event=push)](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/workflows/offline.yml)

*Delivering repeatable web page improvements through shared, spec-aligned browser capabilities.*

[Release horizon](#release-horizon) · [Active work](#active--upcoming-milestones) · [HTML/XHTML plan](HTML_FOUNDATION_PLAN.md) · [CSS plan](CSS_FOUNDATION_PLAN.md) · [Architecture](#performance--architecture) · [Verification](#verification-and-documentation) · [Backlog](#reference--backlog)

</div>

---

## Release Horizon

**Each fully accepted milestone advances to named release preparation.**
Milestone IDs below describe work, not reserved versions, deadlines, or promises
of complete framework support.

| Channel | What it contains | When it changes |
| :--- | :--- | :--- |
| **Current prerelease** | [0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4), tagged at `122d0b7`, delivers the accepted F00 baseline. Its Windows build, runtime/source ZIPs, publication record, checksums and extracted runtime passed verification. | [Verified release and asset hashes](RELEASING.md#current-verified-release) |
| **Public `main`** | F00 baseline and partial F01/F02/F04/F05 implementation, with continuing document, encoding, tokenizer and tree-construction work. | Source development continues through the [foundation register](HTML_FOUNDATION_COVERAGE.json). |
| **Public feature branches** | Reviewable work in progress with its own check status | Meaningful implementation checkpoints, independently of releases |
| **Development binaries** | Planned CI artifacts identified by source commit, not another GitHub Release | Delivery work `DX-02`; not advertised as available yet |

Earlier releases and validation candidates retain their exact source and outcomes
in the [release history](RELEASING.md#release-history).

```mermaid
flowchart TD
    CORE["HTML and XML parsing + shared DOM"] --> DOCS["Elements, controls and document lifecycle"]
    DOCS --> COMPONENTS["Native components + default presentation"]
    COMPONENTS --> GATE["Verified document foundation"]
    GATE --> CSS["CSS requirements and implementation"]
    CSS --> SHELL["YouTube shell and input"]
    CSS --> COVERAGE["Remaining CSS package acceptance"]
    SHELL --> JOURNEY["Search, results, watch and Back"]
    JOURNEY --> MEDIA["Streaming and original playback"]
```

This is a dependency map, **not a calendar**. The [foundation plan](HTML_FOUNDATION_PLAN.md)
defines the active package order, integration dependencies and closure evidence.
The [CSS program](CSS_FOUNDATION_PLAN.md) activates after F14 and uses a
[source coverage register](CSS_FOUNDATION_COVERAGE.json) and
[module inventory](CSS_STANDARDS_INVENTORY.json). CSS required by an included
HTML foundation contract remains a prerequisite of that contract. Product
milestones require their actual CSS dependencies; unrelated printing, speech,
worklets and forward drafts are not blanket prerequisites for YouTube.
ECMAScript, media and the wider HTML application platform retain their explicit
programs; required dependencies cannot be counted as passes.
Isolation and GPU work keep their separate readiness gates.

> [!IMPORTANT]
> **Milestone delivery.** A milestone reaches acceptance after its required
> contracts and integrations pass. Its named release must then pass the
> exact-source validation and publication procedure. Record any failed delivery
> gate against that candidate.

---

## Active & Upcoming Milestones

**Legend:** ✅ **Merged / verified scope** · 🟡 **Partial / active** · ◻️ **Next / planned** · 🧊 **Deferred**

**Planned after HTML/XHTML:** the CSS foundation has 33 work packages C00–C32,
25 requirement families and a source audit of 48 areas. The standards inventory
reviews 166 records, including the requested 2025 snapshot and the 2026 update.
These are planning counts, not completed features or PR estimates. C00 input
locking and all CSS acceptance runs remain future work. Existing finite CSS
implementation is credited in the [plan](CSS_FOUNDATION_PLAN.md#source-baseline-what-already-exists).

Current foundation increment: live getElementsByName filters retain exact UTF-16
and match authoritative native HTML names. Focused checks cover conversion,
liveness, admitted storage and refusal/recovery, with native App interaction.
Canonical Document.prototype.getElementsByName supports ordinary prototype changes,
saved functions and receiver/origin preflight for the displayed backend.
Owner-bound inert HTML/XML/generic and supported child live traversal now uses
actual roots, exact HTML names and native list realms with storage/origin checks.
Ordinary HTML container name setters and Attr routes now retain raw UTF-16
values for exact live queries, cloning and observation. Control/slot/frame/resource
raw-name producers and protected raw-name imports remain open. Ordinary name
imports and template promotion share the producer policy and exact owner/Attr
behavior with transactional refusal/recovery. Child mutations remain open;
Common HTML class/title/lang/dir metadata now survives import and template
promotion with real token/reflection/CSS consumers and resource refusal/recovery;
Canonical Element.className now uses inherited native accessors over exact
attributes, including inert property writes, observation and materialization;
Canonical Element.classList now uses inherited accessors and ordinary PutForwards
ordering over existing main HTML/SVG token lists;
Inert HTML token lists now share exact native reads and mutations, independent
cloning/import, stable template promotion and admitted scratch/refusal recovery;
Genuine inert XML and other namespace Elements now share bounded ordinary
author properties, exact keys and accessor shadows with native identity,
clone/import isolation, adoption and origin/refusal checks;
Inert foreign Element token lists now share exact native attributes, namespace
separation, clone/adoption identity and resource/origin refusal recovery;
Inert id and common HTML native reflection writes now preserve same-spelling
namespaced attributes with exact native lookup, identity and refusal/recovery;
Nullable ARIA getters and inert setters now retain exact native DOMString
values, null/empty separation, namespaces and original refusal/origin behavior;
Displayed HTML registered ARIA metadata now preserves exact producers and Attr
writes through native import/promotion, control-state isolation and App checks;
Public HTML label for values now preserve exact producers and transfers through
actual native target matching and App pointer/editor/paint/link checks;
Canonical HTMLLabelElement htmlFor now shares exact native reflection, ordinary
author shadows, owner/origin checks and bounded read/write refusal recovery;
Canonical label control reads now follow exact same-root ID and descendant
association, native identity, bounded traversal and actual App activation;
Canonical label form reads now preserve native stored owners and bounded
inert connected/detached association with descriptor and App evidence;
Input labels now use a stable live NodeList with exact native association,
hidden-state identity recovery and admitted nested traversal scratch;
button and textarea labels now share bounded live NodeLists with distinct native getter brands; remaining built-in label interfaces and lifecycle dependencies stay required; meter/output/progress/select now complete ordinary built-in labels getters in supported main and inert trees, with other interface IDL and lifecycle work still required; exact OUTPUT for attribute producers now retain native UTF-16 records; its canonical htmlFor token list remains required; OUTPUT htmlFor now uses stable exact DOMTokenList handles and ordinary PutForwards in supported main and inert trees; other output IDL and lifecycle work remain required; OUTPUT form now shares bounded native form-owner association through a canonical readonly accessor; other output IDL and lifecycle work remain required; OUTPUT exact name reflection/producers and readonly type now share native metadata and live name-list behavior; value/reset/validation and lifecycle dependencies remain required; OUTPUT text replacement now preserves exact native children, Range/list/observer identity and bounded traversal while retaining control/resource guards; value/defaultValue/reset and full descendant-text reading stay required; native main/inert descendant-text reads now retain actual resource source text and bounded traversal/join backing with separate template/child-document topology; OUTPUT value/defaultValue/reset remain required; canonical OUTPUT value/defaultValue now separate native default overrides from actual child text with bounded atomic writes and pointer/paint evidence; reset and broader form lifecycle remain required; native reset-button actions now restore OUTPUT defaults and clear overrides with exact current ownership, real child records, scripting-disabled support and bounded refusal/recovery; exact public/null-namespace/Attr OUTPUT form attributes now feed current owner/label/reset selection through shared paid transactions and pointer/paint checks; OUTPUT noncandidate validation getters/methods preserve native authority without invalid/UI effects, and native exact custom-error storage/live OUTPUT ValidityState retain conversion, identity, resource and lifecycle boundaries; other controls remain required; canonical readonly input/button/select/textarea form getters share native/current exact owner authority with bounded origin/resource handling; canonical readonly select/textarea type metadata now uses exact attribute presence and bounded native authority; canonical readonly form.length counts listed built-ins with exact current owners and image exclusion; canonical fieldset identity and readonly form/type metadata now share exact native/inert authority; canonical fieldset name reflection now preserves actual exact native/inert attribute identity, conversion and observation boundaries; canonical fieldset disabled reflection now uses actual attribute presence and paid first-legend-aware native refresh; canonical fieldset noncandidate validation getters and methods now preserve native state without invalid events or UI changes; fieldset live ValidityState and custom errors now share bounded native storage with concrete-interface authority and reset preservation; fieldset form attribute producers preserve exact values and independent descendant reset ownership; legend metadata reads the actual direct fieldset parent owner; progress numeric metadata reflects actual attributes and finite conversion; meter numeric metadata reflects six ordered gauge points through bounded actual attributes; progress/meter cloning, import and fallback replacement now retain exact metadata through checked ordinary-element paths; canonical time dateTime now reflects exact actual attributes through bounded native/inert transactions and checked ordinary lifecycle; canonical data value now reflects exact actual attributes through bounded transactions and ordinary lifecycle; q and blockquote now share the canonical quotation interface and bounded ordinary lifecycle; h1 through h6 now share the canonical heading interface and bounded ordinary lifecycle; heading align now reflects exact actual attributes through bounded transactions and ordinary lifecycle; DIV align now reflects exact actual attributes through bounded transactions and ordinary lifecycle; LEGEND align now reflects exact actual attributes through bounded transactions while preserving fieldset ownership; LEGEND inert-document imports now preserve real destination owners and independent attributes/children through bounded snapshots; LEGEND native clones and main-document imports now preserve actual detached owners and independent metadata/children through bounded copies; canonical paragraph identity and align now share bounded actual metadata and ordinary lifecycle; UL type now shares bounded actual metadata and ordinary lifecycle; UL compact now reflects actual bounded boolean attribute presence; canonical SPAN identity now shares actual native/inert owners and bounded ordinary lifecycle; canonical description-list identity and compact now share bounded actual attribute presence and ordinary lifecycle; canonical list-item identity and type now share bounded actual metadata and ordinary lifecycle; LI value now reflects signed integers through bounded HTML parsing and shared 32-bit conversion; Canonical ordered-list identity and type now share bounded actual metadata and ordinary lifecycle; OL reversed now reflects actual bounded boolean attribute presence; OL start now reflects bounded signed integers with default one and shared number conversion; OL compact now reflects actual bounded boolean attribute presence independently of start/reversed/type; Canonical preformatted identity and width now share bounded signed metadata and ordinary lifecycle; Canonical separator identity and align now share bounded string metadata and ordinary lifecycle; Separator color now shares bounded string metadata independently of align; Separator noShade now shares bounded presence metadata independently of align and color; Separator size now shares bounded string metadata independently of align, color and noShade; Separator width now shares bounded string metadata independently of size, align, color and noShade; Line breaks now have a canonical interface and bounded clear string reflection; Menus now have a canonical interface and bounded compact presence reflection; Directories now have a canonical interface and bounded compact presence reflection; Font color now reflects actual attributes with legacy null conversion and real text descendants; Font face now reflects bounded string metadata independently of legacy color conversion; Font size now reflects ordinary string metadata independently of color and face; The listing/xmp aliases now share canonical HTMLPreElement reflection and real text lifecycle; Eight fixed historical HTML names now use the canonical nonconstructible HTMLUnknownElement interface; Twelve fixed historical HTML names now expose HTMLElement directly; Standard HTML names with the HTMLElement interface now expose its canonical prototype directly; HTML unknown-name fallback now preserves known built-ins, valid custom names and exact namespace identity; shared ins/del dateTime now reflects exact actual attributes with checked native/inert transactions and bounded ordinary text lifecycle; canonical map name now reflects exact actual attributes with checked native/inert transactions and live named lookup; map areas now exposes a readonly live SameObject collection with exact HTML-area filtering, checked cached access and native/inert mutation witnesses; area alt now has canonical exact native/inert DOMString reflection with bounded refusal/recovery and adjacent App paint witnesses; area coords now preserves exact native/inert DOMString metadata through checked reflection, attribute aliases and adjacent App paint; area shape now preserves exact native/inert DOMString metadata without interpreting image regions, with bounded attribute and adjacent App checks; area download now reflects exact native/inert DOMString metadata with bounded attribute and adjacent App checks; area rel now reflects exact native/inert DOMString metadata with bounded attribute and adjacent App checks; area ping now reflects native/inert USVString metadata with bounded attribute and adjacent App checks; area relList now exposes exact live native/inert tokens with ordinary forwarding with bounded attribute and adjacent App checks; area target now reflects exact native/inert DOMString metadata with bounded attribute and adjacent App checks; area hreflang now reflects exact native/inert DOMString metadata with bounded attribute and adjacent App checks; area type now reflects exact native/inert DOMString metadata with bounded attribute and adjacent App checks; area referrerPolicy now exposes canonical known values while preserving raw attribute spelling, with bounded transactions and adjacent App checks; area href now uses shared URL parsing and USVString writes over exact raw native/inert attributes with bounded transactions and adjacent App checks; area HTTP(S) URL components now share bounded native/inert URL parsing, actual href transactions and adjacent App checks; Area and Anchor now expose canonical internal-href stringifiers with bounded native/inert reads and adjacent App checks; Area.noHref now reflects actual native/inert attribute presence with hook-free Boolean writes, bounded transactions and adjacent App checks; detached Area clone/import now preserves independent exact metadata, destination URL authority and deep topology through the bounded shared copy planner with adjacent App checks; supported Area inert connection and template parking now preserve actual node/Attr/token identities, current owner/base and live map order through checked transfer with adjacent App controls; Param now exposes canonical bounded exact name reflection with native/inert Attr backing, observed constructor identity and adjacent App checks; its remaining value/type/valueType DOMStrings now share exact attribute transactions and all-field App controls; detached Param clone/import now preserves independent exact metadata, owner and deep topology through the checked copy planner with actual App controls; checked Param inert/main and leaf template connection now preserves node/Attr identity, owners and observer records with resource and origin refusal recovery and actual App pointer controls; exact native/inert datalist nodes now expose their canonical HTMLDataListElement owner-realm interface with actual App controls; datalist options now provides an exact HTML-namespace live SameObject descendant collection with native/inert identity, origin and refusal checks; DataList detached copies and checked inert imports now preserve independent real attributes, topology and options collections with explicit native option deep-copy boundaries; checked DataList leaf connection and template parking now preserve node, Attr, owner and live collection identities with real observer/refusal recovery; exact native/inert option nodes now expose their canonical HTMLOptionElement owner-realm interface with bounded constructor observation and actual App controls; Option disabled now reflects actual attribute presence with native SELECT state, observer/raw-alias identity and bounded refusal recovery; readonly Option form now follows checked qualifying SELECT ancestry and actual native/inert form owners; readonly Option index now counts the actual qualifying SELECT option list with bounded traversal and live inert order; exact native/inert optgroup nodes now expose canonical HTMLOptGroupElement identity with bounded constructor observation and actual App controls; OptGroup disabled now reflects actual own presence and updates existing SELECT descendant disabled state with bounded refusal recovery; OptGroup label now reflects exact actual DOMString attributes and updates existing SELECT group rows with checked conversion and refusal recovery; detached OptGroup metadata copies now retain exact records and real owners without unnecessary SELECT planning, preserving separate OPTION/lifecycle guards; passive OptGroup connection outside SELECT now preserves real node/Attr ownership through native, inert and template routes with checked refusal recovery; checked standalone OptGroup fragments now preserve balanced passive children and omitted sibling endings through native/template routes, retaining malformed-scope and OPTION lifecycle refusals; passive native HTML OptGroup adoption outside SELECT now retains actual subtree/Attr identities, owners and removal records while preserving control/resource lifecycle checks; passive OptGroup text and child mutations outside SELECT now preserve actual identities, raw text and observers while keeping SELECT/OPTION/content-resource guards; canonical Option.label now reflects exact attributes or HTML-aware text and updates bounded native rows/captions without changing selection or values; canonical Option.value now reflects exact attributes or shared text fallback and atomically synchronizes bounded native values, selected editors and placeholder validation; canonical Option.text and native textContent replacement now synchronize bounded fallback values/labels and actual child/observer/Range identities; canonical Option.defaultSelected now reflects actual presence with clean native selection updates, dirty choices and current-default reset; canonical Select.disabled now reflects actual presence and retains native effective control state; canonical Select.required now synchronizes exact presence with native placeholder validation; canonical Select.name now synchronizes exact DOMString attributes with checked native submission names; canonical Textarea.name shares checked native submission names while preserving live editors and defaults; canonical Textarea.disabled now reflects own presence while retaining native effective fieldset disability; canonical Textarea.required now synchronizes exact presence with native empty-value validation; canonical Textarea.readOnly now synchronizes readonly presence with native editing and validation guards; canonical Textarea.cols/rows now share positive unsigned reflection and native sizing defaults; canonical native Textarea.textLength now counts the current normalized API value in UTF-16 units; canonical native Textarea.defaultValue and textContent now commit child text with pristine editor and reset defaults; canonical Textarea.maxLength now reflects nonnegative signed-long limits and updates future UTF-16 native edits; canonical Textarea.minLength now reflects minimum metadata independently of live values and maximum edit limits; canonical Textarea.wrap now reflects metadata and applies hard wrapping only to native GET/POST submission data; canonical Textarea.autocomplete now processes ordered HTML autofill hint metadata while preserving authored attributes and live editor state; canonical Select.autocomplete now shares that bounded grammar while preserving selected options and native control state; canonical Input.autocomplete now shares processed hint metadata with an explicit hidden-anchor distinction and unchanged native state; canonical Form.autocomplete now reflects known on/off values with on defaults, exact raw storage and named-control precedence; canonical Form.name now preserves exact metadata and current document named lookup with native owner and editor state unchanged; canonical Form.noValidate now exposes hook-free presence reflection connected to the existing native required-check bypass; canonical Form.acceptCharset now reflects exact metadata while preserving native UTF-8 submission and hidden charset reporting; canonical Form.target now reflects exact metadata while navigable selection remains required; canonical Form.rel now reflects exact metadata while relList and link-relation processing remain required; frozen browser qualification stays separate.

**Active priority: complete the document foundation before returning to the
YouTube journey.** The [reviewed plan](HTML_FOUNDATION_PLAN.md) and
[coverage register](HTML_FOUNDATION_COVERAGE.json) replace P1-first scheduling.
F00 has passed its requirements and baseline gate. F01, F02, F04 and F05 have
source-bound partial implementations. Required DOM, parser, form, editing,
lifecycle, native-component and browser integration gates remain open.

The retained P1 package has integrated nested template content, live ParentNode views,
inert queries, passive HTML and supported native-control import, plain image
activation, inline style batches and main-document template import/clone.
Finite namespace-aware SVG, mutation observation, selected component focus,
author properties and Node base URI behavior are also integrated. Remaining
native families/resources, broader same-identity adoption and lifecycle behavior
retain the limits in [capabilities](CAPABILITIES.md).

The previously reviewed signed-out homepage check used
[`cad5f7c`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/cad5f7c14a8fe8ebe2637e4c5269e84eed9697da),
after PR #429. The page still showed the skeleton and the DOM walker reached its
30-second deadline. PR #430 subsequently integrated at `1de1f24`; no post-PR #430
live acceptance result is recorded. A usable header, guide and search field
remain unproved.

<details>
<summary><strong>Earlier homepage checkpoints</strong> — historical observations</summary>

An earlier clean `fff35f8` run at November 21, 04:13 UTC completed all 43
script executions and 61 callbacks without escaped errors but still rendered the
skeleton and logged an opaque error object. No usable shell is demonstrated.
These implementation checks do not prove a usable shell. The
[source-bound checkpoints](COMPATIBILITY.md#latest-reviewed-signed-out-homepage-checkpoint)
retain prior isolated development and owner-supplied observations separately.

The November 20 owner checkpoint on clean development source `cffb9d1` decoded
the main application script and completed 42 of 43 script executions, then stopped
on missing `Range`; the page remained a skeleton. [PR #359](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/359)
subsequently integrated a displayed-document Range subset. Its implementation
checks are separate from a new live-site result. The connected component,
DOM, layout and input queue below supersedes those historical missing APIs. The older dev137
surrogate/STYLE/CSS failures are historical observations. [Latest reviewed owner checkpoint](COMPATIBILITY.md#latest-reviewed-signed-out-homepage-checkpoint)

</details>

The selected page uses its supplied Web Components polyfill. Native custom
elements and complete Shadow DOM are broader capabilities, not established
prerequisites for this selected first-shell path. Keep genuine DOM semantics and
truthful feature detection. Search submission, results, watch navigation/Back
and video/audio playback each require their own observed outcome.

### Active foundation sequence

| Stage | Required outcome |
| :--- | :--- |
| **F00–F03 · Contract and shared data baseline** | Fixed requirement/test inputs, genuine document/namespace/mode model, Encoding/MIME and standard URLs |
| **F04–F05 · HTML parsing** | All tokenizer states and tree-construction modes, error recovery, entities, foreign content and templates |
| **F06–F08 · DOM, markup and XHTML** | Complete shared DOM/bindings, contextual fragments/serialization and XML1.0/Namespaces/XHTML |
| **F09–F11 · Native document behavior** | Element families, scripts/resources/navigation, all form states, focus/activation and editing |
| **F12–F13 · Components and presentation** | Genuine native Custom Elements/Shadow DOM, UA defaults, mode consumers and required rendering/text integration |
| **F14 · Closure** | Required fixed cohorts and Windows App journeys pass; remaining wider-platform work is explicit before product resumption |

This is the default start order. Core implementation and integration closure are
separate: a missing later dependency keeps its consumer open until that
prerequisite and its integration are implemented. Focused checks establish each
increment; full-site results require their own evidence.

### Current product sequence

**Planned after foundation acceptance.** These P1–P6 packages follow the
document-foundation program.

The packages below connect the existing foundations to observable page behavior.
They are **not percentages of a complete browser**, and an implemented API family
does not close its corresponding site checkpoint.

| State | Package | Integrated foundation | Remaining scope and completion criterion |
| :--- | :--- | :--- | :--- |
| ✅ / 🧊 | **P0 · Capacity stabilization** | Iterative flattening and selected compiler/runtime/DOM, compression, image, font, HTTP and media storage/count corrections are integrated. | The broad inventory remains deferred. Re-enter a related family for a demonstrated limit or correctness problem; [remaining policies](CAPABILITIES.md#remaining-capacity-correction) stay recorded rather than being declared unlimited. |
| 🧊 Retained | **P1 · Template preparation and stamping** | Nested inert templates, queries/attributes, supported clone/import/mutations, real observers, author properties, native focus and selected component fixtures. Passive HTML/custom-host content replacement, supported inert Text/HTML/SVG construction and clone/import reservation preserve native identities. Genuine Event author chains are integrated. | Finish remaining protected native-resource/control promotion, broader owner transitions and connected lifecycle/job behavior. Native quota exhaustion retains guarded direct-promotion refusal. A connected original App must create/update genuine controls and icons with preserved input, focus, observers and resource policy. |
| 🟡 Partial | **P2 · Shell layout, CSSOM and interaction** | Shared sizing math, variables, viewport lengths, focus/hover/visibility/stacking, supported SVG geometry/paint and real CSSOM readback. | Finish the selected shell's remaining layout, measurement and input contracts. Demonstrate the actual signed-out header, guide and search field with usable keyboard/focus/click behavior and matching pixels/hit targets. |
| 🟡 Partial | **P3 · Async requests and response bodies** | Real finite main-document text XHR, cancellation/progress, Headers and buffered Response values. | Implement the required Fetch/request/body/stream, response-type and CORS cases. Original App JSON/binary/cancel/policy checks and a selected site request must return actual data. |
| 🟡 Partial | **P4 · History and route traversal** | Native primitive-state pushState/replaceState and same-document history share the App's real entries and URL. | Add the required object-state serialization, traversal/event behavior and route integration. Demonstrate query/results → watch → Back → a different query, with state and URL agreeing. |
| ◻️ Planned | **P5 · Result resources, geometry and scrolling** | Existing original image/font decoders, supported responsive image selection, SVG and finite viewport geometry. | Combine actual result DOM/data with titles, supported thumbnails/icons, usable scrolling, hit targets and result selection. Broader image/font policy or formats are selected by that requirement. |
| ◻️ Planned | **P6 · Media APIs and an original playback path** | Finite authored IVF/VP8 and Matroska/PCM playback, native controls and selected media interfaces. | Build the required object-URL/streaming/MSE behavior and one genuinely offered container/video/audio combination. Observe advancing organic video, audible synchronized output and pause/resume/seek with measured player/decode cost. |

When P1 resumes, complete the known related operation families before another ordinary
first-shell run. Importing a copy and promoting an existing inert node are distinct
contracts: neither can substitute for preserving the identities, observers, live
Range boundaries and native resource authority required by the selected operation.
Full native Custom Elements and Shadow DOM remain broader unfinished features,
not prerequisites imposed on this polyfill-based checkpoint.

Requests or history operations required during initialization can be brought
forward from P3/P4 when first-shell work resumes. During the foundation program,
its package dependencies take precedence. Focused development
checks remain necessary throughout; connected App checks validate package
composition, and live-site checks follow plausible milestone readiness or answer
a specific unresolved runtime question.

**Compression and fonts:** Brotli and WOFF2 remain shared compatibility work.
All 15 responses in the reviewed run used gzip, so missing Brotli did not block
that run. A shared Brotli decoder can serve HTTP content coding and WOFF2, while
WOFF2 also needs font-table reconstruction and the appropriate font-fetch policy.
Promote this work for an actual required unsupported response or the selected
font-fidelity goal; readable fallback fonts remain acceptable for the first shell.

**Playback has two substantial parts:** media APIs/streaming (object URLs,
MediaSource/SourceBuffer, buffering, append/remove events and seeking), and an
original container/audio/video decoder path. Select one genuinely server-offered
combination rather than require every codec. Existing finite VP8/PCM/Matroska
playback does not establish service playback. Verify real synchronized frames and
audible output, pause/resume/seek, player-script timing and sustained decode cost.
The original-code policy already applies to software decoding; the separate
hardware-delegation decision below does not block that work.

The parked framework records retain the raw `onfocusin` failure in legacy jQuery and the `contentEditable` read before ReactDOM mounted on their measured sources. ContentEditable reflection and finite grid item inline alignment have since merged; those implementation changes do not establish a new framework workflow result. [Source-bound scorecard](COMPATIBILITY.md)

<details>
<summary><strong>Earlier framework checkpoints</strong> — source-bound history</summary>

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
for `main` or for the published 0.81.0-alpha.2 binary.

</details>

<details>
<summary><strong>Recorded DOM, CSS and framework milestones</strong> — source-bound evidence</summary>

These earlier implementation and workload checkpoints retain their measured
sources. The active foundation sequence above owns current work order, and the
[capability matrix](CAPABILITIES.md) records subsequent implementation coverage.
The [CSS foundation plan](CSS_FOUNDATION_PLAN.md) defines the broader standards
work after HTML/XHTML; the CSS-01–CSS-10 records below retain their original scope.

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

</details>

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

Development PRs use focused checks. Trusted pushes run source privacy and
configuration checks. Full regression, offline journeys and package verification
belong to release qualification.

| State | ID | Deliverable | Exit criteria |
| :--- | :--- | :--- | :--- |
| ◻️&nbsp;Planned | <code>DX&#8209;02</code> | **Commit-identified development artifacts** | Successful trusted CI exposes a complete, clearly labeled development payload with its real source identity, required notices/data, no private material, and bounded retention. It is not a release, tag, or version bump. |
| ◻️&nbsp;Proposed | <code>DX&#8209;03</code> | **Lightweight public `dev` integration channel** | Adopt only with explicit triggers, protected promotion, trusted identities, and no duplicate validation for unchanged inputs. Keep `main` current with promoted known-good work. Do not create another manually supervised release stage. |

For requested releases, preserve the selected source identity and use the existing
validation and publication procedure. Failed checks leave the corresponding
delivery incomplete. See [RELEASING.md](RELEASING.md).

---

## Verification and documentation

Implementation checks cover the changed contract, directly affected regressions
and relevant security negatives. Connected original App checks validate package
boundaries. Live-site checks need the selected milestone's prerequisites or a
specific runtime question that source and focused fixtures cannot resolve.
A source merge does not establish a new live-site result.

Changed source, merged revisions and published binaries have distinct identities
and validation records. A named release follows the separate delivery procedure.

Documentation separates these concerns:

| Document | Scope |
| :--- | :--- |
| **This roadmap** | Order, milestones, dependencies, and backlog |
| [HTML_FOUNDATION_PLAN.md](HTML_FOUNDATION_PLAN.md) + [register](HTML_FOUNDATION_COVERAGE.json) | Active document-foundation scope, normative coverage, packages and closure evidence |
| [CAPABILITIES.md](CAPABILITIES.md) | Implemented behavior and limits |
| [COMPATIBILITY.md](COMPATIBILITY.md) + [catalogue](compatibility.json) | Versioned, measured application outcomes |
| [FEATURE_CONTRACTS.md](FEATURE_CONTRACTS.md) | Lasting behavior and acceptance requirements |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current system boundaries |
| [RELEASING.md](RELEASING.md) | Milestone qualification and delivery procedure |

Documentation updates describe changed behavior and its limits. Keep production
original Rust/std-only, preserve security boundaries, and retain
unmodified upstream workloads with appropriate notices. Test counts alone do
not establish browser compatibility.

---

## Reference & Backlog

<details>
<summary><strong>✅ Delivered foundations</strong> — published baseline and later source progress</summary>

| Delivery | Area | Finite scope |
| :--- | :--- | :--- |
| **0.81.0-alpha.2** | `B2` navigation | Native tabs/windows, guarded link gestures and context actions, independent page state |
| **0.81.0-alpha.2** | `A1` identity | About/build identity and offline license access; explicit acceptance remains its own contract |
| **0.81.0-alpha.2** | `CP1–CP5` prerequisites | Collection/Node, Comment/RegExp, responsive CSS, conventional select, and literal-prototype foundations |
| **0.81.0-alpha.2** | `T1 / B1` foundations | Scoped TLS/trust, dual-stack transport, stylesheet/resource and viewport behavior; broader coverage remains partial |
| **0.81.0-alpha.2** | Selected media/interfaces | Existing finite VP8/PCM/Matroska, Media/Video and event slices within their documented limits |
| **Post-0.81.0-alpha.2 source** | `DOM-01 / DOM-02 / DOM-03` | PR #29 string/hidden-input/Node-root work, PR #30 anchor URL reflection, and PR #32 native text INPUT fragments; not retroactively part of 0.81.0-alpha.2 |

Published assets remain immutable. Read the [capability matrix](CAPABILITIES.md)
for exclusions and the [changelog](../CHANGELOG.md) for source history.

</details>

<details>
<summary><strong>🧊 Deferred backlog</strong> — unscheduled, with no reserved release numbers</summary>

| Area | State & trigger |
| :--- | :--- |
| **Windows installation and updates** | Preserve one installer, About/license access, optional startup update check, “Check now,” and explicitly approved upgrade contracts. Portable ZIP remains the baseline; schedule after current browser foundations. |
| **Additional media profiles** | The selected service-playback path remains P6 in the active dependency sequence. Further codecs, containers and media interfaces beyond that evidenced path remain unscheduled; existing finite playback is preserved. |
| **Extended web platform** | Wider Unicode, ES modules, networking APIs, storage, workers, frames, accessibility, DOM interfaces, and font/layout coverage are selected by demonstrated shared dependencies. |
| **Additional element APIs** | VIDEO dimensions, Element.toggleAttribute, NamedNodeMap mutation, and other interfaces remain distinct contracts rather than implied framework support. |

</details>

*Dates document observations, not delivery promises. Named release versions are
chosen only for an owner-requested, eligible snapshot.*
