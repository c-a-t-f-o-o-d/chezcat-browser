# CSS foundation and compatibility plan

[Roadmap](ROADMAP.md) · [HTML/XHTML foundation](HTML_FOUNDATION_PLAN.md) · [Coverage and work packages](CSS_FOUNDATION_COVERAGE.json) · [Standards inventory](CSS_STANDARDS_INVENTORY.json) · [Current capabilities](CAPABILITIES.md)

## Decision and sequencing

Activate this program after **HTML/XHTML F14 acceptance**. The current document
foundation continues through its own register. CSS needed to satisfy an included
F09–F13 default-presentation, geometry, namespace or native-control contract stays
a prerequisite of that HTML contract; it cannot be deferred by this sequencing.

The objective is a standards-based CSS implementation shared by HTML and XHTML,
with exact feature coverage across parsing, cascade, APIs, layout, painting,
interaction and dynamic updates. Preserve the substantial original implementation
already present. Expand its finite contracts through the packages below.

The saved YouTube shell, search/results, watch/Back and playback milestones remain
product integration targets. Each requires its actual CSS and platform dependency
closure plus a measured journey. Unrelated speech, printing, worklets or forward
drafts are not blanket prerequisites for those journeys. CSS program acceptance
has its own coverage boundary and cannot be inferred from one successful site.

This change supplies planning and static source evidence. **It does not execute
or complete C00, change the active HTML package order, qualify a CSS package,
publish a new browser binary, or record new compatibility results.**

## What the records contain

| Record | Purpose |
| --- | --- |
| This plan | Architecture, source findings, sequencing, package deliverables and acceptance policy |
| [Coverage register](CSS_FOUNDATION_COVERAGE.json) | 25 requirement families, 110 expandable requirement groups, 48 source observations, exact existing dispatch inventories and 33 packages C00–C32 |
| [Standards inventory](CSS_STANDARDS_INVENTORY.json) | 166 specification records, including all 64 unique modules in the reviewed 2026 snapshot classifications; publication/working-draft distinctions, 34 forward items and 21 adjacent dependencies |

Counts describe the inventory. They are not implementation percentages, test
denominators or PR estimates. No requirement group is marked qualified. Each
group must expand into exact clauses and assertions before its implementation
baseline. `CSS-F01` family IDs belong to this register; HTML `F01` remains a
different package. `P-*` and `LA*` identify inspected source observations.

## Standards baseline and ownership

