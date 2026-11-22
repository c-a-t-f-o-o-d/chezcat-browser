<div align="center">

# Changes

*Source history, measured progress, and immutable published snapshots.*

[Home](README.md) · [Roadmap](docs/ROADMAP.md) · [Compatibility](docs/COMPATIBILITY.md) · [Release procedure](docs/RELEASING.md)

</div>

---

> [!NOTE]
> Versioned entries below retain their historical wording and scope. Their
> original gaps and release-time procedures do not override the current roadmap
> or imply that later source changes are present in an old download.

## 0.82.0-alpha.2

Consolidated JavaScript, DOM, rendering, resource and capacity improvements since
`v0.82.0-alpha.1`, integrated through
[`1de1f24`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/1de1f24e4052fab14157c1419cb6a6e1fdbccf3c).
The final tag and `publication.json` identify the exact packaged source.

- Expand supported async/await expressions and control flow, generator functions
  and delegation, async iteration, computed class methods and tagged templates.
  Add BigInt primitives and width conversion, UTF-16 surrogate string handling,
  and Unicode/supplementary RegExp ranges. These remain implemented language
  subsets with the boundaries recorded in the capability matrix.
- Add real inert HTML documents and template content, attributes and selectors,
  nested clone/import/stamping, namespace-aware SVG preparation and checked
  promotion. Native mutations retain actual node identities, owner boundaries,
  edited controls and resource policies. MutationObserver records cover admitted
  child, attribute, character and content changes with deferred delivery.
- Extend genuine DOM interfaces, Range boundaries, native event families,
  session History/storage and document messaging. Supported author descriptors
  and prototype overlays coexist with canonical native Node operations.
  `Node.baseURI` reads the admitted owner/document base and preserves origin,
  native branding and URL limits; it performs no fetch or navigation.
- Add a finite asynchronous main-document HTTP(S) text XMLHttpRequest adapter,
  including real buffered completion and progress. Original Headers/Response
  body primitives support their documented admitted paths. Global Fetch,
  preflight, streaming, broader response types and persistent storage remain
  outside these completed slices.
- Improve shared CSS sizing variables/math and viewport units, inline-flex,
  supported structural/interaction selectors, focus/hover, visibility and hit
  state, stacking/opacity, and computed presentation. Inline SVG uses supported
  CSS geometry, shared cascade and paint, including the finite local-use path.
  General CSS/SVG conformance remains incomplete.
- Add selected Canvas 2D rectangle drawing, solid colors and RGBA readback, and
  responsive-image viewport behavior. Expand the home page's test sites and
  original scenarios for exercising implemented browser features.
- Replace many independent compiler, evaluator, DOM, callback and resource count
  ceilings with checked byte storage and explicit work accounting. Iterative
  array flattening removes the former 64-level refusal. Source-backed admission
  now also covers additional compression, HTTP, image, font and media records;
  cmap12 lookup avoids a copied group array. Existing format, output, work,
  allocation and security guards remain, and the capacity inventory is unfinished.
- Add `build_dev.cmd` for incremental local iteration with scoped development
  optimization settings. Normal `Build.cmd` and the release workflow retain
  full release optimization and exact-source verification.

