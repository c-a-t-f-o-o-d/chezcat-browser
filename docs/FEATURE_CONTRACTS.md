# Feature contracts

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Compatibility](COMPATIBILITY.md)

This page summarizes ChezCat's lasting behavior and security commitments.
Completion requires the agreed normal, error, lifecycle and security cases;
missing criteria cannot be hidden by reducing a contract after implementation.
A shorter summary does not change those requirements.

A contract is not a claim of complete implementation.
[Capabilities](CAPABILITIES.md) describes supported subsets and exclusions;
[Compatibility](COMPATIBILITY.md) records what an exact workload actually did.
Planned installation and update features below remain planned.
Accepted milestones retain the [release requirements](RELEASING.md).

## Shared commitments

Supported operations use genuine native state, identity and ownership. Saved
methods and accessors must retain their receiver, realm and current-origin
checks. Author overrides, conversions and callbacks keep their specified order
and effects; native state must not be fabricated to make an application pass.

Requests and resource reuse retain destination, redirect, CSP, mixed-content,
cookie, credential, MIME and response-policy checks. An API or format addition
does not grant unrelated network or local-file authority. Replacement, removal,
Stop and navigation retire the correct resource; stale completions cannot regain
the authority of a later owner.

Checked storage, copy/work limits, deadlines and cancellation apply before
publication where the contract requires atomicity. Refusal must preserve the
specified existing state and allow recovery. Visible behavior is measured through
actual layout, pixels, input and hit ownership. A model check, successful response
or lack of script errors alone does not establish a working page.

## Contract directory

The identifiers below remain stable for existing links. They summarize enduring
requirements rather than every later native-interface increment.

<a id="t1--shared-tlstrust-and-data-coverage-baseline"></a>
<a id="t1"></a>

### T1 — Shared TLS/trust and data coverage

Original TLS/X.509 identity and path validation remain authoritative: hostname/SAN,
validity, signatures, CA/usage/path and critical constraints retain their checks.
The original hostname remains authoritative for TLS, HTTP Host, URLs, origins and
cookies even when address-family fallback selects a different socket address.

The default trust data is the reviewed conservative subset of 98 Mozilla NSS
authorities. Exact source, selection policy and notices remain part of its
provenance. Restricted or unsupported authorities must not become unrestricted
trust. Custom PEM stores replace the default, deduplicate exact certificates and
fail closed on unsupported entries under the shared 1,024-root capacity.

Bounded serial dual-stack fallback, per-hop and overall request limits,
cancellation and redacted diagnostics retain their explicit limits. Blocking
OS DNS is not a strict process wall-clock guarantee. There is no implied OS-store
lookup, automatic root update, revocation/CT, full Firefox policy or certification.
Document diagnostics do not establish challenge clearance or site usability.

### Documents, styles and native identity

| Contract | Required behavior |
| :--- | :--- |
| <a id="b1--ordinary-unmarked-external-stylesheets"></a><a id="b1"></a>**B1 — External stylesheets** | Parser and dynamic sheets use guarded requests, final-response URL ownership and complete decoded-source accounting. Source changes and retirement reject stale completion. Applying foreign CSS does not expose its source or grant general stylesheet CORS. |
| <a id="b1a--finite-viewport-media-rules"></a><a id="b1a"></a>**B1a — Viewport media rules** | Supported media conditions use the actual CSS viewport and declared input profile, preserve cascade order and stylesheet ownership, and admit the whole sheet before publication. Unsupported conditions do not grant rules. |
| <a id="cp1--real-collections-calls-and-node-identity"></a><a id="cp1"></a>**CP1 — Collections and Node identity** | Live collections and checked array-like calls retain actual node identity, mutation behavior and access authority. |
| <a id="cp2--genuine-comment-and-native-regexp-input-conversion"></a><a id="cp2"></a>**CP2 — Comment and RegExp conversion** | Comment objects retain native state; RegExp input conversion preserves its required order, abrupt effects and receiver behavior. |
| <a id="cp8--genuine-nodegetrootnode-over-ordinary-trees"></a><a id="cp8"></a>**CP8 — Node roots** | getRootNode follows real supported tree roots, options conversion, borrowing and access checks. |
| <a id="cp7--finite-hidden-input-admission-and-native-form-state"></a><a id="cp7"></a>**CP7 — Hidden inputs** | Supported hidden controls preserve native values, tree/form ownership and atomic refusal. |
| <a id="cp6--bounded-stringprototypeconcat"></a><a id="cp6"></a>**CP6 — String concatenation** | Generic ordered conversion, owner realms, failures and checked work accounting apply to the actual concatenation operation. |
| <a id="cp5--finite-object-literal-prototype-setter"></a><a id="cp5"></a>**CP5 — Literal prototype setter** | Supported static object-literal prototype syntax retains its semantics and required early errors. |
| <a id="cp4--conventional-select-fragments-and-native-ownership"></a><a id="cp4"></a>**CP4 — Select fragments** | Select/option staging, moves and retained state preserve native ownership without partial publication after refusal. |
| <a id="cp3--responsive-stylesheet-to-layout-behavior"></a><a id="cp3"></a>**CP3 — Responsive layout** | Supported media lengths, box-sizing and fractional flex layout must reach actual geometry, paint and hit targets. |

