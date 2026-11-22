<div align="center">

# Feature contracts

*A navigable guide to the behavior and acceptance requirements we preserve.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

> [!IMPORTANT]
> **The full contracts remain normative.** This page is an index, not a replacement
> specification or relaxed acceptance gate. The [detailed reference](FEATURE_CONTRACTS_DETAIL.md)
> contains the foundational contracts and later language, DOM, CSS and resource
> increments; its historical execution notes remain scoped to their recorded source.

Scope is agreed before implementation. Completion requires all contracted normal,
error, lifecycle and security acceptance. Missing criteria cannot be hidden by
shrinking a contract. These documents authorize no live check or public release.

## Choose the right document

| Question | Authority |
| :--- | :--- |
| What must the behavior and negative cases preserve? | [Full contracts](FEATURE_CONTRACTS_DETAIL.md) |
| What is implemented at a specific source checkpoint? | [Capabilities](CAPABILITIES.md) |
| What did an exact library/workflow actually do? | [Compatibility](COMPATIBILITY.md) and its source-bound catalogue |
| What should be developed next? | [Roadmap](ROADMAP.md) |
| When may a named release be published? | [Release procedure](RELEASING.md) |

The detailed reference includes historical source-scoped execution notes. A note
that a past candidate awaited validation is not a claim that its integration is
still pending today. Likewise, a past pass is not proof of a later implementation.
Use current source-linked evidence for status; preserve the lasting requirements.

At the historical `6089546` baseline, the [compatibility scorecard](COMPATIBILITY.md) records each pinned application’s measured source, completed stages and first failure.

Tagged candidate `0.82.0-alpha.2` identifies
[`14134ab`](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/14134aba8ac2ab001c8ec13db77f68fa220b1400),
including integrated feature work through `1de1f24` and release preparation.
Its [release workflow](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/runs/44971512445)
failed at the normal Windows test stage, before packages or publication. No
alpha.2 assets were published. This changes no acceptance requirement or
historical measurement; the verified downloadable baseline remains `0.82.0-alpha.1`.

The [HTML/XHTML foundation plan](HTML_FOUNDATION_PLAN.md) and its
[coverage register](HTML_FOUNDATION_COVERAGE.json) now govern active development.
They include native Custom Elements and Shadow DOM as foundation work. The
[P1–P6 sequence](ROADMAP.md#current-product-sequence) is preserved for product
resumption; its selected polyfill path did not itself require full native
components. Connected App fixtures and actual site acceptance remain separate. The legacy contract directory
below preserves foundational requirements rather than listing every newer
language or DOM increment; those supported slices and exclusions live in
[CAPABILITIES.md](CAPABILITIES.md).

## Contract directory

Legacy section anchors below remain available for incoming documentation links.
Each entry opens the complete corresponding requirements in the detailed reference.

<a name="t1--shared-tlstrust-and-data-coverage-baseline"></a>

### T1

**Shared TLS/trust and data coverage baseline**

Transport, certificate policy, selected authorities and bounded fallback.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#t1--shared-tlstrust-and-data-coverage-baseline)**

<a name="b1--ordinary-unmarked-external-stylesheets"></a>

### B1

**Ordinary unmarked external stylesheets**

Guarded parser/dynamic stylesheets and resource ownership.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#b1--ordinary-unmarked-external-stylesheets)**

<a name="b1a--finite-viewport-media-rules"></a>

### B1a

**Finite viewport media rules**

Viewport/media evaluation, admission and source lifetime.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#b1a--finite-viewport-media-rules)**

<a name="cp1--real-collections-calls-and-node-identity"></a>

### CP1

**Real collections, calls and Node identity**

Live collections, checked array-like calls and genuine Node identity.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp1--real-collections-calls-and-node-identity)**

<a name="cp2--genuine-comment-and-native-regexp-input-conversion"></a>

### CP2

**Genuine Comment and native RegExp input conversion**

Native Comment state and ordered RegExp input conversion.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp2--genuine-comment-and-native-regexp-input-conversion)**

<a name="cp8--genuine-nodegetrootnode-over-ordinary-trees"></a>

### CP8

**Genuine Node.getRootNode over ordinary trees**

Real roots, options conversion, borrowing and access checks.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp8--genuine-nodegetrootnode-over-ordinary-trees)**