The selected component fixture at
[`cd0b372` / PR #416](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/416)
exercises real import, mutation, deferred jobs, focus, icons and native controls;
it does not establish full native Custom Elements or Shadow DOM. Audited native
Node getters reduce redundant runtime bookkeeping in PR #430. One identical
depth-16 fixture measured 24.583 seconds before and 22.595 seconds after that
change; this single comparison is preliminary and is not a live-site result.

The last recorded signed-out YouTube run on
[`cad5f7c`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/cad5f7c14a8fe8ebe2637e4c5269e84eed9697da)
still renders a skeleton and reaches a 30-second script deadline during DOM
traversal. A usable homepage/search journey and video playback remain unproved.
Later merged source and offline fixtures do not establish site acceptance.
See [capabilities](docs/CAPABILITIES.md), [compatibility](docs/COMPATIBILITY.md)
and the [release procedure](docs/RELEASING.md) for exact scope and delivery checks.

## 0.82.0-alpha.1

Shared JavaScript, DOM, native-control and rendering improvements since
`v0.81.0-alpha.3`. Measurements below retain their actual source revisions.

- Add bounded WeakMap and WeakSet, receiver-aware Reflect.get/Reflect.set, and
  ordinary-object Proxy reads, writes, membership, prototype and forwarded
  own-property queries. Canonical DOM interface inheritance and SVG identity,
  ordinary label properties and absent-attribute removal enable selected Vue
  workflows. On [`f0dc9ef`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/f0dc9ef0bf797dc8746bf87212e8768d0bab8c87),
  the Vue 3.5.45 app mounts, accepts an edited name and two clicks, and completes
  its next-tick/timer update without recorded faults. Vue 2.7.16 passes input,
  asynchronous updates and responsive layout at 640/1280 px on
  [`1561f4d`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/1561f4dbb3c03139a2723a2e2b2db214b3c348e1).
- Add async functions and arrows returning genuine Promises, with bounded
  `await` suspension and resumption through the Promise job queue. On
  [`c669dd9`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/c669dd9b3618913419e621d6cac3961e31d04db8),
  both original pages paint pending, fulfilled and caught states, retain a
  rejection reason through a later handler, and each exercise one intentional
  unhandled rejection. Await positions and control-flow forms remain finite.
- Add native checkbox and textarea fragments, fresh checkbox/radio type
  assignment, defaultChecked reflection and native input interface accessors.
  Textarea and radio clones copy live values/checkedness and dirty flags, with
  defaults from their copied markup and groups following actual tree/form
  ownership. Tree changes and Reset retain neighboring edits and reject
  unsupported batches atomically.
- Add bounded ordinary Node cloning and read-only Element.outerHTML
  serialization, inert attribute-less LINK and explicitly empty TABLE fragments,
  EventListener objects with live handleEvent lookup, and genuine embedding-realm
  HTMLIFrameElement identity. Frame activation and resource mutation retain
  their existing checks.
- Improve CSS root-variable themes, equal circular background corners, uniform
  solid rounded borders, single-layer background-color clipping, calculated
  margin/padding products, and variable line-height and height. At
  [`59ed086`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/59ed0864bfbf5c1a9c7d45cada668d2f62400629),
  the pinned Bootstrap cards have a real 24 px wide-screen gutter while the
  trusted dropdown and narrow-screen stacking continue to pass.
- Match absolute-caption intrinsic width to actual collapsed-text advances.
  On integrated [`ae148c0`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/ae148c0263c10b2d5f3840ff5952348e17254a2e),
  the automatic caption stays on one line and the Bootstrap progress meter
  matches its 240×16 literal control at 25% and 75%, including pixels, at both
  640 and 1280 px.
- Add a real implicit single-column grid with auto rows, item stretching,
  row gaps and shared paint/hit geometry. On
  [`e11acce`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/e11acce392d6b8c8e0deec56e8b08a7639d312f2),
  Bootstrap's action stack matches its literal control's two 160×40 rows and
  pixels. A trusted click changes the gap from 8 to 16 px and the moved link
  remains clickable. Authored track templates and general grid placement remain
  outside this slice.

On `7608f4c`, legacy jQuery 1.12.4 reaches an attribute write on a detached
element at line 4743. On `2f9fa8d`, the React 18.3.1 runtime exports load and
ReactDOM reaches a `contentEditable` read on the active element at line 58,
before the component mounts. Wider Proxy
operations, await forms, CSS transforms, rounded descendant clipping and native
control styling retain their documented limits. See [compatibility](docs/COMPATIBILITY.md)
and [capabilities](docs/CAPABILITIES.md) for source-scoped results and boundaries.

Delivery uses runtime/source ZIPs, `publication.json` and `SHA256SUMS` to identify
the final release source and bytes. Publication and downloaded-asset verification
remain separate from these source notes.

## 0.81.0-alpha.3

A consolidated source catch-up since alpha.2, including the shared browser
capabilities used by the selected Bootstrap dropdown workflow.

At integrated source
[`ec7db2f`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/ec7db2f61cddb78c457b2966b72e1e362b89e66a),
the unchanged pinned Bootstrap 5.3.9 / Popper 2.11.8 fixture initializes at
1280 and 640 pixels. Cards form two columns at the wider viewport and stack at
the narrower one. Trusted input opens the dropdown, selects amber, updates the
visible state and closes the menu, with no recorded script fault. This is a
selected offline workflow result; broader framework and live-site support remain
unproved.

- Extend shared DOM operations with atomic DocumentFragment insertion, canonical
  Element queries, static/live NodeList iteration, sibling traversal, bounded
  selector negation and HTML disabledness, and live Node.contains ancestry.
  Ordinary-tree assignedSlot, Text construction and concrete DOM object strings
  retain genuine node identities and existing owner/realm checks.
- Extend hidden-input state, native text INPUT/BUTTON creation and reflection,
  and movement of existing checkboxes through real trees and fragments. Staged publication
  preserves edited/default values, selection, Reset, ownership and neighboring
  controls, with atomic refusal for unsupported mutations. Anchor HTTP(S) URL
  reflection retains raw attributes, effective bases and navigation guards.
- Add bounded String.prototype.concat, ordinary object-prototype access, finite
  base and Script-derived class construction, super and lexical new.target.
  Ordinary Document and dispatched base-Event properties retain their native
  access boundaries. The pinned modern jQuery app passes startup, trusted input
  and its asynchronous timer text update.
- Resolve bounded CSS custom properties for supported padding, borders,
  background, foreground and font-size values. Composite supported RGBA borders
  over the painted backdrop and honor native button padding, with actual App
  pixel, geometry and hit-target controls.
- Add live computed custom-property, overflow, position and transform readback;
  real offsetParent and parent-relative offsetLeft/offsetTop; and viewport vertical
  scroll offset access. Bounded CSS translations share cascade, layout, painting,
  clipping and input geometry, including identity transforms and containing
  blocks. Offset coordinates remain based on untransformed layout.
- Repair document-worker completion and expose development source identity in
  build information. Keep source-bound local/hosted checks and explicit resource,
  allocation, origin and ownership limits across these additions.

Earlier integrated source checkpoints:
[PR #29](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/29),
[PR #30](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/30),
[PR #31](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/31),
[PR #32](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/32),
[PR #33](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/33) and
[PR #34](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/34).

CSS syntax, DOM/CSSOM coverage and child-frame geometry remain finite. Transform
support covers admitted translations, including translate3d with zero z; general
rotation, scaling and 3D transforms remain unsupported. Nested element scrolling
and scrollbar widgets remain outside this slice. These results do not establish
complete modern/legacy jQuery, Vue or React support.

Versioned source notes do not establish publication. Exact-source release gates
and delivered-asset checks remain required; publication.json and SHA256SUMS
identify the eventual release source and bytes. No installer or automatic update
mechanism is added.

## 0.81.0-alpha.2

Release target: [v0.81.0-alpha.2](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.2).
This is one consolidated catch-up for the completed finite B2 milestone and the
integrated prerequisites accumulated since 0.80.0-alpha.1. A versioned source
section is not a publication claim. Delivery requires exact clean source,
normal/optimized/PE and offline gates, complete payload, an extracted-runtime
journey and downloaded-asset checks. The actual release's
publication.json and SHA256SUMS identify its source and delivered bytes.

- Correct the runtime trust-documentation link to its public source; development
  generators remain outside the runtime ZIP. Allow scoped external Cargo and
  verification storage while retaining exact-source, receipt and evidence guards.
- Complete the finite B2 tab/window and native link/context milestone. A bounded
  Session owns independent pages, history, scroll and loading state; browser-owned
  profile, download and history authorities preserve shared state and reject stale
  editor/gesture identities. Native tabs, shortcuts, real windows and tab-pinned
  inspectors use checked routes and reentrant continuations. Primary/middle link
  gestures and explicit foreground/background/window context commands consume
  genuine source-bound intents. Popup hover/leave preserves the original target;
  dragging or any held mouse button retires it. Capture generations and exact
  release envelopes refuse stale or unpaired gestures without changing ordinary
  controls, selection/bookmark drag or the existing Alt download path.
- Integrate CP1 and CP2 shared DOM/language prerequisites: stable live
  main-Document/supported Node child lists, checked borrowed slice/array-like
  apply, genuine Document Node accessors, fresh text replacement, native-created
  Comment leaves and RegExp input ToString through the original conversion path.
  Comments retain real identity/data/topology without becoming rendered Text or
  resource/selector content. No full DOM or complete library support is claimed.
- Deliver CP3's finite responsive rendering improvement. The unchanged
  JS-enabled Foundation 6.9 fixture forms two columns at 1280 and stacks at 640.
  Initial-font em media, terminal before/after stylesheet targets, explicit
  box-sizing inheritance and fractional flex fitting reuse the shared cascade
  and layout. Original App resize/zoom/geometry, independent pixels and link-hit
  checks remain. Generated-content painting and Vue initialization are separate.
- Deliver CP4 conventional select/option/optgroup fragments backed by genuine
  native controls and staged ownership. Whole-select removal/movement/reconnect
  retains identities and edited values, retires popup/focus/hit participation and
  projects connected form order/disabled state before publication. Twelve reduced
  controls include App choice/submission and active-popup retirement/reconnect.
  The unchanged W2 recheck advances to refused INPUT type=hidden mutation;
  initialization and interaction still fail.
- Deliver CP5 finite static colon object-literal prototype setters through the
  existing guarded chain. Null, ordinary objects and functions retain real
  prototype identity; primitive RHS values are evaluated once and ignored without
  coercion or an own property. Duplicate setters are early syntax errors. Ten
  language/App controls include real input and independent text pixels. One
  unchanged W3 invocation at two widths now parses the pinned Vue runtime, then
  first fails on undefined concat at 11:3613. This does not prove the later literal
  executed; Vue initialization/input/async remain failed in both configurations.
- Integrate T1's bounded dual-stack correction and the finite shared transport,
  diagnostics and trust-data work. Resolver scanning retains both families and
  preserves preference/order within explicit scan/storage/attempt limits; serial
  fallback reserves deadline time for a queued alternative family. Redirect hops
  share an absolute overall deadline. Exact-host document diagnostics and a
  pinned conservative 98-authority NSS subset retain source/policy notices and
  existing certificate, origin, cookie and request-policy checks. Blocking DNS,
  serial cancellation latency, concurrent Happy Eyeballs and broader TLS remain
  gaps; document responses do not establish site usability or challenge clearance.
- Retain the integrated B1 stylesheet/viewport slices and finite A1 About panel:
  guarded unmarked cross-origin stylesheets, real host resize/zoom media geometry,
  descriptive full product/source/build text, exact offline legal views and safe
  build-information Copy. Declared tags are publication-unverified metadata;
  captured App payload does not establish native clipboard delivery. No installer,
  release discovery or upgrade control is added.
- Retain the completed finite Media/Video and CustomEvent interface slices:
  genuine owner-realm roots and canonical controls, source/clock/frame/Promise
  lifetime, Boolean controls reflection, real constructed/legacy notification
  dispatch/cancellation and child-owner checks. Local App/input/offscreen evidence
  remains distinct from physical audio, physical desktop and public-service proof.

Checkpoint evidence is source-scoped. The B2 accepted source
[67c5d69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/67c5d691012eeb39767e44000d9c1aeac8cdea1a)
was integrated by [PR23](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/23).
The subsequent select checkpoint
[73f3315](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/73f3315ae23760966efb1c1e6bf8ff3e2492c6c4)
and literal checkpoint
[fb385cb](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/fb385cb6c0b06988d55beb8c60f9f9a7cab6e431)
were integrated by [PR24](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/24)
and [PR25](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/25).
Recorded local/hosted offline verification applies to those checkpoints.
Separate actual Windows desktop records cover six groups at 67c5d69, six at 73f3315
and four at fb385cb. Each record is limited to its observed source/scope; none
identifies the new release binary. The bounded T1 correction was integrated by
[PR22](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/22).

Named/script-opened contexts, wider accessibility, existing-select interior edits,
new explicit select form associations, hidden-input admission, ordinary own/legacy
`__proto__` operations, native-host prototype chains, String.prototype.concat and
broader framework support remain outside these completed slices. Wider Unicode,
CSS, storage, frames, codecs/containers and media/player APIs remain partial.
Google Search/Images/Maps are not fully supported; YouTube playback is unproved.
Installer/update work and further F6/service work remain deferred. The next
independent compatibility reduction follows this milestone's release closeout.

## 0.81.0-alpha.1 — validation candidate only

The immutable [candidate tag](https://github.com/c-a-t-f-o-o-d/chezcat-browser/tree/v0.81.0-alpha.1)
passed the normal offline gate and packaging, then failed the runtime
documentation-link check before extracted startup. No release or assets
were published. Its source and tag remain unchanged; the correction uses
0.81.0-alpha.2 with fresh exact-source delivery checks.

## 0.80.0-alpha.1

- Adopt prospective CSAPUL 1.1 with full offline native acceptance, explicit
  personal/organization source and headless commands, strict external receipts,
  same-text reuse and changed-terms checks. Decline and failed writes stop before
  browsing; source builds check acceptance without creating it. External
  substantive intake remains reports-only until its controls are verified.
- Correct startup navigation bridge replacement and preserve the original
  address/search, result/destination, one-Back and different-query input journey.
- Support visible version-zero VP8 single-vector I/P interframes and dependency
  seeking, plus constrained Matroska PCM16 synchronization. One authored video
  supports real load/pause/seek/play promises and events, decoded-frame readiness,
  source retirement and acknowledged PCM time alongside native controls.
- Extend that finite authored profile to complete HTTP(S) IVF/Matroska transfers
  selected by explicit MIME, with redirects, CORS/credentials, cookie rules, CSP,
  TLS identity checks and cancellation. Partial content remains refused.
- Paint one URL background layer on ordinary block/inline-block boxes with
  stylesheet-relative ownership, cascade, repeat/position/size, bounded loading
  and independent request/pixel/geometry checks.
- Add genuine owner-realm Event/UIEvent/MouseEvent constructors, prototypes,
  branded readonly fields and ordered dictionary conversions. Event/UIEvent
  dispatch/cancellation, legacy initialization, path copies and owner clocks use
  real stored state; native reads preserve their caller and foreign-owner denial.

Mouse dispatch/default activation and native pointer payloads,
KeyboardEvent/CustomEvent constructors, streaming/ranges, MSE, muted Matroska,
multiple players and broader HTMLMediaElement support remain unsupported. Routine
App/input/offscreen evidence and separate opt-in WinMM sample-clock checks do not
claim physical audibility, physical-desktop or live-service playback acceptance.

## 0.79.0-alpha.2

Replacement of the completed animation milestone on a clean public snapshot.
The operative CSAPUL 1.0 license is unchanged. Scope, packaging and publication
guards identify the containing source revision and preserve earlier valid grants.
Raw validation reports and private working records are excluded. No runtime
feature was added for this privacy reset. The withdrawn earlier alpha is not reused.

## 0.79.0-alpha.1 (withdrawn)

Main-Window requestAnimationFrame/cancelAnimationFrame are ordinary replaceable,
deletable and restorable operations. Saved natives retain real scheduling and
IDL conversion, receiver/realm, callback-this, microtask and cancellation behavior.
The checked queue grows on demand, preserves next-frame deferral and shares job
work with checkpoint scans. Ordinary exceptions continue; fatal work retires
callbacks and recovers at a new job. The original executable wrapper journey
checks actual text/pixels, cancellation, later frames and navigation.

Full SemVer strings and Windows numeric0,79,0,0 identify this experimental alpha.
Selected local media and ordinary navigation remain partial; no new Google/
YouTube, physical-desktop, audio-device or security-certification claim is made.
The tagged release record identifies actual build, assets and verification.

## Source import

CSAPUL 1.0 adoption and the original source import are complete. The current
public graph begins at the later snapshot documented in HISTORY_IMPORT.md;
the former initialization and imported ancestry are not retained in this graph.
Earlier valid release grants remain governed by their original terms.

## 0.78

Real JavaScript load() reuses source retirement/stable selection and actual local
video decoding without rewriting raw src/Attr identity. At that checkpoint,
script play/pause/seek and network playback remained incomplete.

## 0.77

Owner-realm base Node/Element/HTMLElement ancestry, real author Element.prototype
extensions, distinct Image ancestry and canonical EventTarget methods. Text uses
its real DOM path; Attr has no parent. Specialized interfaces remain partial.

## 0.76

Genuine standalone EventTarget construction, canonical per-realm prototypes,
receiver-checked methods and target-only synchronous dispatch.

## Earlier retained checkpoints

0.75 added detached STYLE.type metadata and Attr owner guards;0.74 measured
supported vertical scrolling areas;0.73 reflected parsed LINK URLs;0.72 added
mutable video source selection. Earlier original media, resource, language,
search-provider and navigation work is preserved in the current snapshot.
These entries do not retroactively claim broader compatibility or current live
service success. Historical version numbers are preserved.