### Browser and delivery

| Contract | Required behavior |
| :--- | :--- |
| <a id="b2--finite-tabs-link-context-and-new-windows"></a><a id="b2"></a>**B2 — Tabs and windows** | Shared authorities, tab/window identity, input routing and retirement remain correct across the supported link contexts. |
| <a id="a1--about-and-running-build-identity"></a><a id="a1"></a>**A1 — About and build identity** | Native About, exact legal views and safe descriptive metadata identify the running build without presenting metadata as authentication. |
| <a id="i1--one-windows-installation-package"></a><a id="i1"></a>**I1 — Windows installer — planned** | A per-user installation package must preserve user data and retain its specified recovery behavior. |
| <a id="u1--opt-in-published-release-discovery"></a><a id="u1"></a>**U1 — Release discovery — planned** | Startup discovery is opt-in, with manual checks and zero network contact while the startup setting is OFF. |
| <a id="u2--explicitly-approved-authenticated-upgrade"></a><a id="u2"></a>**U2 — Upgrade — planned** | Upgrade requires authenticity verification, explicit approval and recoverable replacement. |

### Navigation, animation and media

| Contract | Required behavior |
| :--- | :--- |
| <a id="f1--replaceable-animation-frame-operations"></a><a id="f1"></a>**F1 — Animation frames** | Supported callbacks schedule and cancel through the real operation even when author code replaces exposed properties. |
| <a id="f2--navigation-providers-and-session-history"></a><a id="f2"></a>**F2 — Navigation and history** | Address input, provider results, destination navigation and Back retain actual request and session-history semantics. |
| <a id="f3--script-controlled-supported-local-video"></a><a id="f3"></a>**F3 — Local video** | Supported playback uses genuine operations, promises, time and decoded frames rather than simulated state. |
| <a id="f4--network-media-transaction"></a><a id="f4"></a>**F4 — Network media** | Complete bounded transfers retain TLS, authored CORS/credential policy and source replacement/retirement rules. |
| <a id="f5--bounded-vp8-dependencies-with-existing-pcm-synchronization"></a><a id="f5"></a>**F5 — VP8 and PCM synchronization** | Supported finite interframes retain independent references and synchronize against acknowledged audio progress. |
| <a id="c1--a-css-image-path-through-the-shared-renderer"></a><a id="c1"></a>**C1 — CSS images** | The supported single URL background layer uses the shared resource policy, layout and painting lifecycle. |
| <a id="f6--staged-authored-page-and-media-integration"></a><a id="f6"></a>**F6 — Authored page/media integration** | Supported composition must preserve the agreed page/media interactions and existing negative cases. Deferred broader scope is not completed by a finite integration fixture. |

## Scope and evidence

These commitments apply alongside the explicit supported subsets and refusals in
[Capabilities](CAPABILITIES.md). Later language, DOM, CSS and resource additions
retain the same native-state, lifetime and security principles. Native Custom
Elements, Shadow DOM, installation, upgrades, GPU acceleration or isolation are
not established merely by appearing in the [roadmap](ROADMAP.md).

Historical measurements keep their original source, inputs and limits. A prior
pass does not qualify a changed implementation, and a source-level feature does
not establish a live-service result. Public release claims must identify the
actual verified source and delivered assets.