The requested [CSS Snapshot 2025](https://www.w3.org/TR/2025/NOTE-css-2025-20250918/)
is retained for comparison. The current classification baseline reviewed on
2026-11-24 is [CSS Snapshot 2026, 22 June 2026](https://www.w3.org/TR/2026/NOTE-css-2026-20260622/).
Snapshot membership describes specification stability; browser adoption is a
separate feature-level question. CSS is a set of modules with independent levels,
not a sequence of complete CSS1/CSS2/CSS3/CSS4 browser implementations.

The reviewed 2026 classification adds Geometry Interfaces 1 and CSS Nesting 1 to
the rough-interoperability group, and removes Cascade 5's duplicate entry there
while retaining it among reliable Candidate Recommendations. It adds text/css
registration, security and privacy sections and updates per-feature pre-CR
exceptions. The register preserves both years rather than silently treating the
2025 list as current.

Use the [W3C module descriptions](https://www.w3.org/Style/CSS/specs.en.html),
[current-work index](https://www.w3.org/Style/CSS/current-work.en.html) and
[CSSWG editor drafts](https://drafts.csswg.org/) for discovery. The 166 module URLs
were retrieved; 17 publication statuses remain explicitly unknown and 13 index
versus retrieved-publication differences remain recorded. The dated publication,
current editor draft, snapshot tier and adoption status are separate fields.
C00 must resolve the selected normative revision and relevant errata before
execution. A floating draft URL is not an immutable input lock.

Several ownership decisions prevent implementing stale algorithms:

- Container-query work is owned by [Conditional Rules 5](https://www.w3.org/TR/css-conditional-5/); the older Containment 3 publication remains a provenance reference.
- `@scope` belongs to [Cascade 6](https://www.w3.org/TR/css-cascade-6/). Current [CSS Shadow 1](https://drafts.csswg.org/css-shadow-1/) consolidates shadow styling/parts; old Scoping publications must not revive obsolete `/deep/` or `::content` behavior.
- [Geometry Interfaces](https://www.w3.org/TR/geometry-1/) owns the shared matrix/point/rect/quad APIs; the old standalone matrix proposal is historical.
- Real worklets depend on [HTML worklet realms and loading](https://html.spec.whatwg.org/multipage/worklets.html). Rust value types alone do not implement Typed OM or script-facing worklet APIs.
- CSS2.1 remains the base for unsuperseded rules; replacing modules govern their own revised behavior. Historical print/mobile/TV profiles and abandoned proposals are recorded without making them modern desktop completion gates.

The inventory distinguishes the selected implementation program, selected
feature slices with remaining watchlist, gated APIs, specialized renderers and
historical references. Forward items have individual owners and prerequisites;
they are not silently omitted or collectively claimed complete. For example,
[Color 6](https://drafts.csswg.org/css-color-6/) is explicitly not ready for
implementation, and unofficial proposals need a new readiness decision before
web exposure. Current syntax, useful feature boundaries and evidence must be
reconfirmed at that point.

## Source baseline: what already exists

The source audit is tied to public revision
[`6d1212e1`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/6d1212e18acb37028bbba63290eddc323ecd5a23).
It inspected central consumers and relevant test sources. It did not run CSS
builds, native tests, Windows App checks or browser WPT. Source/test anchors in
the register identify the finite behavior; historical results keep their own
revision, host and selection.

| Area | Reuse | Required expansion |
| --- | --- | --- |
| Syntax and values | Shared finite rule/declaration scanners, typed lengths, selected math, custom properties | Standards token/component-value model; common property grammars; full defaulting, substitution and serialization |
| Selectors and cascade | Real combinators/attributes/structural predicates, strict `:not()`, native focus/hover/form state, author importance/inline/specificity/order | Escapes/namespaces, forgiving/relative lists, `:is/:where/:has/:scope`, complete origins/layers/scope and consistent support detection |
| Resources and media | Whole-sheet admission, decoding, URL ownership, cancellation/stale-response checks; finite `@media` and live main-document `matchMedia` reads | Imports, conditional face/resource discovery, general sheet CSSOM/CORS/SRI, complete media/supports grammar and change scheduling |
| CSSOM | Genuine inline declaration mutation and renderer-backed computed/geometry reads | Complete property consistency, stylesheet/rule objects, constructed/adopted sheets, resolved values, observation and Typed OM |
| Flow and sizing | Real blocks, inline/inline-block/inline-flex, physical edges, finite intrinsic sizing | Anonymous boxes/fragments, floats/clear/BFCs, margin collapsing, complete intrinsic/percentage/replaced sizing and logical axes |
| Flex/grid/tables | Flex sizing/freezing/wrapping/alignment; one implicit grid column; separate-border nonspanning tables | Full flex axes/minimums/baselines, explicit grid tracks/placement/subgrid, table spans/captions/collapsed borders and fragmentation |
| Fonts and text | Original font parsing/rasterization, proportional glyph advances, fallback and legacy kerning | OpenType cluster shaping, Unicode segmentation/bidi, international line breaking, writing modes and full font loading/matching |
| Painting and geometry | Real relative/absolute/fixed positioning, translation, clipping, hierarchical stacking, group opacity, finite backgrounds/borders | Sticky scrolling, general matrices/3D, consistent colors/alpha, gradients/multiple layers/shadows/masks/filters and complete paint order |
| Scrolling and input | Root vertical scroll, finite overflow clipping, native focus/hover/hits and renderer-backed measurements | Nested/horizontal scrollports, real scrollbar/UI behavior, snapping/anchoring, complete viewport APIs and user preferences |
| Timelines and output | Existing easing metadata, event interfaces, rAF callbacks and independent animated-image playback | CSS keyframes/transitions/WAAPI, scroll/view timelines, multicolumn/fragmentation, paged output and individually gated extensions |

These findings correct several misleading shortcuts:

- `display:grid` is not full Grid: current layout is one implicit column with successive rows and finite alignment.
- `position:sticky` computes but has no scroll-dependent clamping.
- Terminal `::before`/`::after` recognition does not create generated targets or boxes.
- Transform translation has real paint/hit consumers; recognizing other valid transform syntax does not render it.
- `FontSpec::shape` currently maps individual characters and applies legacy kerning; the name does not establish cluster shaping.
- Easing declarations, `AnimationEvent`, rAF and image playback do not supply a CSS animation timeline.
- `touch-action` computation does not establish touch/pen gesture arbitration.
- Current `matchMedia().matches` is a genuine live main-document read through the finite media grammar. Native change delivery and full preference/device behavior remain open.

The inspected routing inventory has **107 distinct render/computed dispatch
names**, including SVG bridges; inline CSSOM advertises **69 standard names**;
computed-style dispatch explicitly handles **13 standard names** plus custom
properties. These overlapping sets are preserved in the register. They are not
counts of completely implemented properties. In particular, inline `order` and
`vertical-align` have no corresponding general layout consumer, while easing is
specified metadata.

Two source-derived cases should become frozen regressions in the first syntax
and property batches: comment stripping turns `a/**/.x` into `a .x`, and literal
`margin:inherit` lacks the common defaulting route used by the variable-reset
path. These were identified statically and were not executed by this plan.
Include token-boundary controls beside the selector case: `a/**/b` must retain
two identifier tokens and remain invalid, and `color:r/**/ed` must not become
`color:red`. Follow each malformed rule with an independent valid rule to verify
recovery. Replacing comments with spaces and simply deleting comment bytes can
both change CSS token semantics.

## Architecture and invariants

### One syntax and property model

Use [CSS Syntax](https://www.w3.org/TR/css-syntax-3/) tokens/component values for
external sheets, STYLE text, style attributes, CSSOM writes and support queries.
Keep source locations, URL bases, origin, layer, scope and owner provenance.
Property metadata defines initial values, inheritance, applicability, grammar,
shorthand expansion, computation, animation type and serialization. Arbitrary
declaration retention is not validated support.

Separate declared/cascaded/specified/computed/used/actual values. Percentages,
font/container/viewport units and calculation expressions retain their context
until the relevant phase. Invalid syntax, valid unimplemented behavior,
invalid-at-computed-value and resource refusal are different outcomes. A winning
value must not silently resurrect a lower declaration, an unrelated default or
stale geometry merely because its consumer is incomplete.

Implement [cascade origins and layers](https://www.w3.org/TR/css-cascade-5/) in
C04 before extending conditional/group syntax around them. C19 integrates
imports, nesting, scope and shadow styling into that model. C21 contributes real
animation/transition origins; it does not create a parallel property cascade.

### Shared boxes, fragments, text and geometry

Distinguish DOM/flat tree, style contexts, formatting boxes, fragments, paint
order and accessibility/reading order. Build anonymous-box and continuation
interfaces before completing complex layouts. Preserve fractional geometry and
define rounding at API/paint boundaries. One measured result must feed pixels,
clips, pointer hits, anchors, focus, selection, CSSOM geometry and native controls.

Fonts and text need original parsing, matching, cluster shaping, bidi and line
breaking with pinned Unicode/OpenType data. Logical axes and orthogonal flows
must inform intrinsic sizing, flex/grid/tables and positioning. C06 reserves the
fragment model; C08 and C22 complete text and fragmentation algorithms. Do not
build separate screen, print and CSSOM geometries that disagree.

### Lifecycle, security, capacity and accessibility

Reuse actual stylesheet/resource generations, source ownership, cancellation
and stale-completion checks. Every rule/DOM/state/font/image/viewport/preference/
timeline change has a defined invalidation path. Same-job CSSOM geometry reads
must flush the correct state; retirement or refusal must not publish stale
pixels/measurements as fresh results.

Preserve original Rust/std-only runtime ownership, TLS validation, CSP, origin,
CORS, nonce and credential boundaries. OS window/input/surface facilities and
font files as data do not authorize an external browser engine, shaper or codec.
Imports, fonts, images, effects and worklets retain their fetch destinations and
source bases. Rule exposure requires origin-clean checks; history selectors,
font/preference queries and captured surfaces require privacy boundaries.

Bound actual work and retained/intermediate storage. Existing atomic whole-sheet
admission is valuable, but the separate 64-byte declaration-name, 2,048-byte
value, 16,384-byte inline-style prefix and selector/ancestor cutoffs need explicit
contracts. A limit must not silently change an admitted selector or apply only a
stylesheet prefix. Keep normative recovery, unsupported feature behavior and
resource termination separate, with cancellation and usable recovery.

Keyboard access, user styles, visible focus, zoom, forced colors, reduced motion,
selection and logical reading order are acceptance requirements from the first
relevant package. C23 closes their cross-feature integration; it is not permission
to defer basic usability. Visual `order`, clipping or generated content must not
silently rewrite DOM or accessibility semantics.

## Execution order and package acceptance

`core_prerequisites` form an acyclic producer order. `integration_requires` list
interfaces that must work before related contracts close; they can point to
later packages. A core checkpoint permits subsequent implementation while
dependent acceptance stays blocked. It does not require every downstream
package to close recursively. Examples: C10 sticky needs C15 scrollports, C02
conditional imports need C16 and CSSOM security checks need C17, and C05 container
units need C18. These dependencies are explicit in the register.

C16 condition parsing and C17 declaration/rule APIs can begin once the shared
value/cascade model is ready; their viewport, font, layout and event integrations
close later. This avoids deferring the APIs needed to test the layout work.

| Work wave | Packages | Result |
| --- | --- | --- |
| Lock and language core | C00–C05 | Pinned requirements; one syntax, selector, property, cascade and value model |
| Structure and text | C06–C10 | Boxes/fragments, fonts, international inline layout, generated content and positioning |
| Layout and rendering | C11–C15 | Shared painting, complete tables/flex/grid, real scrollports and geometry |
| APIs and adaptive styles | C16–C19 | Media/supports, CSSOM/observation, containment/containers, nesting/scope/shadow; begin independent core slices earlier |
| Dynamic and output integration | C20–C23 | Effects, timelines, columns/pages, controls/preferences/accessibility |
| Explicit extensions | C24–C30 | Anchors, view transitions, Typed OM/registration, gated worklets, advanced typography/publishing and forward items |
| Compatibility and reporting | C31–C32 | Mode/legacy obligations and continuous feature-level acceptance reconciliation |

Package IDs are not a requirement to implement everything serially, and are not
PR counts. Split a package into cohesive reviewed changes while preserving its
locked assertions and shared contracts. The first batch after F14 is C00, C01,
C02's source/association core, C03, C04 and C05, with C16/C17 core work following
as their prerequisites are ready. Preserve existing behavior at each boundary.

### Package details

Each package below reuses the source observations linked in the JSON register.
Listed checks are future acceptance obligations, not results of this plan.

#### C00 — Freeze the implementation contract and evidence baseline

**Core prerequisites:** HTML F14 acceptance and current-source reconciliation. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `selected_implementation_program`.

**Deliverables**

- Reconcile the source audit and record the exact local/source commit, Windows x64 target, allowed OS interfaces, zero runtime dependency policy and HTML/XHTML acceptance gate.
- Create a spec lock, feature/requirement ledger and immutable WPT selection manifest; resolve ownership moves and status conflicts.
- Define native, render, API and security harness capabilities, failure taxonomy, resource budgets and release-claim rules before implementation.

**Acceptance:** A ledger validator rejects missing dated sources, unidentified exclusions, unfrozen WPT revisions, duplicate requirement IDs and cyclic package prerequisites. Capture a known visible control document, deliberate failing control, HTML/XHTML pair and secure multi-origin fixture through the real frontend; mark unavailable harness features blocked.

**Boundary:** No CSS completion claims or test execution against a floating corpus. No changes to existing TLS/CSP/origin enforcement.

#### C01 — Build shared CSS syntax and grammar infrastructure

**Core prerequisites:** C00. **Integration interfaces:** C02, C04, C17. **Track:** `selected_implementation_program`.

**Deliverables**

- Replace or complete lexer/component-value parsing with structured tokens and source spans.
- Add rule/declaration parsing, unknown-rule recovery, nesting/block recovery and declarative property-grammar entry points.
- Add serialization primitives and differential entry-point tests for stylesheet/declaration/value parsing.

**Acceptance:** Malformed escapes/strings/URLs, EOF in every token state, nested delimiters, custom-property payloads and unsupported whole-declaration rejection produce exact expected structures without panic or unbounded growth. HTML and XHTML embedded/external/style-attribute examples apply valid fallback declarations after invalid constructs; malformed input cannot consume unrelated later rules.

**Boundary:** Bound token storage and nesting using documented fail behavior; preserve bytes/encoding provenance for C02.

#### C02 — Attach and fetch stylesheets through the existing secure loader

**Core prerequisites:** C00, C01. **Integration interfaces:** C04, C07, C16, C17, C19. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement style/link/style-attribute and XML stylesheet association, media/title/disabled states and base-URL provenance.
- Implement stylesheet byte encoding/MIME processing, @import ordering/cycles, redirects, resource cancellation and load/error readiness.
- Connect stylesheet changes to document style invalidation and page lifecycle.

**Core checkpoint:** Ordered style/link/inline source association retains exact owner, base URL, source order, origin-clean disposition, whole-source admission and replacement/retirement invalidation through real HTML/XHTML producers. Original positive, invalid-input, cancellation, refusal/recovery and source-replacement controls pass for the selected association core. Conditional imports, dynamic font discovery, general CORS/SRI and CSSOM exposure stay individually blocked until their required interfaces pass.

**Acceptance:** Source lists and import graphs preserve order, bases and ownership; cycles/duplicates/failures have deterministic outcomes. Local multi-origin HTTPS fixtures verify blocked/allowed inline sheets, attributes/imports/fonts/images, cross-origin sheet application versus CSSOM access, MIME and redirect failures; CSS loading does not bypass document readiness rules.

**Boundary:** Use existing TLS verification, request destinations, CSP directives, CORS/referrer/mixed-content policy; cap bytes, imports and outstanding requests.

#### C03 — Match selectors and maintain invalidation

**Core prerequisites:** C01, C02. **Integration interfaces:** C08, C17, C19, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement namespace-aware Selectors 3 and specificity with HTML/XML case distinctions.
- Add admitted Selectors 4 functional/relative/state selectors and selector-list failure rules.
- Index rules and invalidate matching on tree/attribute/state mutations, with an explicit private-history policy.

**Acceptance:** Match results and specificity are checked against hand-derived DOM fixtures, including namespaces, :is/:where/:has/:not, nth formulas and invalid/forgiving lists. Style changes after insertion/removal, sibling changes, class/id changes and focus/form-state changes reach computed style and paint; HTML/XHTML case and namespace differences stay correct.

**Boundary:** Protect :visited data; bound pathological matching with observable failure diagnostics, not silently wrong supported matches.

#### C04 — Resolve property semantics, origins, layers and inheritance

**Core prerequisites:** C03. **Integration interfaces:** C05, C17, C19, C21. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement a property registry with initial/inherited/computed/serialization/animation/invalidation metadata and atomic shorthand expansion.
- Implement origins/importance, specificity/source order, presentational hints, CSS-wide keywords and inheritance.
- Implement named/anonymous/nested layer order, important-layer reversal and revert-layer in the common winner model; preserve source/base/origin/layer provenance and reserve explicit scope/animation interfaces.
- Share initial/inherited/applicability/defaulting/longhand-shorthand metadata across rendering, inline CSSOM, computed reads and support detection; distinguish invalid input, unimplemented valid behavior and resource refusal.

**Acceptance:** Conflicting UA/user/author rules, !important, inline styles, shorthand resets and invalid-at-computed-value cases have exact winning declarations and values. Paired HTML/XHTML documents verify UA/user/author priorities, important layer reversal, inline placement, stylesheet replacement and literal/var-produced CSS-wide keywords. Invalid declarations follow recovery rules; recognized unsupported winners and resource refusal cannot silently masquerade as supported fallback.

**Boundary:** No stringly typed style map as a substitute for property semantics; property admission records every pipeline stage.

#### C05 — Compute units, math and custom properties

**Core prerequisites:** C04. **Integration interfaces:** C07, C08, C15, C18, C26. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement value types and deferred unit/percentage resolution with font/viewport/reference-box context.
- Implement calculation grammar/type checking and admitted CSS math functions.
- Implement custom-property token preservation, var() dependency/cycle/fallback rules, env() and declarative @property registration prerequisites.

**Acceptance:** Dimensional arithmetic, percentage mixtures, cyclic variables, nested fallback, invalid substitution and serialization are exact; overflow/non-finite behavior is deterministic. Nested percentage/font/viewport examples, resized documents and inherited custom properties visibly update; custom properties containing commas, URLs or !important boundaries round-trip correctly.

**Boundary:** Cap expression work and expansion without bypassing URL taint/security context; choose and record numeric precision.

#### C06 — Generate boxes and implement normal flow

**Core prerequisites:** C05. **Integration interfaces:** C08, C12, C13, C14, C15, C22. **Track:** `selected_implementation_program`.

**Deliverables**

- Separate DOM/style/box/fragment trees; implement anonymous boxes, display decomposition and replaced-element hooks. Reserve fragment continuations and writing-mode axes now; C08 and C22 complete their algorithms without replacing the box model.
- Implement block and inline-block flow, intrinsic/percentage/min-max sizing, margins/padding/borders and margin collapse.
- Implement floats/clear, formatting contexts, display:none/contents and standards/quirks differences.

**Acceptance:** Geometry cases cover adjoining positive/negative margins, float clearance, shrink-to-fit, containing blocks, percentages and min/max constraints. Reference pages and geometry assertions cover nested block/inline/replaced content; DOM changes update both boxes and paint without stale hit targets.

**Boundary:** No layout recursion tied to untrusted tree depth; cap work and keep accessibility ownership separate from box generation.

#### C07 — Implement the font and text-metric substrate

**Core prerequisites:** C02, C05. **Integration interfaces:** C08, C16, C17. **Track:** `selected_implementation_program`.

**Deliverables**

- Define permitted first-party font parsing, decompression, matching, shaping and raster paths against pinned format/Unicode versions.
- Implement Fonts 3 baseline plus selected Fonts 4 matching, @font-face descriptors, fallback and loading behavior.
- Integrate font readiness/cache invalidation and line metrics for C08; queue script-facing loading interfaces for C17.

**Acceptance:** Glyph mapping, metrics, fallback selection, ranges, malformed tables and compressed-size limits have deterministic fixtures. Pinned licensed fonts and Ahem-based pages verify fallback/loading failure/late swap and layout changes on Windows x64; CORS/CSP failures do not expose font bytes.

**Boundary:** Reject corrupt fonts safely; cap expansion/cache/glyph work; no external shaping engine or codec introduced.

#### C08 — Lay out international inline text

**Core prerequisites:** C06, C07. **Integration interfaces:** C09, C12, C13, C14, C22, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement whitespace/text processing, line formation, baseline alignment and decoration.
- Add Unicode bidi/segmentation/shaping integration, logical properties, vertical and orthogonal flows.
- Implement baseline ruby/first-line/first-letter behavior and text selection/caret geometry; stage advanced text in C28.

**Acceptance:** Golden line fragments cover mixed bidi, grapheme clusters, tabs, preserved/collapsed spaces, breaks, baselines and logical-to-physical mappings. Pinned-font reftests include Latin, Arabic/Hebrew, CJK, vertical text and mixed writing modes; zoom/selection/hit testing correspond to actual displayed clusters.

**Boundary:** Declare language/data coverage; user font size and text overrides must survive author styling.

#### C09 — Generate content, lists, counters and pseudo boxes

**Core prerequisites:** C08. **Integration interfaces:** C17, C22, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement generated ::before/::after/::marker boxes and content/quote/attribute/counter values.
- Implement lists, counter scope/reset/increment/set and @counter-style algorithms/fallback.
- Attach pseudo-element styles and accessibility alternative content; keep advanced highlights/APIs for C23/C17.

**Acceptance:** Nested/reversed counters, suppressed boxes, marker positions, quote depth, invalid styles and algorithm fallback match expected text and structure. HTML lists and equivalent CSS lists match in screen/vertical layouts; dynamic list insertion/stylesheet changes update numbering, markers and accessibility output.

**Boundary:** Bound counter/quote recursion and generated text/image growth; do not modify document text to emulate content.

#### C10 — Position, stack, paint-order and hit-test boxes

**Core prerequisites:** C06, C08. **Integration interfaces:** C15, C20, C22, C23, C24. **Track:** `selected_implementation_program`.

**Deliverables**

- Complete relative/absolute/fixed geometry and containing-block rules; implement sticky constraints against the C15 scrollport interface and keep sticky acceptance blocked until real scrolling is integrated.
- Build stacking-context and paint-order traversal including top layer/backdrops where HTML gates permit.
- Use the same geometry for painting, clipping, hit testing, focus outlines and later transforms.

**Acceptance:** Static-position and inset equations, sticky constraints, z-order and overlap hit results have exact fixtures. Overlapping/nested positioned elements, scroll containers, dialogs/popovers and viewport changes show matching pixels and click targets.

**Boundary:** Top-layer visuals cannot change origin/focus permissions; bound paint list growth.

#### C11 — Paint base colors, images, backgrounds and borders

**Core prerequisites:** C07, C10. **Integration interfaces:** C17, C20, C21, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement color parsing/computation, base color conversion, opacity and painter primitives.
- Implement gradients, multiple backgrounds, borders/radii/shadows, replaced-image sizing and object-fit/position.
- Connect first-party image decoders and resource lifecycle; preserve unsupported format/function fallback.

**Acceptance:** Color/gradient samples, background geometry, border radii and intrinsic image sizing are numerically verified. Deterministic reftests include transparent/repeating backgrounds, gradients, broken resources and scaled/replaced content; both sides of each reference comparison are visibly nonblank.

**Boundary:** Use bounded decode dimensions and paint surfaces; preserve fetch destinations and malformed-image safety.

#### C12 — Complete table formatting

**Core prerequisites:** C06, C08, C11. **Integration interfaces:** C22. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement table anonymous-box fixup and row/column/cell/caption structure.
- Implement fixed/auto widths, spans, intrinsic sizing and baseline/caption placement.
- Implement separated/collapsed borders and later fragmentation interfaces.

**Acceptance:** Sparse/spanned tables, min/max contributions, border conflict winners and anonymous structures have exact geometry fixtures. HTML tables and CSS display:table equivalents render identically where specified, including RTL/vertical/nested/replaced content and mutations.

**Boundary:** Cap combinatorial span/layout work; malformed markup recovery remains owned by HTML/XHTML.

#### C13 — Implement flex layout and shared alignment

**Core prerequisites:** C06, C08, C10. **Integration interfaces:** C22. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement flex items, basis, line collection and intrinsic contributions.
- Implement grow/shrink freeze iterations, automatic minimums and cross-size calculation.
- Implement order/wrapping/baseline/gap/alignment and absolute children.

**Acceptance:** Algorithm fixtures assert intermediate frozen sizes, sums, rounding and baselines rather than only final success. Responsive flex pages cover min-content constraints, percentage basis, replaced elements, orthogonal text, reverse order and dynamic content.

**Boundary:** Visual order must not silently replace DOM keyboard/accessibility order.

#### C14 — Implement grid and subgrid

**Core prerequisites:** C06, C08, C13. **Integration interfaces:** C18, C22. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement named/explicit/implicit grids and auto placement.
- Implement track sizing, fractions, minmax, auto-repeat and alignment.
- Implement subgrid propagation, intrinsic dependencies and integration with positioning/fragmentation.

**Acceptance:** Track sizing phases, placement occupancy and subgrid contributions match traced fixtures; pathological cycles remain bounded. Nested/reordered/subgrid pages verify geometry, paint and mutation across writing modes; Grid 3 remains separately gated.

**Boundary:** Cap implicit grid growth and arithmetic; report resource-limit termination distinctly from unsupported syntax.

#### C15 — Implement overflow, viewport and scrolling

**Core prerequisites:** C10, C11. **Integration interfaces:** C17, C18, C21, C23, C24. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement overflow/clipping, viewport/root propagation and scroll metrics.
- Implement pointer/keyboard/programmatic scrolling, smooth behavior and scrollbars.
- Implement snap/anchoring/overscroll controls with nested containers and viewport changes.

**Acceptance:** Scrollable extents, snap candidate choice, anchoring suppression and logical scroll coordinates have exact fixtures. Real Windows input plus geometry assertions verify scrollIntoView, nested scroll chaining, sticky interactions, resize/zoom and scrollbar hit testing.

**Boundary:** Keep layout/visual viewport boundaries, keyboard reachability and safe scroll ranges; do not regress input-origin checks.

#### C16 — Evaluate media and supports conditions

**Core prerequisites:** C04, C05. **Integration interfaces:** C02, C07, C15, C17, C18, C22, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement Media Queries 3/4 boolean/range parsing and environment model.
- Add selected Media Queries 5 preference/capability features and print switching.
- Implement @supports selector/font tests and change notifications with exact capability reporting.

**Acceptance:** Unknown features, invalid boolean syntax, range boundaries and supports outcomes match pinned truth tables. Viewport/DPI/theme/forced-color/reduced-motion changes invalidate styles correctly; unsupported features preserve fallbacks and CSS.supports remains consistent with declaration admission.

**Boundary:** Fingerprinting-sensitive capabilities use an explicit privacy policy; no guessed hardware capability.

#### C17 — Expose CSSOM, layout geometry and observation

**Core prerequisites:** C04, C05. **Integration interfaces:** C02, C07, C08, C10, C15, C16, C18, C19, C20, C21, C22, C23, C26. **Track:** `selected_implementation_program`.

**Deliverables**

- Build declaration/rule identity, mutation and serialization against the common syntax/property model early; geometry, fonts, media and observation close as their real producers become available.
- Connect rule/declaration/stylesheet object identity, mutation, constructed/adopted sheets and serialization to the original JS/Web IDL runtime.
- Expose computed/resolved style, MediaQueryList, CSSOM View and Geometry interfaces.
- Implement ResizeObserver and FontFace/FontFaceSet callbacks/promises with layout/event ordering.

**Core checkpoint:** Selected declaration and same-origin rule objects have genuine original-runtime identity, receivers, live mutation, serialization and shared property validation, with source origin checks enforced before exposure. Actual object mutations reach the existing style/paint consumer. Wider constructed/adopted/cross-origin lifecycle, resolved geometry, fonts, media and observation remain blocked by their named producer interfaces; native stand-ins are not accepted.

**Acceptance:** Native interfaces validate ownership, live collections, exception classes, read-only state and serialization. Unmodified testharness/idlharness tests execute in ChezCat; CSSOM edits affect pixels; geometry matches rendered boxes; cross-origin cssRules access rejects correctly. For applicable CSSOM View rows, compare positioned/transformed boxes and multiline/multicolumn fragment rectangles with actual paint and hit geometry after scrolling, resizing and mutation. Those rows remain blocked until the C10, C20 and C22 interfaces pass.

**Boundary:** If original JS/Web IDL/event-loop support is absent, API rows remain blocked; native stand-ins never count as WPT passes.

#### C18 — Implement containment and container queries

**Core prerequisites:** C14, C15, C16, C17. **Integration interfaces:** C21, C22, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement containment/content-visibility and intrinsic fallback size behavior.
- Implement query-container lookup, size queries and container units.
- Add admitted style/scroll-state queries and change/invalidation scheduling with explicit cycle handling.

**Acceptance:** Containment boundaries, query selection, unit fallback and style/layout dependency graphs match targeted fixtures. Nested named containers, hidden/revealed content, animated/resized containers, focus/search and observer delivery work without oscillation or stale rendering.

**Boundary:** Follow current Conditional 5 ownership; do not conflate content skipping with inaccessible hidden semantics.

#### C19 — Integrate nesting, scope, shadow styling and layer imports

**Core prerequisites:** C03, C04, C17. **Integration interfaces:** C21, C23, C26. **Track:** `selected_implementation_program`.

**Deliverables**

- Integrate the complete C04 layer model with imports, CSSOM mutation, nested declarations and stylesheet adoption; do not add a second property-specific layer cascade.
- Implement CSS nesting including nested declarations and current @scope proximity/boundaries.
- Integrate CSS Shadow host/slot/part rules with real Shadow DOM and adopted sheets.

**Acceptance:** Cascade-order matrices and scoped selector fixtures include important-layer reversal, mixed nested declarations and shadow encapsulation. Style edits, slot reassignment and part changes update computed style/paint; old scoping syntax is rejected unless separately documented as required compatibility.

**Boundary:** No style leakage across tree/origin boundaries; preserve privacy in selector-dependent resource loads.

#### C20 — Add advanced color and graphics effects

**Core prerequisites:** C11, C17, C19. **Integration interfaces:** C21, C22, C23. **Track:** `selected_implementation_program`.

**Deliverables**

- Complete admitted Color 4/5 spaces/interpolation/relative colors plus richer Images 4 values.
- Implement transforms, inverse geometry, motion-path substrate, blend/isolation, clipping/masking and filter pipeline.
- Add separately gated 3D/backdrop/high-range/SVG-referenced effects with scoped resource boundaries.

**Acceptance:** Color conversions/gamut cases, matrices/decomposition, clipping and filter primitives have numeric or pixel oracles. Reftests and hit tests cover nested transforms, compositing groups, mask/filter URLs and animated effects with fixed test clocks.

**Boundary:** Bound filter kernels/intermediate surfaces and prohibit cross-origin pixel disclosure; no external renderer/GPU library substitution.

#### C21 — Implement CSS and script-driven animation

**Core prerequisites:** C17, C20. **Integration interfaces:** C23, C25, C26. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement shared Web Animations timing/effects with deterministic sampling.
- Implement CSS transitions/keyframes, easing, interpolation/composition and events.
- Add WAAPI, motion paths and admitted scroll/view timelines; stage newer trigger/pointer features.

**Acceptance:** Timeline boundary, negative-delay, iteration, cancellation/reversal and composition tests assert exact sampled values and event order. Browser tests exercise CSS/JS interaction, promises, pseudo-elements, mutation, scroll timelines and reduced-motion behavior; references use a frozen clock.

**Boundary:** Budget frame work and concurrent effects; inactive/hidden contexts and cancellation release resources.

#### C22 — Lay out columns, fragments and printed pages

**Core prerequisites:** C08, C12, C13, C14, C16. **Integration interfaces:** C23, C29. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement fragment data model and break rules with widows/orphans.
- Implement multi-column sizing/balancing/spanning and nested fragmentation.
- Implement paged media/@page baseline and deterministic print output.

**Acceptance:** Fragment chains, break choice, balancing and page-box geometry have exact fixtures. Screen columns and paged render tests verify page count/content sequence, captions/counters, margin boxes and repeated layout; print artifacts are visually inspected.

**Boundary:** Cap pages/fragment work and print surface memory; no PDF/print codec dependency added without project authorization.

#### C23 — Finish controls, preferences, highlights and accessibility integration

**Core prerequisites:** C08, C09, C15, C16, C17, C19. **Integration interfaces:** C25. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement UI styling, outlines/cursors/carets/selection and admitted control appearance/accent features.
- Implement color adjustment and user preference behavior across controls, content and print.
- Implement highlight APIs/pseudo painting and full keyboard/zoom/forced-color acceptance across layout modes.

**Acceptance:** Focus/selection/caret state and highlight inheritance/painting fixtures validate semantic state separately from appearance. Windows keyboard and accessibility-tree checks cover logical reading order, visible focus, scrolling reachability, zoom/text spacing, forced colors and reduced motion.

**Boundary:** Accessibility is a requirement in every earlier package; C23 is integration closure, not permission to defer basic usability.

#### C24 — Implement anchor positioning

**Core prerequisites:** C10, C18, C19. **Integration interfaces:** C25. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement anchor names/scopes/selection and relative inset/size functions.
- Implement position areas/alignment, fallback tries and visibility.
- Integrate scroll changes, top layer and cycle-safe style/layout scheduling; stage Anchor 2.

**Acceptance:** Anchor eligibility, reference selection, fallback order and cycle cases have traced geometry fixtures. Popover/tooltip/dropdown examples remain aligned after scrolling/resizing and preserve focus/reading order; unsupported fallback declarations still function.

**Boundary:** Prevent unbounded fallback/layout loops and cross-tree leakage.

#### C25 — Implement view transitions with lifecycle integrity

**Core prerequisites:** C21, C23. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `selected_implementation_program`.

**Deliverables**

- Implement same-document capture, pseudo trees and ViewTransition lifecycle.
- Integrate cancellation/skip, animation timing, focus/selection and resource cleanup.
- Add separately gated cross-document transitions with HTML navigation/history/origin prerequisites.

**Acceptance:** Capture/lifecycle state machines test rejection, interruption and cleanup deterministically. Actual DOM/navigation transitions verify old/new pixels, promise/event order, reduced motion and origin eligibility; navigation continues correctly when transitions fail.

**Boundary:** Cap captured surfaces and lifetime; avoid exposing cross-origin or stale sensitive snapshots.

#### C26 — Expose registered properties and Typed OM

**Core prerequisites:** C05, C17, C21. **Integration interfaces:** C27. **Track:** `selected_implementation_program`.

**Deliverables**

- Complete @property/CSS.registerProperty validation, inheritance and interpolation.
- Implement typed property maps and CSS value object families.
- Implement conversion/serialization/exceptions and mutation parity with string CSSOM.

**Acceptance:** Registration/type algebra and string/typed round trips preserve semantics, including invalid and unsupported representations. Unmodified IDL/API tests run through ChezCat; typed edits affect layout/paint identically to equivalent CSS text, including animated custom properties.

**Boundary:** Original JS object model is mandatory; no fake objects in the test host.

#### C27 — Add worklet infrastructure and separately gated Houdini APIs

**Core prerequisites:** C17, C20, C21, C26. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `gated_api_extension`.

**Deliverables**

- Implement HTML worklet realms/module fetching/registration/lifetime and execution budgets.
- Implement Paint API in an isolated slice with input properties/arguments and paint context.
- Implement Layout and Animation Worklet experiments only after their constraint/fragment/timing APIs are stable and testable.

**Acceptance:** Realm isolation, registration failure, resource quotas, reentrancy and lifecycle cleanup have native adversarial fixtures. Secure-origin module loading and CSS paint/layout/animation tests execute through real worklets; CSP/CORS denial and runaway scripts are contained.

**Boundary:** No external JS/layout/paint engine; not-ready interfaces remain disabled and accurately reported.

#### C28 — Expand advanced typography and font features

**Core prerequisites:** C07, C08, C20, C23. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `selected_implementation_program`.

**Deliverables**

- Complete remaining Fonts 4/5 and international typography rows from the locked inventory.
- Implement advanced Inline/Text/Decoration/Ruby behavior in property/algorithm-sized slices.
- Evaluate rhythmic sizing, line grids, mobile adjustment and Text 5 independently.

**Acceptance:** Language/font/variation metrics and line-breaking/decoration algorithms use fixed, licensed fixtures and exact data versions. Visual and caret/selection/a11y tests cover each admitted script/feature; Unicode or font-data upgrades rerun the impacted matrix.

**Boundary:** State unsupported font formats/scripts/features explicitly; no claims based on Latin-only samples.

#### C29 — Expand paged publishing, regions and speech

**Core prerequisites:** C09, C22, C23. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `specialized_renderer_and_extended_publishing`.

**Deliverables**

- Implement generated paged content, running strings/elements, footnotes and page floats in independent slices.
- Evaluate region/exclusion/shape extensions with explicit use cases and reference behavior.
- Implement speech as a separate renderer integration or retain an explicit unsupported scope row.

**Acceptance:** Pagination/counter/footnote ordering, region-flow and spoken-style state have deterministic expectations. Paged references verify content continuity and accessibility; speech acceptance requires observable spoken output, cancellation and user controls.

**Boundary:** Historical Print Profile is not the scope gate; bound pages/audio/resource work and preserve origin policy.

#### C30 — Maintain individually gated forward-feature work items

**Core prerequisites:** C00. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `forward_watchlist`.

**Deliverables**

- For each experimental-registry item, record current owner, dated draft, prerequisites, usable feature boundary and proof obligations before implementation.
- Implement one coherent feature behind an explicit development gate, preserving release-parser fallback while disabled.
- Promote only rows with verified native/browser/API/security evidence; retain blocked/declined/superseded proposals with reasons.

**Acceptance:** Each item supplies its own algorithm/error/resource tests; merely appearing in a CSSWG index does not establish a stable implementable contract. Every admitted feature has observable rendering or API acceptance and an unsupported-off control; no collective 'future CSS complete' gate.

**Boundary:** Color 6 explicitly says not ready for implementation; treat that and unofficial proposal drafts as monitor-only until their status changes.

#### C31 — Close compatibility gaps with bounded, evidence-based behavior

**Core prerequisites:** C19, C22, C23. **Integration interfaces:** the applicable shared pipeline and adjacent standards. **Track:** `selected_implementation_program`.

**Deliverables**

- Reconcile WHATWG Quirks/Compat and normative deprecated aliases with current property/rule support.
- Run standards/limited-quirks/quirks and HTML/XHTML differential cases on all affected layouts.
- Record intentional unsupported historical profiles, abandoned proposals and vendor-specific behaviors; add only evidenced compatibility.

**Acceptance:** Mode/alias dispatch cannot change modern syntax or bypass validation; stale historical definitions never override selected current owners. Real legacy fixtures preserve useful fallback while modern pages and security regressions remain unchanged.

**Boundary:** Compatibility never enables unsafe executable CSS extensions or weakens CSP/origin/TLS.

#### C32 — Reconcile feature acceptance and publish measured boundaries

**Core prerequisites:** C00. **Integration interfaces:** C01, C02, C03, C04, C05, C06, C07, C08, C09, C10, C11, C12, C13, C14, C15, C16, C17, C18, C19, C20, C21, C22, C23, C24, C25, C26, C27, C28, C29, C30, C31. **Track:** `program_reporting`.

**Deliverables**

- At each core or integration checkpoint, reconcile claimed requirements with the source, spec lock, selected assertions and open dependencies. Program-wide reports retain every specialized, gated and watchlist boundary.
- Run frozen native/Windows browser/API/security lanes and explain every fail, skip, timeout, crash and blocked harness item.
- Produce supported-feature and resource reports with immutable evidence and a reviewed next-package backlog.

**Acceptance:** All claimed requirements have applicable evidence; no expected-failure file is altered to turn implementation failure into a pass. Deterministic integration cases cross parsing/cascade/layout/paint/mutation/input; no CSS3/CSS4 or all-CSS completion claim is inferred from totals.

**Boundary:** Release only the verified subset; retain outstanding Speech/Houdini/specialized/experimental boundaries as open work.

## Freeze tests before implementation

C00 must lock exact specification editions/commits and a full immutable WPT
revision, archive and file hashes, licenses/notices, selected paths/subtests,
references, support scripts/fonts/images/server dependencies and runner versions.
The current register deliberately leaves commit/hash/selection fields unset.
Candidate WPT directories are discovery hints until verified against that tree.
No CSS baseline has been executed by this planning change.

Before starting a requirement, record the property/value/selector/at-rule/algorithm/
IDL clause; HTML versus XHTML/XML and document mode; all applicable pipeline
stages; expected observable result; source/build/host; selection denominator;
and blocking interfaces. Expand each of the 110 requirement groups into these
actual assertions. Unavailable infrastructure remains a named blocker.

Use four distinct evidence lanes:

1. **Original native tests:** tokenizer/value/cascade/intermediate geometry,
   shaping/timing and adversarial resource cases exercise production components.
2. **Unchanged browser tests:** real ChezCat loads pinned documents, testharness/
   idlharness and reference pages through its original script/runtime/resource
   path. Native stand-ins or translated assertions do not count in this lane.
3. **Native App and visual/input integration:** pixels, geometry, clipping,
   hit testing, live edits, keyboard/focus/selection, preference changes and
   refusal/recovery share the real renderer and controls. Software App proof and
   physical desktop output retain separate labels.
4. **Product journeys:** frozen original integration fixtures and actual selected
   site journeys establish their own load/startup/render/interact/async outcomes.

Pin viewport, DPI/device scale, zoom, locale, Unicode/font files, colors,
animation clock, resource readiness and reftest wait conditions. Use visible
positive controls and deliberately incorrect references so blank pages cannot
pass. Inspect printed output and test reentrant DOM/style/resource mutation.
Track all selected assertions, including fail, timeout, crash, skip-with-reason,
blocked harness, unimplemented and not-applicable-with-reason outcomes. Do not
change the denominator, expected output or tolerances to hide product failures.
Adapted native fixtures remain separately labeled with original provenance.

Development PRs run the focused changed-family tests and affected integration
selection. Broader frozen cohorts run at core/integration milestones and release
gates. A documentation update does not require an unrelated full browser suite.
Keep package claims tied to exact committed source and the checked artifact.

## Completion and handoff

C32 maintains a requirement-to-evidence reconciliation at every milestone.
A feature closes only when all applicable locked assertions and real consumers
pass, including its security/resource and dynamic behavior. If a stage is not
applicable, record why. Module names, parser acceptance, a property count, old
build totals or success on one framework are insufficient.

The selected screen/print/API program, specialized speech/publishing extensions,
gated worklets and forward drafts retain explicit boundaries. A report may name
a verified subset and its open requirements; it must not imply all CSS is done.
Future specification changes update the inventory and selected feature lock
through a reviewed delta, preserving prior evidence and unresolved failures.

Before implementation resumes after F14, refresh the source audit against current
main, keep any newly completed contracts, lock C00 inputs, and choose the first
remaining syntax/property slice. The historical CSS-01–CSS-10 roadmap milestones
remain source-bound increments; these C-packages are the broader standards plan.

## Reference roles

Normative algorithms come from the selected W3C/CSSWG publications and the
adjacent WHATWG, Unicode, OpenType and format standards listed in the inventory.
The supplied [MDN CSS documentation](https://developer.mozilla.org/en-US/docs/Web/CSS)
is useful for author terminology, examples and discovery. The supplied
[W3Schools property reference](https://www.w3schools.com/cssref/index.php) is a
secondary lookup, not a complete algorithm or API checklist.

The [Library of Congress format description](https://www.loc.gov/preservation/digital/formats/fdd/fdd000482.shtml)
provides archival/file-format context; its 2018 significant-update date makes it
unsuitable as the current module baseline. The supplied histories at
[GeeksforGeeks](https://www.geeksforgeeks.org/css/css-history-versions/) and
[Modern CSS](https://modern-css.com/history-of-css/) provide background. Their
version timelines do not define this implementation program or conformance.
The standards inventory retains the primary linked modules beyond these entry
pages and records what remains unverified.