<a name="cp7--finite-hidden-input-admission-and-native-form-state"></a>

### CP7

**Finite hidden-input admission and native form state**

Hidden controls, native values, tree/form ownership and atomic refusal.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp7--finite-hidden-input-admission-and-native-form-state)**

<a name="cp6--bounded-stringprototypeconcat"></a>

### CP6

**Bounded String.prototype.concat**

Generic ordered conversion, realms, failures and work accounting.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp6--bounded-stringprototypeconcat)**

<a name="cp5--finite-object-literal-prototype-setter"></a>

### CP5

**Finite object-literal prototype setter**

Static literal prototype semantics and early errors.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp5--finite-object-literal-prototype-setter)**

<a name="cp4--conventional-select-fragments-and-native-ownership"></a>

### CP4

**Conventional select fragments and native ownership**

Select/option staging, retained state, moves and ownership.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp4--conventional-select-fragments-and-native-ownership)**

<a name="cp3--responsive-stylesheet-to-layout-behavior"></a>

### CP3

**Responsive stylesheet-to-layout behavior**

Media lengths, box-sizing and fractional flex layout.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#cp3--responsive-stylesheet-to-layout-behavior)**

<a name="b2--finite-tabs-link-context-and-new-windows"></a>

### B2

**Finite tabs, link context and new windows**

Shared authorities, tab/window identity, input and retirement.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#b2--finite-tabs-link-context-and-new-windows)**

<a name="a1--about-and-running-build-identity"></a>

### A1

**About and running-build identity**

Native About panel, exact legal views and safe descriptive metadata.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#a1--about-and-running-build-identity)**

<a name="i1--one-windows-installation-package"></a>

### I1

**One Windows installation package**

Planned per-user installer, recovery and preservation requirements.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#i1--one-windows-installation-package)**

<a name="u1--opt-in-published-release-discovery"></a>

### U1

**Opt-in published-release discovery**

Planned startup toggle, manual checks and zero-contact OFF state.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#u1--opt-in-published-release-discovery)**

<a name="u2--explicitly-approved-authenticated-upgrade"></a>

### U2

**Explicitly approved authenticated upgrade**

Planned authenticity, explicit approval and recoverable upgrade.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#u2--explicitly-approved-authenticated-upgrade)**

<a name="f1--replaceable-animation-frame-operations"></a>

### F1

**Replaceable animation-frame operations**

Real callback scheduling/cancellation despite author replacement.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f1--replaceable-animation-frame-operations)**

<a name="f2--navigation-providers-and-session-history"></a>

### F2

**Navigation, providers and session history**

Input/results/destination/Back flow and history semantics.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f2--navigation-providers-and-session-history)**

<a name="f3--script-controlled-supported-local-video"></a>

### F3

**Script-controlled supported local video**

Genuine playback operations, promises, time and decoded frames.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f3--script-controlled-supported-local-video)**

<a name="f4--network-media-transaction"></a>

### F4

**Network media transaction**

Complete bounded transfers, TLS/CORS/credentials and source lifecycle.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f4--network-media-transaction)**

<a name="f5--bounded-vp8-dependencies-with-existing-pcm-synchronization"></a>

### F5

**Bounded VP8 dependencies with existing PCM synchronization**

Finite interframes, independent references and acknowledged audio clock.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f5--bounded-vp8-dependencies-with-existing-pcm-synchronization)**

<a name="c1--a-css-image-path-through-the-shared-renderer"></a>

### C1

**A CSS image path through the shared renderer**

One background URL layer through resource policy, layout and paint.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#c1--a-css-image-path-through-the-shared-renderer)**

<a name="f6--staged-authored-page-and-media-integration"></a>

### F6

**Staged authored page and media integration**

Retained finite integration requirements and deferred broader scope.
**[Read the full contract →](FEATURE_CONTRACTS_DETAIL.md#f6--staged-authored-page-and-media-integration)**

---

## Later contract increments

The reference has grown beyond the original milestone directory. These grouped
links expose the current shared behavior contracts without turning the index into
a duplicate capability matrix.

| Area | Detailed requirements |
| :--- | :--- |
| **History and native interfaces** | [Same-document entries](FEATURE_CONTRACTS_DETAIL.md#same-document-session-entries), [live Range boundaries](FEATURE_CONTRACTS_DETAIL.md#live-dom-range-boundary-increment), [form named lookup](FEATURE_CONTRACTS_DETAIL.md#native-form-interface-and-named-lookup) and [textarea values](FEATURE_CONTRACTS_DETAIL.md#native-textarea-interface-and-canonical-value) |
| **Templates, owners and activation** | [Nested templates](FEATURE_CONTRACTS_DETAIL.md#nested-checked-template-fragments), [inert queries](FEATURE_CONTRACTS_DETAIL.md#inert-selectors-and-genuine-query-snapshots), [passive HTML](FEATURE_CONTRACTS_DETAIL.md#passive-html-containers-through-cloneimport), [native controls](FEATURE_CONTRACTS_DETAIL.md#default-native-control-import-and-activation), [images](FEATURE_CONTRACTS_DETAIL.md#plain-image-cloneimport-and-real-activation) and [style batches](FEATURE_CONTRACTS_DETAIL.md#inline-style-cloneimport-and-atomic-mixed-stamping) |
| **Mutations and observation** | [Inert tree records](FEATURE_CONTRACTS_DETAIL.md#inert-tree-mutation-records-and-characterdata), [whole-content replacement](FEATURE_CONTRACTS_DETAIL.md#whole-content-replacement-notifications), [transient subtrees](FEATURE_CONTRACTS_DETAIL.md#transient-subtree-mutation-observation) and [same-owner replacement/promotion](FEATURE_CONTRACTS_DETAIL.md#same-inert-owner-node-replacement-and-loaded-parked-promotion) |
| **Native and author properties** | [HTML reflections](FEATURE_CONTRACTS_DETAIL.md#common-htmlelement-reflections-and-author-overrides), [ARIA strings](FEATURE_CONTRACTS_DETAIL.md#element-ariamixin-nullable-string-reflections) and [placeholder hints](FEATURE_CONTRACTS_DETAIL.md#native-text-control-placeholder-reflection) |
| **Requests and body values** | [Asynchronous XHR](FEATURE_CONTRACTS_DETAIL.md#original-script-request-and-xmlhttprequest-increment), [Headers](FEATURE_CONTRACTS_DETAIL.md#native-headers-values-for-shared-request-and-response-adapters) and [buffered Response/Body](FEATURE_CONTRACTS_DETAIL.md#buffered-response-values-and-native-body-consumption) |
| **CSS and SVG** | [Variable layout values](FEATURE_CONTRACTS_DETAIL.md#shared-variable-sizing-and-existing-layout-grammars), [typed sizing math](FEATURE_CONTRACTS_DETAIL.md#shared-typed-sizing-calculations), [computed presentation](FEATURE_CONTRACTS_DETAIL.md#actual-computed-display-and-foreground), [SVG geometry](FEATURE_CONTRACTS_DETAIL.md#svg-geometry-in-the-shared-document-cascade) and [SVG paint](FEATURE_CONTRACTS_DETAIL.md#shared-svg-fill-stroke-and-color) |
| **Events and diagnostics** | [AnimationEvent](FEATURE_CONTRACTS_DETAIL.md#genuine-animationevent-construction), [WheelEvent](FEATURE_CONTRACTS_DETAIL.md#genuine-wheelevent-construction), [ErrorEvent](FEATURE_CONTRACTS_DETAIL.md#genuine-errorevent-construction-and-synthetic-delivery) and [native Error console provenance](FEATURE_CONTRACTS_DETAIL.md#native-error-console-provenance-and-saved-namespace-operations) |
| **Connected component preparation** | [Resource-governed flattening](FEATURE_CONTRACTS_DETAIL.md#resource-governed-iterative-array-flattening) and [template SVG with real focus](FEATURE_CONTRACTS_DETAIL.md#loaded-flow-template-svg-and-connected-component-focus) |

## Maintaining contracts

Edit the detailed reference when changing an actual behavior contract; update this
index only when its navigation or summary changes. Preserve code-level negatives,
limits, source identities and original evidence. Do not treat planned installer,
update, GPU or isolation work as implemented because it has a roadmap entry.

The current CI approach stays in place. The agent diagnoses failed checks and
repairs affected work. Completing a contract or PR does not trigger a release or
end the remaining development queue.
