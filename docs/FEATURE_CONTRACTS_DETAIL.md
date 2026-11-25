# Feature contracts

Scope is agreed before implementation. Completion means all contracted normal,
error, lifecycle and security acceptance is met; missing criteria cannot be hidden
by shrinking the contract afterward. These contracts authorize no live check or
public release by themselves.

This document owns lasting behavior, refusal and acceptance requirements. The
[roadmap](ROADMAP.md) owns work order, the [capability matrix](CAPABILITIES.md)
implemented limits, and [compatibility](COMPATIBILITY.md) measured workflows.
Historical execution results remain bound to their recorded source and scope;
they do not replace affected checks for a changed implementation. Fully accepted
milestones enter release preparation under [RELEASING.md](RELEASING.md).

## T1 — shared TLS/trust and data coverage baseline

Status: finite diagnostics, trust-data and bounded dual-stack corrections are
integrated; ordinary document/resource interoperability remains in progress.
Original TLS 1.2/1.3 and X.509
cryptographic, identity and path rules remain. Configured trust authority expands
to the explicitly selected data, with a shared bounded root capacity.

The default contains 98 selected authorities from Mozilla NSS built-ins 2.94,
revision 8c60e757c30b1b239bae46532b4f6d4ebb1e1cad. Retain the exact source,
provenance, policy/selection manifests and separate MPL notices. Selection joins
exact certificate/trust identities and requires explicit original-verifier
capability review. Exclude four unimplemented leaf-issuance cutoffs, the Kamu
authority's externally imposed .tr constraint, ten unsupported authorities and
54 non-server-auth entries; never promote a restricted authority to unrestricted
trust. This is a conservative data subset, not the full Firefox trust policy.

PEM parsing, the custom store, TLS admission and path validation share a
1024-root bound. Retain the two-MiB regular-file input limit, twelve-MiB reusable
PEM decoder limit, 64-KiB certificate bound, eight-certificate presented chain
and eight matching root candidates. Growth follows actual input. Custom stores
replace the default and deduplicate exact certificates; unsupported custom
entries remain fail-closed, rather than receiving default-data filtering.
Hostname/SAN, validity, signatures, CA/usage/path and critical-constraint checks
are unchanged. There is no OS trust-store lookup, automatic root update,
revocation or CT implementation implied by this subset.

Refresh is an explicit reviewed offline operation. Pin source and restriction
hashes, re-audit candidates with the exact original native verifier, and update
the reviewed policy before generation. The generator refuses changed verifier
inputs, unreviewed authorities or unknown selection data; its default mode
checks exact committed output without downloading or installing anything.
Native regressions pin the full 98-root PEM, check every selected root's CA,
time and self-signature, preserve the prior eight identities independently of
order, and retain G3 identity, wrong-SAN and signed-path negatives. Original
custom-store/TLS tests verify admission above the old raw count, exact dedup,
real signed-path validation and refusal before writes; data-tool tests cover
reproducibility, source gates and strict negative cases. These native/tool checks
passed; full normal verification remains required for an implementation checkpoint.

Shared HTTP redirects give the initial exchange and each next admitted hop a
20-second allowance, clipped by one checked absolute 120-second overall deadline.
Pool acquisition, TLS, send, framing/decode and post-read retention share that
hop clock; a stale-GET retry does not renew it. The five-redirect limit, per-hop
policy/cancellation, POST/cookie/CORS and byte-accounting rules are unchanged.
Five original owned-loopback short-profile controls cover actual old-window
exhaustion versus two/three successful renewed hops, single-hop and overall
timeouts, cancellation and redirect/loop limits. These do not prove site access.
Existing caller admission/wait limits and independent watchdogs still apply;
blocking OS DNS can exceed a transport deadline, so it is not a strict process
wall-clock guarantee.

Shared TCP setup scans at most 256 resolver entries, retaining the first eight
distinct socket addresses per family. The resolver's first family stays first;
families then alternate, preserving order within each family, into at most eight
attempts. Filling one family does not stop scanning for the other. Hitting the
scan bound reports remaining results as unknown, including at exactly 256
entries; family-storage and attempt-queue omissions are reported separately.
Literal IPv4/IPv6 bypass DNS, and single-family hosts remain valid.

Connects remain serial, with the existing two-second per-attempt ceiling. When
an opposite-family candidate remains in the actual queue, a slow attempt receives
at most half the remaining hop/overall allowance, reserving time for fallback.
Failures advance immediately. Cancellation/deadline checks surround resolution,
enumeration, each attempt and socket setup; failed, cancelled or unconfigured
sockets are dropped. Blocking DNS and one bounded connect can still delay
cancellation. This is not concurrent Happy Eyeballs or asynchronous DNS.

Bounded diagnostics distinguish scanned raw family counts, retained unique and
selected counts, attempted families, elapsed time, numeric OS codes and error
kinds. The last error is labelled as one attempt, with no resolved-address list
or arbitrary error text. The original hostname remains authoritative for TLS
SNI/certificates, HTTP Host, URLs, origins, cookies and redirect checks. Original
offline regressions use explicitly simulated family failures and local sockets;
they do not establish external IPv6 availability or a live-service outcome.
The correction was integrated by
[PR22](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/22), with accepted source
[63ac9c6](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/63ac9c61054bed3f639904b3d94c91dff455d7bc).
Its recorded normal/optimized and local/hosted offline evidence covers this finite
correction, not completion of the broader T1 area.

`--diagnose URL --report FILE` uses the real HTTP/TLS client for document GETs,
without an App profile, page scripts, subresources or form submission. At most
eight targets are accepted; all are parsed before report creation or network
contact. Encoded/decoded bodies retain the two-MiB bound, including attachments.

Optional `--diagnose-allow-host HOST`, at most 128 entries and only with
--diagnose, restricts the initial destination and every redirect before DNS,
connecting or request transmission. ASCII DNS/IPv4 and bracketed IPv6 hosts use
existing parser canonicalization and exact equality; URLs, ports and wildcards
are refused. Subdomains/www variants need their own entries. Omitting this
option preserves the legacy unrestricted diagnostic redirect policy.

Reports retain the destination checked by the guard, actual exchanges and
measured phases. A last measured phase is not an inferred failure cause.
Metadata is single-line escaped and bounded to 4096 UTF-8 bytes per field.
HTTP statuses and possible body markers are observations; HTTP 200 or absent
markers establish neither access, challenge clearance nor page usability.
Diagnostics perform no challenge interaction.

Eight original native diagnostics regressions and the existing document-guard
regression passed. They cover host admission, zero-contact redirect refusal,
legacy redirects, actual failed-hop/phase evidence, body bounds/prevalidation,
status/marker separation and metadata handling. Full normal integration,
release/CLI execution and coordinated public-host outcomes remain separate gates.

Coordinated document checks distinguish improvements in trusted-path coverage
from remaining chain/parser failures, timeouts and intentional admission refusals.
Continue resource-specific investigation with the original TLS/X.509/App fixtures.
Preserve certificate validation, origins, CSP, cookies, cancellation and checked
bounds. Document responses do not establish page usability, challenge clearance or
general TLS compatibility; no challenge interaction is authorized here.

Proceed to demonstrated everyday resource/render/input gaps after that recheck,
then finite tabs, link contexts and windows; perfect TLS coverage is not a
prerequisite for all later page work. Completed media slices remain supported;
further F6/YouTube work is deferred.

## B1 — ordinary unmarked external stylesheets

Status: in progress for broader page usability. The bounded stylesheet path and
original native/App regressions are implemented; normal verification is required
for every integration checkpoint.

Load parser-authored and genuinely inserted unmarked LINK stylesheets over
HTTP(S), including legitimate foreign origins and redirects, using the existing
HTTP client and CSS renderer. Apply original request blocking, element nonce/CSP,
mixed-content and cancellation checks before each destination. Restrictive COEP
and response CORP remain enforced. Include credentials only through the existing
cookie host/path/Secure/SameSite rules and retain eligible response cookie effects
even when CSS publication is refused. Scripts, images, fonts and local-file
resources gain no new network permission.

Charge complete decoded response bodies even after policy or post-read refusal;
redirects share the request byte allowance. Require complete bounded responses
and unambiguous text/css for foreign sheets;
retain foreign taint across redirects returning to the document origin. Preserve
final-response CSS URL provenance. Cache reuse must reauthorize all redirect
destinations for the consuming element's mode/nonce/CSP, and nonempty integrity
declines before network or reuse while verification is unsupported. Any present
crossorigin attribute keeps the current same-origin subset; this does not claim
general stylesheet CORS. CSS imports and CSSOM rule/source access remain absent.

Dynamic source/mode/nonce changes, disabling, removal, Stop and navigation retire
the exact old request. Stale completion cannot paint or deliver author load/error
events, and accepted/retired work settles its real loading counts. Keep source,
decode, aggregate CSS and worker bounds and existing recovery behavior.

Acceptance: original owned two-origin parser/dynamic and redirected requests;
applied geometry and independently checked pixels; final CSS base and safe reuse;
pre-contact CSP/request/mode/mixed-content/integrity refusals; wrong/missing/
ambiguous MIME, CORP/COEP and incomplete response negatives; cookie/redirect
isolation; held-request replacement/Stop/navigation and capacity recovery.
Ordinary source/rule reads remain unavailable after foreign CSS applies. Neither
HTTP200 nor one fixture establishes complete CSS or public-page compatibility.

### B1a — finite viewport media rules

Status: bounded source behavior and original native/App checks are implemented.
Executable offscreen behavior and full normal verification remain integration
requirements; broader B1 page usability remains in progress.

Evaluate ordinary rules nested within matching @media groups at their original
cascade position. Support screen/all media types, nonmatching print, comma
alternatives, media-type only/not, conjunctions, exact/min/max pixel width and
height, and hover:hover/none. Unknown media types do not match and may be
negated; unknown features/values and malformed or unsupported alternatives do
not grant rules even under negation. Relative query units, boolean features,
MQ4 range/or grammar and feature-only negation remain outside this profile.
No matchMedia or general conditional-rule/CSSOM interface is created.

Use one host-only per-Document context: unrounded finite CSS-pixel viewport
dimensions, including the reserved scrollbar, and an explicit primary-input
profile. The Windows App currently selects its desktop mouse profile; this is
not touchscreen/primary-device auto-detection. Capture that context before
initial document staging and critical resource/font work; refresh it before
App reflow on resize or zoom, including source/resource report views. Inspector
and forced geometry use the actual document context. Standalone layout may use
its known dimensions with unknown hover; unsupported child contexts do not
inherit an invented parent viewport or pointing capability.

Retain each declaration's real stylesheet owner and final-response URL base.
Nested groups preserve order; inactive or unknown-feature groups cannot bypass
complete-source admission. Count containers and descendant blocks before
publication, with the existing generous source/document/rule/declaration/work
bounds and an on-demand checked iterative stack. A refusal or allocation error
must not publish a sheet prefix. Existing cancellation/resource policy and
foreign CSSOM restrictions remain unchanged.

Conditional font-face discovery stays unsupported and is never flattened into
requests. Initial top-level font faces use the same environment-aware computed
families as layout; new dynamic or resize-triggered font fetching is not added.
Style/link media attributes keep their empty/all/screen subset. CSS imports,
other conditional at-rules and wider layout support remain separate gaps.

Acceptance: original matching/nonmatching and fractional-threshold controls;
print/unknown/negated/malformed conditions; nested source order and stylesheet
ownership; inactive descendant admission, whole-sheet refusal and recovery;
initial font-use selection without activating nested faces; actual App geometry,
background pixels and link hit ownership across full-width thresholds and zoom,
with source/document lifetime retained. The executable offscreen sequence also
checks owned requests, paired source/layout/pixels and dynamic stylesheet
application. These proofs establish the finite local rendering behavior, not
physical desktop presentation or complete public-page compatibility.

## CP1 — real collections, calls and Node identity

Status: delivered prerequisite slice. Later source checkpoints retain the normal
native/build/executable integration gates. CP1 does not establish library
initialization or visible improvement; connected follow-up behavior is in CP2.

Use the genuine retained main Document and supported main Node objects. Each
childNodes getter returns one stable, live NodeList identity for that root. Reads
observe the actual immediate Element/Text children in tree order, including Text;
native indexed slots remain protected. The canonical owner NodeList.prototype.item
operation supports ordinary overrides/deletion, checks its native receiver and
origin before once-only unsigned-long index conversion, and reads current nodes.
Saved methods remain branded after deletion. NodeList.forEach
uses the owner Array intrinsic: it captures and converts effective length before
callback validation, then performs live HasProperty/Get with actual Node values.
The native length getter is read-only; ordinary own properties can shadow its
effective value for generic iteration. Out-of-range indices return undefined and
item returns null; HTMLCollection named aliases are
not introduced. Borrowed Array.prototype.slice reuses the existing host backend:
it captures initial length, performs ordered conversion/HasProperty/Get, retains
holes and returns an ordinary static Array of the same actual node identities.
Null/undefined slice receivers give an ordinary TypeError before argument
coercion; unrelated unsupported borrowed-array operations are not broadened.
Function.apply accepts admitted native array-like objects through the existing
call and transient-lifetime machinery. It captures/converts length once, checks
the argument policy, then Gets each position in order; absent positions become
undefined arguments. Getter failures and fatal limits retain their existing
semantics, target-callable ordering is preserved, and no partial argument list
invokes the target.

Element textContent replaces all children: nonempty text creates a fresh Text,
while empty text creates no child. Saved superseded nodes keep their original
identity/data and detached subtree relationships, and remain charged to the
document budget. Complete validation, checked accounting and storage reservation
precede publication. Loaded resources, native form/caption ownership and existing
inert-subtree mutation restrictions remain. A cached live list refreshes dirty
topology before returning new Text handles, preserving owner/realm checks before
first access or argument coercion. Native function ownership is not chosen by
the realm that happens to read a method first.

Normal host policy allows 65536 NodeList identities and 1048576 retained snapshot
references, with checked on-demand growth and a disjoint handle representation.
Explicit small profiles retain refusal/rollback/recovery witnesses. The original
1000-update semantic cases retain their workloads and verify fresh Text and
preserved old data within the normal budgets.

The tokenizer retains real parsed Comments in loaded, inert and checked fragment
paths. Template content has a real inert DocumentFragment with stable childNodes
and owner-document identity. Inert-owner fragment factories and same-owner
Element/fragment appendChild/insertBefore/removeChild preserve actual mixed-node topology,
saved aliases and donor emptying under atomic storage/copy/work admission.
Cross-document inert adoption, replaceChild and inert observation
remain outside that mutation slice; ancestry/resource refusals retain the existing
escaped host-error path. Hidden child Documents keep their existing collection
boundaries; ordinary list reads do not activate inert queries/resources.
Parsed, executed script Text is inspectable, while its forbidden content/removal/
move/non-noop normalize mutations remain refused without source/DOM/queue changes.

The genuine logical main Document exposes nodeType9, nodeName#document, its
connected root/child identities and correct null owner/parent/sibling facts. Real
Node.prototype nodeType/nodeName accessors read supported Document/Element/Text/
Attr slots, with stable descriptors, brands and owner checks. Prototype lookup and
saved canonical getters stay distinct. Readonly/strict writes, ignored arguments,
foreign holders/receivers and bounded recovery are tested. This adds no new
constructor, Document mutation, baseURI, nullable setter or child-frame scope.

Required original controls include the exact Document/root/Element borrowed-slice
case, live versus copied identities across detach/reinsert/replace, Text empty
lists, item coercion/brand/arity ordering, holes and abrupt completion, cached and
saved foreign-owner refusal, fresh-Text first access, capacity recovery, real App
input/render ownership and an unmodified selected-library workflow recheck.
Framework initialization, interaction, CSS layout and reference-browser comparison
have separate outcomes; cascaded errors do not count as new first blockers.

Semantic references: [DOM childNodes](https://dom.spec.whatwg.org/#dom-node-childnodes),
[DOM textContent](https://dom.spec.whatwg.org/#dom-node-textcontent), and
[ECMAScript 2025 Array.prototype.slice](https://262.ecma-international.org/16.0/#sec-array.prototype.slice), and
[CreateListFromArrayLike](https://262.ecma-international.org/16.0/#sec-createlistfromarraylike).

## CP2 — genuine Comment and native RegExp input conversion

Status: delivered prerequisite. Focused original Page, App and language controls
cover the finite slice; each new checkpoint requires normal native/build/executable
verification. At the historical CP2 checkpoint, the unchanged legacy workload next
refused a select-containing innerHTML fragment. CP4's later unchanged recheck reached
hidden-input mutation; CP7 now advances to an input-containing innerHTML fragment.
Library initialization and visible workflow improvement remain unproved.

Main Document createComment performs required DOMString conversion and creates a
real retained Comment leaf with nodeType8, nodeName#comment, actual owner/topology
and data/UTF-16 length. Supported data/nodeValue/textContent updates, cloning,
ordinary tree insertion/removal, live child lists and bounded serialization use
the real node. Comments separate Text runs during normalize. Shared node/data
admission, allocation preflight, readonly/brand/owner rules and rollback remain.
Comment data does not become Element selector membership, descendant Text,
button/form captions, executable script/style source, a resource request, a
rendered box, an intrinsic/flex item or a link hit. Original App controls verify
real nodes with preserved text/layout/hits and resource ownership.

The main document exposes the Comment constructor and its CharacterData-based
prototype. The shared tokenizer now retains parsed comments in documents,
inert documents and checked fragments. Literal data, newline preprocessing,
closing/EOF behavior, node admission, SVG provenance and real clone/import/splice
ownership are covered by original connected controls. Full CharacterData and
DocumentType surfaces, template activation and child-context factories retain
their separate partial contracts. Loaded-resource, origin and cancellation
boundaries remain.

Existing native RegExp exec/test paths now perform actual input ToString, using
String-preferred conversion with ordered symbol/ordinary getters and callbacks.
Genuine existing Function source conversion is reused, including public conversion
overrides. Receiver branding precedes input conversion, which precedes matching
state/lastIndex reads. Thrown identity, reentry, owner/result/error realms,
transient lifetime and shared step/copy/regexp budgets retain their rules; fatal
capability/quota failures are not swallowed and do not reset the job allowance.
Generic RegExpExec/custom exec, object-valued lastIndex conversion, constructor
pattern/flag conversion and other string/RegExp protocols remain unchanged.

Required original controls cover Comment data/identity/leaf/topology, normalize
barriers, query/text/script/style/render/resource exclusions, allocation/owner
refusal and actual App input/render ownership; RegExp Function/object conversion,
hint/order/callback identity, invalid receivers/Symbols, lastIndex/reentry,
cross-realm behavior, native denial and bounded recovery. Framework rechecks keep
the next first blocker separate from later missing-library cascades. Selected
CP2 Test262 checks retain 18 passes and two ordinary Array constructor failures
across 20 mode runs, with all 30 runner controls passing and unchanged inputs;
this is no full-suite conformance claim. The select fragment was the historical
CP2 first blocker. CP3 subsequently delivered the independent responsive CSS
improvement; hidden-input admission is a separate CP7 prerequisite.

Semantic references: [DOM createComment](https://dom.spec.whatwg.org/#dom-document-createcomment),
[Comment](https://dom.spec.whatwg.org/#interface-comment), and
[ECMAScript 2025 ToString](https://262.ecma-international.org/16.0/#sec-tostring).

## CP8 — genuine Node.getRootNode over ordinary trees

Expose the real zero-arity Node prototype method through ordinary property
resolution, override/deletion and borrowed-call behavior. Require a genuine
accessible Node receiver before options hooks. Read the optional dictionary's
composed member through observable Get and ToBoolean; preserve getter effects and
exceptions, then walk the current tree after those effects. Nullish options use
the default; non-dictionary primitives refuse. No Node-shaped object or direct
host call may bypass dictionary conversion or receiver/access validation.

Return the genuine Document for a connected ordinary tree, the actual root of a
detached subtree, and the Attr itself even when it has an ownerElement. Supported
same-origin child trees retain their own Document identity; cross-origin access
refuses before options hooks. Both composed values share the ordinary root because
ShadowRoot/shadow-tree APIs remain unavailable. Preserve bounded work, edge/index
identity and cycle checks; malformed ancestry refuses without a DOM commit.

Nine Node root controls, four existing Node constants and thirty-two namespace
controls pass. An initial new-test oracle compared a returned Document with a
method; correcting that expectation required no production change. The same
original trusted-click App fixture now gives zero script errors, genuine
connected/detached/reconnected roots and changed visible text/pixels. Its clean
diagnostic build is distinct from the earlier dirty workload builds. This is not
isolated-crop, physical desktop or full candidate acceptance.

Modern jQuery fixture revision2 now advances to anchor href assignment at7963:1;
initialization/input/async still fail. Bootstrap retains class syntax at599:3 and
its separate initialization-dependent dropdown blockage. Earlier fixture revision1
startup and HarnessBlocked records remain historical. Full normal regression and
hosted integration are still required for the final candidate.

## CP7 — finite hidden-input admission and native form state

Admit hidden type on a fresh detached HTML INPUT through the genuine native form
model. Existing parsed hidden inputs share bounded attribute and IDL name/value/
defaultValue reflection with the hidden default-value mode. Whole-node insertion,
removal and movement preserve identities, current tree/form ownership and connected
submission order. Hidden controls never gain a layout box, hit target or focus,
including when author CSS requests display and dimensions. Unrelated edited
controls retain their values and caret/selection state.

Stage attribute, tree and native form changes before publication. Preserve host
work/copy/value/control limits and namespace/foreign-owner refusal; logical limit,
namespace and owner refusal publish no partial DOM or native state. Existing
process-OOM limitations remain. Existing native input-type
transitions, changing INPUT contents and general input-containing innerHTML
fragments remain refused. No library-specific path or quota expansion is admitted.

Nine focused controls and twelve adjacent select controls pass. Actual App software
pointer down/up creates the hidden control and changes only the authored text in
an isolated white crop; down alone does not run the action. The test checks no
hidden box/focus despite author display rules, native form request serialization,
neighbor value/cursor preservation and unchanged source/location/history. A
foreign-namespace negative first reproduced a failure and then passed the atomic
guard. These are bounded model/App results, not physical desktop, HTTP submission
or whole-library acceptance.

The unchanged W2 workload advances from hidden type mutation at jQuery1.12.4
1265:4 to `div.innerHTML = '<input/>'` at2684:2. Load/initial render pass while
initialization/input/async still fail. Its explicitly dirty development build and
separate earlier failures remain recorded; normal regression and hosted integration
are still required for the final candidate.

## CP6 — bounded String.prototype.concat

One genuine native method uses the existing generic String receiver conversion,
then converts arguments once in order with the string hint and returns a primitive
string. Live conversion hooks may replace later hooks, reenter the method or throw;
saved native identity, callback realms and abrupt-completion identity are retained.
The method has native arity one and ordinary writable/configurable, non-enumerable
prototype metadata. Existing native-capability denial, byte/copy/step allowances,
root guards and explicit lone-surrogate refusal remain; no Array.concat change,
polyfill or library-specific route is admitted.

Nine original VM/realm/App groups cover generic/boxed receivers, argument and hook
order, reentry, Symbol and abrupt failures, metadata/prototype changes, native
denial and budget recovery. Actual software pointer down/up changes independent
DOM/layout text pixels; down alone does not activate. Four adjacent substring
groups pass. The unchanged Vue workload advances to object-prototype access and
still fails initialization/input/async. This is a prerequisite, not whole-library
support or physical desktop evidence. Normal regression and hosted integration
remain required for the final candidate.

## CP5 — finite object-literal prototype setter

Status: delivered finite prerequisite, integrated by
[PR25](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/25) from checkpoint
[fb385cb](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/fb385cb6c0b06988d55beb8c60f9f9a7cab6e431).
Ten original language/App controls and recorded normal/build/executable integration
cover that checkpoint. New checkpoints still require exact verification.
Only static noncomputed colon definitions
whose name is `__proto__` set the new object's existing internal prototype link.
Null removes the link, ordinary objects/functions retain exact identity, and
other primitive RHS values are evaluated once and ignored without coercion or
own-property creation. The RHS keeps source order and abrupt completion; it does
not infer a property function name. Duplicate special setters are early syntax
errors before script effects, including quoted/escaped static names and primitive
RHS values. Existing member/name, chain, cycle, extensibility and work bounds apply.

Computed keys, shorthand, methods, accessors, JSON own keys and legacy `__proto__`
reads/writes retain explicit unsupported boundaries. Host objects remain outside
script prototype chains; a native refusal does not invoke host getters/setters or
turn a fatal capability error into an author-catchable success. Ordinary field
descriptors and unrelated Object.prototype behavior are unchanged.

An original App fixture uses real pointer down/up twice to create and retain a
null-root state bag across input jobs. Exact DOM/layout and isolated white-region
text pixels change I to WWW to III; down-only does not run the handler. Ordinary
fields, null-root identity, zero script errors, loaded source, location, history
and realm identity remain genuine. This is software App input/paint proof, not
physical desktop or framework initialization evidence.

One unchanged pinned Vue 2.7.16 W3 invocation on clean abe2e3b, at 1280x800 and
640x800/zoom100 with JavaScript/images enabled, now admits the whole file past its
prior literal parse refusal. Both executions first fail at line11,column3613:
an undefined noncallable concat member. This does not prove the later prototype
literal ran. Loading, initial rendering and independent Foundation CSS pass;
Vue initialization, reactive input and async stages fail in both configurations.
Ordinary prototype membership/access was unobserved at that checkpoint; the
later CP6 recheck now reaches that separate boundary.
No library edits, polyfill, reference substitution or visible Vue improvement is
claimed; later missing-library errors remain cascades.

Semantic references: [ES2025 object-initializer early errors](https://262.ecma-international.org/16.0/#sec-object-initializer-static-semantics-early-errors),
[PropertyDefinitionEvaluation](https://262.ecma-international.org/16.0/#sec-runtime-semantics-propertydefinitionevaluation),
and [OrdinarySetPrototypeOf](https://262.ecma-international.org/16.0/#sec-ordinarysetprototypeof).

## CP4 — conventional select fragments and native ownership

Status: delivered finite prerequisite, integrated by
[PR24](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/24) from checkpoint
[73f3315](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/73f3315ae23760966efb1c1e6bf8ff3e2492c6c4).
Recorded normal/optimized and local/hosted offline verification covers that
checkpoint. The published 0.81.0-alpha.2 source includes this prerequisite.

Twelve original parser, native-model, Page and App controls pass. The recorded
integration covers this finite slice; each new checkpoint still requires exact
normal/build/executable verification.
One unchanged JS-enabled W2 recheck at 1280x800/zoom100 on clean5cee344 advances
past the select fragment to jQuery1.12.4 line1265,column4: setting INPUT type to
hidden was explicitly unsupported at that checkpoint. Loading and initial rendering pass;
initialization, dropdown/form input and async stages fail. Later Bootstrap and
missing-library errors remain cascades. No visible workflow improvement or
framework initialization is claimed.

Checked ordinary HTML fragments admit a finite select/option/optgroup grammar,
including conventional omitted endings, normal attribute preprocessing and real
single-select defaults. Comments retain their DOM positions without entering
native option labels or values. Unsupported recovery, rich descendants, foreign
content, resources and parser bounds retain explicit refusals. Pure parser
select contexts do not enable mutation inside an existing native control.

New native controls are prepared independently with genuine option/group node
identities and bounded control/option/value/retained-payload accounting. Existing
editors, caret/selection and selected values are not reconstructed. Whole-select
container insertion, removal, replacement and movement stage prospective parent,
nearest-form and disabled-fieldset/first-legend metadata before publication.
Detached controls retain identity/value while leaving layout, focus, popup, hits
and submission participation; reconnect restores native participation without
restoring retired focus. Submission walks the connected document in tree order.

Logical limit, work, origin, owner and allocation-preflight refusals precede tree
and native publication. Existing std BTreeMap physical allocation failure can
still abort the process; recoverable process-OOM atomicity is not claimed.
New select explicit form associations, existing-control interior mutation,
createElement(select), multiple/large select controls and
customizable select content remain unsupported. Existing resource and ID-owner
guards are unchanged; no framework-specific fallback or library edits are used.
Hidden-input admission was outside CP4; CP7 adds its separate finite native path.

The App controls exercise actual software paint, popup choice and input/change
events, then timer-driven removal while focused with an open popup. Stale points
cannot change the detached value; reconnection preserves DOM/native identities,
form order and an unrelated edited input's value/caret/selection/offset. These
controls do not claim physical desktop or external HTTP submission coverage.

Semantic references: [HTML fragment parsing](https://html.spec.whatwg.org/multipage/parsing.html#parsing-html-fragments),
[select selectedness](https://html.spec.whatwg.org/multipage/form-elements.html#selectedness-setting-algorithm),
and [form owner reset](https://html.spec.whatwg.org/multipage/form-control-infrastructure.html#reset-the-form-owner).

## CP3 — responsive stylesheet-to-layout behavior

Status: delivered finite rendering prerequisite, with the unchanged
Foundation 6.9 fixture and original App controls integrated before B2. The source
[5d2f115](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/5d2f1151e58a6d16ec62a5c1cb3a06cc90d6497b)
records this shared rendering correction. Each new checkpoint still requires
exact normal/build/executable verification.
At 1280x800, zoom100 and normal JavaScript, the original panels now share a row;
at 640x800 they retain their original stacking. Vue execution is separate; the
historical W3 first-root failure at that checkpoint is recorded under CP6.
No reactive interaction or full-framework pass is claimed.

Attribute selectors share one parser and matcher across CSS and supported DOM
queries, `matches` and `closest`. In addition to presence, exact and token matching,
support dash-prefix, starts-with, ends-with and contains operations. The three
substring operations never match an empty operand; dash-prefix retains its exact
or immediately-followed-by-hyphen rule. Preserve quoted punctuation literally,
existing HTML attribute case defaults and ASCII-only `i`/`s` flags. Unsupported
selector-string escapes and namespaces still reject the whole selector list.
Retain the512-byte per-selector,16-member list and existing query/result limits.
Contains uses a selector-bounded prefix table and a linear borrowed scan; query
string work is debited before inspection, including inside negation. No DOM
attribute copy, lowercased attribute allocation or unmetered quadratic search is
introduced. Original local trusted App coverage binds inert script metadata and
ordinary Element queries to actual CSS marker pixels and retained native input.
See [attribute matching](https://www.w3.org/TR/selectors-4/#attribute-substrings).

Finite em media lengths use the same browser-owned initial16CSSpx font basis as
default Style initialization. Author html/body fonts do not change that basis;
the real CSS viewport lifecycle supplies resize and zoom conditions. Preserve
finite/overflow checks, unknown/malformed negation refusal and inactive-block
admission. Wider units and conditional/dynamic font fetching remain separate.

Recognize terminal double-colon before/after stylesheet targets while preserving
atomic selector lists. They never match an ordinary Element, contribute another
alternative's specificity or generate painted content. Unknown/malformed list
members and existing selector/work caps still refuse atomically. DOM query
pseudo-target support remains outside this stylesheet-only slice.

Apply explicit box-sizing inherit from the actual parent; initial/unset select
the noninherited content-box initial value. Preserve ordinary cascade and natural
noninheritance. Retain fractional flex main/basis/min/max sizes during line
admission/distribution and round at existing cumulative rectangle allocation;
two50% border-box items fit odd widths, while genuine content-box or minimum-size
overflow still wraps. Keep existing length/work/geometry limits.

Original native controls cover author-font independence, fractional em thresholds,
invalid/overflow/inactive media, pseudo/list specificity and refusal, explicit
versus natural box sizing and fractional flex sizing/overflow. Actual App tests
keep1023/1024,640/1280,125% zoom and odd-width geometry, exact padding/guard pixels,
link hits and unchanged document/source/epoch. The pinned Foundation workflow
retains original assets, JS mode, viewport and independent image-pixel checks;
its remaining runtime error stays visible. Resource/CSP/origin/cancellation and
stylesheet admission negatives remain mandatory affected checks.

Semantic references: [Media Queries relative units](https://www.w3.org/TR/mediaqueries-4/#units),
[pseudo-elements](https://www.w3.org/TR/selectors-4/#pseudo-elements),
[box-sizing](https://www.w3.org/TR/css-sizing-3/#box-sizing), and
[flex line collection](https://www.w3.org/TR/css-flexbox-1/#algo-line-break).

## B2 — finite tabs, link context and new windows

Status: completed finite milestone after the measured responsive improvement;
shared authorities, Session ownership, native tabs/windows and explicit link/context
actions were integrated by [PR23](https://github.com/c-a-t-f-o-o-d/chezcat-browser/pull/23).
The accepted source [67c5d69](https://github.com/c-a-t-f-o-o-d/chezcat-browser/commit/67c5d691012eeb39767e44000d9c1aeac8cdea1a)
and subsequent checkpoints 73f3315 and fb385cb have recorded local/hosted offline
verification. Separate actual Windows desktop records cover six groups at 67c5d69,
six at 73f3315 and four at fb385cb. Each is limited to its observed source/scope;
the published 0.81.0-alpha.2 source at 36adf20 also has six scoped desktop groups
and its own exact-source and extracted-runtime verification. Desktop checks apply
to affected native behavior; full normal/optimized and hosted verification remain
integration gates. Provide new/switch/close
tabs and link actions for new tabs or ChezCat windows, including supported middle/
Ctrl-click and keyboard actions. Keep independent page/history/scroll/loading and
address/title state with deliberate shared profile/cookies. Acceptance must prove
current-page preservation, tab-scoped Back/Forward, focus transfer, bounded inactive
work and no worker/script/media completion crossing a closed or different owner.

One UI-thread shared profile authority retains the existing Store's exact-byte
disk conflicts, file lock, bounded serialization and atomic publication. App
views own immutable revisioned snapshots; no dynamic borrow guard crosses App
work. Saves require the captured revision, including after view refresh. Busy,
stale, unavailable, read-only, external-edit, invalid-size and exhausted-revision
refusals preserve committed state. Valid no-op saves retain existing Store
semantics and do not consume a revision or create a missing file.

Settings drafts retain their original token through nested rule editors.
Bookmark manager indices, destinations, folder IDs and press/popup/drag actions
cannot be rebound by peer changes, including deleted/reused folder IDs. Stale
publication/navigation refuses while Cancel and local draft editing remain usable.
Explicitly reopening a panel captures fresh data and authority.

Peer refresh adopts a profile snapshot without navigation, reflow, VM/form/history
replacement or session JS/images/zoom changes. Effective bookmark-bar visibility
stays with the App until successful local Settings Apply, preserving viewport,
media environment and geometry. Paint-only appearance preferences adopt at
refresh, including rebuilding the browser-owned link-color cache. New Apps use
the shared persisted defaults. Original two-App tests exercise actual input and
a retained JavaScript callback, typed form state, stale nested drafts, bookmark
index/folder reuse, popup/drag/press refusal and fresh-action recovery.

Stable coordinator routing, browser-owned downloads and wake authority, shared
history shutdown and terminal resource admission remain prerequisites for real
tab close. A standalone cancellation helper or dropping an App is not acceptance.

The download ownership prerequisite retains one original bounded Manager in a
browser authority, with weak App views and owned entry snapshots. Dropping an
originating App/view does not cancel a session transfer. Existing single-window
Apps retain their own authority, and startup shared attachment refuses any
retained active or finished work instead of silently replacing it. Popup state
remains App-local; busy/closed operation failures are visible. Original limits,
TLS, response validation, explicit received-POST/no-replay behavior, cancellation,
retry, part-file cleanup and no-overwrite publication remain unchanged.

Workers use a stable retargetable browser signal dispatcher. It releases its
mutex before invoking or dropping callbacks. Page wake changes cannot retarget
an externally owned browser download route. An already-cloned callback may still
run after retargeting; native routing must reject closed/reused WindowIds
and use nonblocking signals. The adapter alone does not provide native lifetime
acceptance. Original held-transfer/two-App, wake reentry/retarget, admission,
cancel/retry/no-overwrite, received-response, closed-authority and legacy popup
controls cover this integrated prerequisite; full normal integration remains required for each new checkpoint.

Saved browsing history retains the original Store's locks, bounded data, exact
disk conflicts, expiry and atomic publication under one browser authority.
Weak App views own immutable snapshots; neither view drop nor attached App close
performs final clear-on-exit. Explicit final authority shutdown runs once after
success; a failed shutdown remains retryable. Legacy single-App startup retains
an explicit local owner and its existing exit behavior.

History selection captures the full observed Entry independently of later view
refresh. A changed or reused ID cannot silently open or remove a replacement;
peer refresh invalidates the selected action and its armed pointer. Settings
capture an opaque original-policy token separately from edited values. Visits
and expiry preserve that token; observed policy changes, including A-to-B-to-A,
replace it. Apply checks the token and original persisted policy inside the
existing locked transaction. Unobserved external A-to-B-to-A edits cannot be
detected without a persisted revision and are not claimed. Busy, stale, closed
and external-policy failures preserve committed state and remain visible.
Original App/history controls cover these finite ownership behaviors; stable
coordinator routing and complete terminal retirement remain separate acceptance.

The internal Session core adopts the initial App's existing profile, download
and history authorities without reopening files or replacing a live Manager.
Fresh boxed Apps keep independent realms, forms, documents, Back/Forward,
scroll and loading state. They share the cookie jar and immutable trust, retain
their own request origin/context, and copy only configured base fonts.

Full typed TabId/WindowId values include a checked process-wide Session generation
and nonreused checked serials. Diagnostic serial getters are not routing IDs.
Closed, wrong-window and foreign-Session identities refuse access. Bounded typed
wake queues coalesce events and request a finite sweep on overflow; round-robin
poll budgets and deadlines keep inactive timers/resources progressing.

Closing the active tab prepares its actual survivor through the normal snapshot
and resize path before committing close; observation failures preserve the
requested owner. All closed routes disappear before consuming the Box. Teardown
retires App/realm media tokens and drops realm/resource workers directly, without
Stop/reflow, author callbacks or new resource admission. Browser downloads
survive page closure. Explicit Session shutdown alone ends browser downloads and
finalizes shared history; failed history cleanup remains retryable while refusing
new owners. Session Drop does not silently clear history.

Nine original controls exercise real VM/form/source/scroll/Back isolation, input
capture transfer, resize/close activation, stale IDs/wakes and bounded inactive
timers, held scripts/stylesheets/images/backgrounds, live-download adoption,
decoded media-token retirement and final-only history/readonly retry. This is an
internal App ownership slice. The native shell now routes through checked numeric
registry keys and complete typed owners, with no App pointer in native userdata.
It initializes the UI from actual client bounds before refresh or input, including
hidden windows. The tab strip composes the active App and supports mouse tab
selection/close/new actions, overflow scrolling, Ctrl+T/W/Tab/Shift+Tab and Ctrl+N.
New-window failure retires only its reserved native route and rolls back its new
core owner. Platform continuations retain their original App/document/editor and
typed tab identity across modal reentry; logical owner changes refuse late results.

Detached inspectors stay pinned to their actual tab, including inactive tabs.
Routes suspend before consuming App mutations and retire before native destruction.
No Session/App/registry borrow spans Win32, modal, clipboard or painting calls.
One scheduler preserves earlier deadlines and transfers to a surviving window.
Translated characters retain their originating tab; switching tabs purges old
translations without swallowing WM_QUIT. Only successful final history shutdown
posts quit; failed cleanup leaves a non-browsing retry host.

Seven original continuation controls, ten pure Session UI controls and eight
native controls cover these finite behaviors. Three native controls create actual
hidden task-owned HWNDs on dedicated threads, exercise real message dispatch and
destruction, and use a real read-only history failure/retry. Unexpected test-only
modal errors are recorded and fail outside native callbacks. They use no OS input,
owner HWND, live site or native clipboard. They do not establish physical desktop
interaction or link dispositions/context menus. Full normal/optimized executable
integration is required for every checkpoint.

Native link routing now consumes the anchor engine and checked Session admission.
Genuine
primary click and middle auxclick use the existing owner-realm MouseEvent brands,
actual host coordinates/modifiers, listener path, passive/inline cancellation and
microtask checkpoint. Invalid targets and failed bounded dispatch veto default;
ordinary author throws alone do not cancel. Synthetic Mouse dispatch is unchanged.
Unmeasured target-relative offset/layer coordinates remain explicitly unsupported.

An opaque App hit captures the actual rendered HTML anchor, raw href/download,
document/source/effective base and geometry identity. Stale, foreign, dragged,
obscured and same-href replacement targets refuse before author code. The native
scrollbar owns its painted strip. After callbacks, cancellation or queued source
navigation suppresses default while preserving accepted author effects. Otherwise
the same surviving node's current href/download drives default, including author
movement. Authored downloads remain source-owned after cancellation checks.
Browser context Open consumes a stricter original token and fabricates no click.

Owned non-cloneable targets carry the already-resolved Location and file root,
plus the source top-level cookie initiator. Consuming a target uses a new GET
chain, including explicit opaque-local origin, without POST replay or destination
origin/reload fallback. Session admission validates full original owner IDs and
active membership, preflights caps/identities/storage/shared-service observations,
and publishes background owners without temporarily activating them. Failures
retain existing owners; new-window native creation failure retires its destination.

| Native link input | Disposition after author cancellation/download checks |
| --- | --- |
| Primary | Current tab |
| Ctrl + primary | Background tab |
| Ctrl + Shift + primary | Foreground tab |
| Shift + primary | New ChezCat window |
| Middle | Background tab |
| Shift + middle | Foreground tab |
| Context command | Explicit foreground tab, background tab or new window |

The existing native Alt-only download shortcut retains its path. Other Alt/Meta
bits remain real event data and add no disposition. Authored download attributes
remain source-owned, including context actions; commands fabricate no DOM click.
Named browsing contexts, authored target handling and script-opened windows are
outside this finite user-controlled disposition slice.

Native pointer capture has a checked full-owner/generation lease. One-shot release
envelopes carry the original posted screen point and modifiers. Missing, sent,
stale-owner and older-generation releases cannot cancel or consume a replacement
press/capture; valid releases finish once. Jitter preserves capture and drag-return
never reacquires a link. Non-link controls, media, selection and bookmark drops
retain their original release behavior with native legacy link default disabled.

Context origin identity is captured before native coordinate/capture calls; popup
completion rechecks the original source, geometry and guarded menu scope. Passive
hover and leave retain that scope; dragging and any held-button native move retire
it, including all five button bits without capture. Ctrl/Shift-only movement is
passive. Other keyboard/text, button, wheel, drop, navigation, owner, document,
live-href and native move/resize invalidations remain. No
Session/App/registry borrow spans popup creation, tracking, destruction or native
reentry. Keyboard context requests without a measured anchor keep default native
behavior. Temporary owned popup focus/capture cancellation preserves its token.
Original factory7, App/UI19 and native16 controls cover these additions; the native
controls use actual hidden task-owned HWNDs under the original serialized fixture,
including creation refusal, exact payloads, owner/press ABA and capture exhaustion.
Those hidden-window controls alone do not claim physical menu interaction.
The separate actual desktop acceptance recorded above establishes the finite
checkpoint behavior. Full normal/optimized and actual desktop acceptance remains
required for each new delivered checkpoint.

HTML anchor href/download content mutations use existing conversion, retained
limits and atomic DOM/Attr identity commits. Mutation alone navigates nowhere.
Layout destinations and UA link styling read live null-namespace attributes,
including removal and empty re-addition. Namespaced/uppercase metadata stays
inert; unrelated element, foreign/resource and NamedNodeMap mutation guards stay
unchanged. Original focused controls cover these finite semantics; full normal
and optimized executable integration remains required at each checkpoint.

Local-file identity controls include genuine Windows short-path aliases when
available and compare complete accepted browser Locations. Held-resource controls
wait for both the actual committed App document and the withheld response, force
the parser-image-before-first-paint order, and explicitly release only after
close. Test-only weak worker probes and server completion acknowledgments replace
fixed sleep assumptions; original page/form/source/pixel and no-escape assertions
remain. A held-server timeout never publishes a response.

## A1 — About and running-build identity

Status: finite source/UI implementation, with normal build and actual executable
acceptance required for integration. Use the existing browser-owned native panel
and menu input paths. Keep the underlying document/source, history, scroll and
resource workers owned by the same page; About Escape closes the panel rather
than issuing Stop. Closing returns to that page without reload or navigation.

Display the Cargo base version with a development label `+dev.N.g<12-character-SHA>`
when the clean embedded source is on the complete public main first-parent path;
N counts that path through the embedded source, not through a newer main tip.
Existing Cargo build metadata appends `.dev...` instead of a second `+`.
Unavailable ordinals use a SHA-only development label (or `dev.unknown`) and an
explicit unavailable status. Keep the full source SHA and honest
origin/clean-dirty-unknown state authoritative. Declared tagged builds display
the exact Cargo version; Cargo, PE and license versions remain separate and
unchanged. Metadata is a build-time description: an exported
revision does not establish an unchanged tree, and a declared tagged build is
explicitly publication-unverified. It authenticates no publisher, remote tag,
release, executable or signature. About performs no runtime Git or network work.

Native Build/License/Scope/Notices controls expose the exact embedded root
LICENSE, LICENSE_SCOPE.md and main THIRD_PARTY_NOTICES.md index offline, using
read-only text with keyboard focus, scrolling and resize. The complete referenced
third-party terms remain in the distribution's packaged notice/license files;
this finite viewer does not add routes to every referenced file. It reopens no
acceptance flow, reads/writes no receipt and changes no user/profile setting.

Copy build information uses a fixed allowlist of product/build/source and license
version/digest fields. Exclude page markup/activity data, paths, profile/history,
usernames and receipts. The native action uses the existing owned Windows Unicode
clipboard backend. A development input harness may capture only the actual typed
safe payload into local evidence; it must not call the OS clipboard or accept
arbitrary external actions. This proves App action/payload behavior, not successful
native clipboard delivery or authenticated build identity.

Acceptance includes real menu/keyboard/pointer selection and focus; complete legal
text identity in the actual rendering document; text scrolling/resize and paired
native-panel pixels; safe Copy action through the real control paths; canceled
press/closed-control refusal and recovery; preserved page/history/scroll and held
resource-worker completion across About open/Escape; agreement with the running
development/packaged executable's embedded metadata. Full normal verification,
build/PE and package checks remain required. Installer, release discovery and
update controls are separate planned features, with no placeholder update button.

## I1 — one Windows installation package

Status: preserved backlog, deferred behind the current browser-foundation queue.
Prefer one per-user x64 MSI,
retaining the portable ZIP and complete exact validated runtime payload, trust
provenance, assets/pages and license/notices. Use standard Windows installation,
shortcuts and uninstall registration, without a privileged updater service or
developer-tool requirement. Keep profiles/settings/bookmarks/custom trust and
acceptance receipts outside replaced program files; do not overwrite a running
executable or change browser/OS trust defaults.

Choose and test monotonic MSI numeric versions, full SemVer and prerelease/channel
identity before publication. Isolated acceptance covers fresh install/launch,
upgrade, supported repair/reinstall, cancel/failure recovery, uninstall and data
preservation. Report code-signing status honestly; no protection-disabling or
self-signed consumer-trust shortcut is part of installation.

## U1 — opt-in published-release discovery

Status: planned with delivery work. Provide a persisted Check for updates on
startup toggle, default OFF, and a manual Check for updates now action. Enabled
work is asynchronous, bounded, rate-limited and cancellable, never startup-
blocking. OFF makes no unsolicited update request. Keep useful checking/current/
available/offline/error states, version/notes and a safe official release action.

Use authenticated HTTPS separate from page navigation, without page cookies,
accounts, device identifiers, telemetry or history. Disclose ordinary connection
metadata sent to the official release host. Discover actual published milestone
releases, excluding drafts, branch builds and Actions artifacts. Compare full
versions and alpha/stable channels deliberately, with no silent stable-to-alpha
enrolment or downgrade; latest-stable-only discovery is insufficient for alpha.
Original local fixtures prove persistence, zero-contact OFF, eligible versions,
malformed/oversized data, cancellation/TLS/offline/rate-limit handling and honest
failure states. Routine CI does not poll live releases.

## U2 — explicitly approved authenticated upgrade

Status: planned after I1 and an implemented authenticity/recovery mechanism.
Checking does not approve download, install, termination, restart or new terms.
Before execution verify product/architecture/channel/version, complete hash and
authenticated publisher/signature or an explicitly designed authenticated manifest;
a nearby checksum alone is not publisher authentication. Reuse the chosen package
mechanism, stage outside active files, preserve data and require explicit restart
and changed-terms acceptance. Silent unattended installation is separate scope.

Original isolated acceptance covers wrong/truncated/replayed/downgrade/unauthentic
packages, cancellation, failed/interrupted upgrade and actual recoverable success.
Until this passes, U1 may open the official download page but does not claim a
self-updater. An authenticity blocker affects U2 execution, not ordinary browsing
development or release discovery.

## F1 — replaceable animation-frame operations

Outcome: page wrappers can replace, delete and restore main-Window
requestAnimationFrame/cancelAnimationFrame while saved natives still schedule and
cancel real callbacks. Keep shared measured timestamps and next-frame deferral.

Scope: existing main-document scheduler, ordinary configurable properties,
required arguments/callback errors, numeric cancellation conversion, root identity,
checked queue/identifier limits and disabled/foreign-owner refusal before effects.
Child schedulers and full event constructor families remain separate.

Acceptance: descriptors and aliases; getter-before-argument evaluation; no hidden
native restoration after deletion; real wrapped callback/cancellation/microtasks;
small-profile failure/recovery and generous default; saved call/apply/bound owner
controls; disabled scripting; normal suite/build/PE and a real local page journey.
Implemented for the main window. Acceptance is exercised by
src/script_timer_global_tests.rs, src/script_blank_frames.rs,
src/app_script_tests.rs and tools/check_animation_harness.py with original
animation-global-properties.js/animation-wrappers.html fixtures.

Saved/unbound/null/undefined calls use the native owner global; incompatible
receivers fail before IDL conversion and unsupported child targets explicitly
decline. Strict/sloppy/bound/arrow callbacks preserve their real realm semantics.
The queue grows fallibly on demand up to the adjustable normal65,536 policy; small
profiles test refusal/atomicity, full-u32 ID exhaustion and fresh-job recovery.
Frame and Promise-checkpoint traversal shares measured work/deadline limits.
Ordinary exceptions report and continue; fatal jobs discard stale work, remain
inert until begin_job and do not produce a second checkpoint error. A normal
4,096-callback drain and actual rendered wrapper journey cover useful capacity
and behavior. No child scheduler, legacy event factory or live-site acceptance
is added by this contract.

## F2 — navigation, providers and session history

Outcome: submit input, read/click actual results, reach a destination, use one Back,
and submit a different input in the same session. Preserve direct URL/file and
fragment intent and explicit None; no network-error-to-search fallback.

Acceptance: original loopback forms/redirects/replacements and address fixtures;
encoding/Unicode/punctuation; history/document identity; cancellation and cookie/
origin boundaries; persisted settings/cancel/save conflicts and failure recovery.
Automatic Location navigation selects replacement at request time only while
the target document is not completely loaded and its incumbent Window lacks
transient activation. Load-event callbacks precede complete loading even when
readyState is complete. Preserve ordinary later navigation and trusted-input
activation; synthetic DOM dispatch does not create activation. Original loopback
controls cover startup/load/microtask bridges, explicit replace, HTTP redirects,
completed-document navigation and exactly one Back followed by another query.

Implementation: request-time Location handling uses separate complete-loading and
bounded native activation state, with incumbent caller isolation and same-origin
descendant notification. Original check_navigation_harness.py exercises all six
bridge controls with real App input, HTTP response identities and paired pixels.
Native tests cover expiry, synthetic dispatch, child creation timing, reserved
keys, fragments and resource preservation. This is finite offline acceptance.

Opt-in public evidence records version/hash, entry path, session and each stage.
A challenge/denial stops that service and requires the human step once; elapsed
time, a fresh process, HTTP200, a title or zero script errors is insufficient.

## F3 — script-controlled supported local video

Outcome: the existing bound authored VP8 player responds to real
play/pause/load/currentTime operations with correct promises, state, task/event
order, real decoded frames and measured media time.

Acceptance: genuine receivers and owner checks; stable source selection without
Attr rewrites; pause preserves loading/active seek work; official and default-start
positions; finite conversion/overflow handling; requested-frame publication before
seek completion; pending-play promise ownership/settlement exactly once; repeated
controls, same-frame/EOF, rapid replacement, callback reload/navigation and stale
completion cancellation; independent frame pixels. Keep unsupported formats,
element audio, multiple players, network and MSE explicit until separately agreed.

Pause sub-contract: stable genuine length0 pause freezes the measured position
synchronously and applies to the real clock at issue time before advancement or
snapshot replacement. Loading/seek/frame work and raw source Attr identity survive.
One started timeupdate/pause task shares its allowance and listener checkpoints;
source retirement before start cancels it, while same-document load inside the
started task preserves its tail. Ordinary errors continue; fatal jobs stop the
unfinished tail. Admission failure leaves state unchanged. Existing native
controls, standalone playback and source-replacement coverage remain required.
The seek and play sub-contracts below extend this same bound local player.

Seek sub-contract: finite double conversion, signed premetadata defaults and
job-stable official positions feed the existing real decoder. Source-bound
controls apply in FIFO order independently of task dispatch and decode completion.
Requested bitmaps precede completion notifications; playing seeks retain intent
while the clock waits for data. Native Play during a pending seek also waits.
Default-start metadata precedes target data-ready events and document load.
Original Page, queue and actual worker/App tests cover clamping, same-frame/EOF,
supersession, pause interleavings, producer failure and stale source/generation.
Control admission reserves its entry before projection; later asynchronous
notification-allocation failure enters a reported stopped media error. A small
capacity test proves that failure and real reload/seek recovery. No future-event
storage guarantee is implied.

Play sub-contract: a genuine receiver-checked play() returns a real realm-owned
Promise and updates logical paused state synchronously. Supported policy-denial
and interruption paths reject with genuine DOMException reasons. Source-scoped
pending promises settle once; the playing notification precedes fulfillment
reactions. Pause, load/source replacement, Stop and navigation preserve the
ownership and cancellation boundaries. Loading and pending seeks do not advance
the real clock before requested decoded data is ready. Trusted App input supplies
activation; synthetic events do not grant unrestricted playback.

Original Runtime/Page and worker/App tests cover pending outcomes, readiness,
source retirement and failure paths. The optimized-executable authored-video
App/input/offscreen journey passes native controls, script pause/seek and a
trusted script play click: genuine Promise identity, synchronous state, exactly
one playing-before-fulfillment sequence, measured time advancement and pixels
against independent clip timestamps/RGBA. A subsequent script pause confirms
settled frozen time and frame pixels; native seek, EOF/replay, Stop and navigation
remain covered. This finite local video-only evidence does not establish broader
HTMLMediaElement conformance, audio, multiple players, network/MSE playback or
live-service compatibility.

## F4 — network media transaction

Outcome: one original supported clip plays from controlled HTTP(S) origins using
existing source/decoder machinery. Define full-fetch/range behavior before coding.

Acceptance: allowed/denied CORS and credentials, redirects, CSP, TLS, MIME, bounded
encoded/decoded input, cancellation/replacement/errors/retry policy, valid range
responses if used, and actual frames/time. Routine CI uses loopback servers only.
Transport200 alone does not pass playback. No stream-admission/token/DRM bypass.

Transaction strategy: bounded complete GET, with no Range requests or progressive
playback. Validate a complete final 200 response, unambiguous video/x-ivf MIME and
absence of unsolicited partial-content metadata before the original IVF decoder.
Retained complete bytes serve real seeking. Preserve TLS, per-hop CSP/blocking,
credentials and cancellation; no automatic retry beyond the transport's existing
safe stale-connection handling. Explicit load/source selection owns a new attempt.
Network documents retain separate local-file authority. Missing crossorigin keeps
exact-origin policy. Explicit authored anonymous and use-credentials states use
genuine CORS checks, including redirect-origin taint, credential history, required
response headers and normal SameSite/secure/HttpOnly cookie boundaries. Invalid
or empty attribute values use anonymous semantics. Dynamic crossOrigin APIs and
opaque unmarked foreign loading remain outside this finite slice.
Encoded/decompressed media uses the
adjustable media input allowance with a 512 MiB transport ceiling and checked growth;
ordinary document/download/resource caller limits stay unchanged.

Original transport and actual App tests cover fixed/chunked/compressed complete
responses, MIME/status/partial-content refusal, bounded encoded/expanded input,
policy denial before contact, CORS/credential headers and cookie effects, redirect
history, Stop/replacement/navigation, stale completions and explicit recovery.
Actual foreign playback uses genuine Play promises, measured time and independent
frame pixels. Optimized HTTP and authenticated TLS 1.3 journeys repeat the full
local control/frame contract. Separate executable media-source negatives prove
issuer/hostname rejection before media HTTP and player installation, with a real
rendered error event. These establish the finite F4 transaction, not general
streaming, Fetch/CORS, format or live-service support.

## F5 — bounded VP8 dependencies with existing PCM synchronization

Outcome: a finite visible, fixed-canvas VP8 I/P profile reuses the original IVF
path and existing Matroska/PCM16 playback. Retain explicit unsupported tools after
inspecting actual original bitstreams; encoder flags alone do not prove their
absence. No new container, audio codec, runtime or operating-system decoder is
needed for this slice. Preserve the existing still-image/keyframe versions.

Acceptance includes original dependency-bearing frames and independent full
decoded references, checked dimensions/indices/references, genuine entropy and
reference updates, transactional decode failure/reset, malformed/truncated input,
bounded memory and aggregate replay work, cancellation and stale-result isolation.
Prove real pixels, dependency-aware seeks, EOF/replay and the existing acknowledged
PCM sample-clock synchronization. WebM/Opus/Vorbis, hidden video frames, MSE and
general service playback remain separate.

Implemented finite profile: visible fixed-canvas version-zero I/P frames with
intra macroblocks and ZERO/NEAREST/NEAR/NEW single16x16 motion vectors; LAST,
GOLDEN and ALT references; format-defined reference copy/refresh order; real
coefficient/mode/MV probability updates, segment-map retention and loop filtering.
SPLITMV, hidden frames and nonzero-version interframes explicitly decline.
Existing keyframe versions remain supported; this is not complete VP8 conformance.

Retain real post-filter YUV through immutable reference ownership, normalize
visible-edge padding and publish candidate state only after the final cancellation
check. One requested bitmap owns a fresh key-to-target decoder sequence with shared
adjustable work/packet allowances, including work spent on failure. Small profiles
verify refusal and fresh-request recovery; normal defaults grow on demand.

Original authored64x48/8fps/24frame two-GOP inputs have independent native-decoded
YUV, default RGBA and a separate explicit centered-bilinear BT.601 display oracle.
The new display profile requires mean RGB error<0.5 and maximum channel error2;
it does not replace or loosen existing reference checks. Original bit-packet tests
exercise entropy rollback, segment disable/reenable, reference copy/refresh,
malformed/truncated input and final-cancellation atomicity. Real App tests hold
decoded results across newer seeks, source changes, native Stop and navigation.

The executable journey checks actual script/native controls and promises, all 24
independent frames in nonsequential/backward IVF and Matroska seeks, EOF/replay,
source lifecycle and complete loopback HTTP(S) video. Existing Matroska PCM16
selected offsets and an explicit WinMM device journey cover their distinct scopes:
only the latter verifies acknowledged advancement and completed drain. Physical
audibility and service compatibility remain unproved. Routine CI remains offline
and does not open the audio device.

Semantic basis: [RFC6386](https://www.rfc-editor.org/rfc/rfc6386.html), especially
sections9–10 and16–18. Viewing-format conversion is explicit under section5.
Container, audio and network contracts remain separate: no new WebM/audio codec,
additional audio codecs, range streaming, MSE, or public-service acceptance.

## C1 — a CSS image path through the shared renderer

Outcome: the selected single URL background layer reaches ordinary block and
inline-block boxes through real cascade, stylesheet-relative resolution, image
policy, decoding, layout and painting. Supported longhands are image URL/none,
repeat/no-repeat/repeat-x/repeat-y, finite position syntax and size auto/contain/
cover. The existing color-only shorthand resets these noninherited values.
URL/compound shorthands, gradients, multiple layers, inline fragments, replaced
element decoration, row-only decoration and background animation remain outside
this slice. No full CSS or live-page screenshot parity is implied.

Preserve raw URL case and the winning declaration's source owner, including
redirected external stylesheets and reentrant source changes. Reuse exact-origin,
CSP, local-root, image-preference, decode and retained-image policy. Separate
worker slots prevent background cancellation from canceling an element image on
the same owner. Backgrounds have no natural layout dimensions or invented link/
control target. Checked on-demand request storage and a host policy of65,536
attempts per document bound this path; small profiles test denial without contact,
no automatic retry, checked ID exhaustion and fresh-document recovery.

Painting uses the existing alpha/filter primitives and only visible surface
pixels, without a scaled bitmap or tile-list allocation. Padding-box positioning,
border-box clipping, negative offsets, repetition, zoom, flex/table resize,
ancestor clipping, positioned/fixed movement and conservative box occlusion have
original checks. Real App cases cover script-disabled pages, image independence,
policy refusal, source replacement, Stop/navigation and held stale responses.
Startup and stylesheet/image/script callbacks cannot finish window load before
their actual selected image work settles.

The original loopback executable fixture checks case-sensitive stylesheet-relative
requests, exact MIME/body hashes, independent asymmetric control/background pixels,
unchanged measured geometry and paired document/layout/canvas evidence. It uses
App input and offscreen painting; physical-desktop, live-service and complete CSS
conformance remain separate. See tools/fixtures/css-background/provenance.json.
Semantic reference: [CSS Backgrounds and Borders](https://www.w3.org/TR/css-backgrounds-3/).

## F6 — staged authored page and media integration

Status: deferred for shared trust and everyday browsing foundations. Completed
authored media and interface slices retain their finite scope and evidence;
remaining F6/YouTube integration is outside the active development queue.

The service objective remains actual accessible non-DRM playback with readable
controls and real decoded video/audio output. Authored fixtures
and interface prerequisites establish finite progress; they do not establish
YouTube or other public-service acceptance.

Finite Media/Video interface prerequisite: genuine owner-Window
HTMLMediaElement and HTMLVideoElement interface objects and prototypes expose
the existing implemented player. Constructor/prototype inheritance follows
Media to HTMLElement and Video to Media. Direct calls and construction throw
TypeError; they are not element factories. Main-document HTML VIDEO class
identity is independent of its selected source or Snapshot lifetime. Real VIDEO
nodes have an ordinary expando store and actual immediate Video prototype;
unrelated element families retain their existing immediate-prototype policy.
AUDIO retains opaque immediate identity with genuine Media ancestry.

The Media prototype exposes load(), pause(), play(), src/currentTime and
controls accessors, and currentSrc, duration, paused, ended, seeking, readyState
and networkState getters. The Video prototype exposes decoded videoWidth/videoHeight.
The real NETWORK_* and HAVE_* constants are inherited from the Media prototype
and also appear on its interface object. Canonical methods/accessors have
ordinary Web IDL descriptors, genuine brands and owner Function/Promise/
DOMException realms. Ordinary instance/prototype shadows remain observable;
saved canonical operations read or change native slots independently of them.

Canonical src/currentTime setters share the existing native write transaction:
primitive conversion, validate_set, semantic source-selection reservation,
original native incumbent, actual native write, pending-Promise effects and
stable-task completion. A missing setter argument is Undefined and undergoes
that attribute's conversion. Canonical load/pause/play reuse the actual control
FIFO, issue-time position, decoded-frame/acknowledged-PCM readiness and genuine
play Promise registry. Saved canonical holder and actual receiver origins are
checked before coercion; existing Event/DOM borrowing policies are unchanged.

Class identity alone does not add a player. Genuine unbound VIDEO/AUDIO
load/pause calls throw a real NotSupportedError DOMException; play returns a
genuine rejected Promise with that reason. Other playback state getters/setters
still decline the missing binding explicitly; controls reflects the actual
content attribute without requiring a player. These refusals do not install an EMPTY
player or alter another player's source, control queue or device. Child players
and additional controllers remain unsupported. VIDEO width/height content
reflection, canPlayType, MediaError/TimeRanges and the wider media API remain
deferred. Existing authored A/V, transport and physical-control
profiles are unchanged.

Controls reflection prerequisite: genuine main-document HTML VIDEO/AUDIO
controls getters and setters reflect the real null-namespace content attribute,
including on unbound elements. Setter Boolean truthiness invokes no object
primitive conversion; true writes an empty controls attribute and false removes
it. A saved canonical setter bypasses ordinary own shadows through the existing
native validation/write transaction and retains holder/receiver owner checks.
Reflection itself creates no player, load or playback operation.

Existing supported Element set/removeAttribute, namespace and attached Attr
value/nodeValue/textContent, attachment and removal routes use the same checked
controls transaction. Namespaced lookalikes remain inert metadata. Lookup and
Attr identity do not imply NamedNodeMap mutation support; those mutations and
Element.toggleAttribute remain explicitly unsupported. Other protected media
attributes retain their existing restrictions.

For the bound video, native bar paint, hit testing and keyboard handling follow
the content attribute. When App reflow observes it absent, it cancels an armed native
press and releases native control-key focus. Restoring the attribute does not
restore stale input intent. The selected source, raw source Attr, clock, decoded
frame work, pending seek and Play Promise lifetime remain independent of bar
visibility; no reload, implicit pause/play or source retirement is added.

Original Page and App controls cover Boolean and owner rules, ordinary shadows,
unbound reflection, supported Attr routes/refusals, actual bar/input geometry,
capture/focus cancellation and pending-seek preservation. The optimized
App/input/offscreen controls journey captures hidden-playing/advanced, hidden-
paused/probed and shown/restored phases in distinct snapshots. A real pending
Play Promise survives removal and fulfills once after playing. Independent
timestamps/decoded pixels, the exposed bottom strip and exact native bar pixels
verify actual output. Real hit queries and page/focused Space paths verify hidden
and unfocused refusal, restored native Play, pause and frozen output. The same
document/source/Attr persists with one complete loopback clip fetch, paired
document/DOM/layout/canvas observations, current zero interpreter errors and no
audio device. Earlier authored/source/dispatch/A/V journeys remain. Full normal
verification remains a required integration gate; this finite result establishes
no physical OS input, audibility or public-service acceptance.

Semantic references: [Media](https://html.spec.whatwg.org/multipage/media.html#htmlmediaelement),
[Video](https://html.spec.whatwg.org/multipage/media.html#htmlvideoelement) and
[Web IDL attribute setters](https://webidl.spec.whatwg.org/#dfn-attribute-setter).

Seven original Page/Runtime controls cover canonical roots/descriptors/constants,
slot reads versus ordinary shadows, real Promise reasons and pause/load queues,
extracted setters and stable src FIFO, owner/foreign checks before effects,
unbound-class identity/refusal, and bounded getter-work failure/recovery. Queue
acknowledgement in these tests is a Page/App handoff, not playback evidence.
check_media_interface_harness.py joins the required normal offline journeys.
The matching optimized executable passed actual canonical controls despite own
shadows, with the original clock, independent pixels, events, source lifetime,
seek, EOF/replay, Stop and navigation checks and paired document/layout/canvas
identity. Validation scope is the finite model/interface and App/input/offscreen
slice; full normal verification remains a separate required integration gate.
The broader F6 objective remains deferred. This adds no device, audibility,
physical-desktop or public-service acceptance.

Base Event prerequisite: new Event(type, init) uses a real owner-realm
constructor and canonical prototype, branded getters/methods, phase constants
and an own unforgeable isTrusted accessor. Required DOMString conversion and
ordered EventInit bubbles/cancelable/composed flags use the existing runtime
conversion paths. Own shadows and changes to Event.prototype's ordinary parent
remain observable; dispatch uses stored type/flags rather than a shadowed type
getter. Factory-created base
events remain uninitialized until initEvent; initialization preserves composed
and timeStamp while resetting the specified propagation/cancellation state.

composedPath returns independent Arrays from the actual active light-DOM path,
including through the canonical inherited method on genuine native events.
Retained created events have an empty path after dispatch cleanup; native-input
handles retain their existing callback-lifetime restriction. Native click and
keyboard producers set composed;
load/DOMContentLoaded do not. Constructor flags do not infer composition from
the event name. Timestamps use the real owner context, coarsened to whole
milliseconds. Saved constructors/getters/methods and fresh/cached
access preserve owner checks. Failed per-instance setup withdraws only the new
unpublished record; prior/reentrant effects survive and issued IDs are not reused.

Original base Event tests cover descriptors/brands, ordered and reentrant
conversion, shadow/slot separation, cancellation/path cleanup, owner realms and
clocks, and bounded failure/recovery. The dispatch journey adds two trusted
button activations with real construction, capture/target/bubble cancellation,
independent path copies, initialized legacy control and child owner prototypes;
visible results have paired document/layout/canvas pixels and zero current
interpreter errors. That child check uses an owned loopback web origin; child
contexts for local files remain unavailable.

KeyboardEvent construction and its owner-realm UIEvent-derived prototype now
use genuine native records. Key/code preserve UTF-16; readonly getters,
modifiers, location constants and legacy keyCode/charCode values reflect real
constructed state. Trusted keydown/up keep existing input, cancellation and
callback expiry through genuine KeyboardEvent ancestry. Constructor records
remain untrusted and synthetic dispatch does not generate native editing.
One shared UI-family variant avoids retaining simultaneous mouse/keyboard
payloads in each record; key/code payloads and actual record capacity are
admitted before publication and included in unpublished rollback. Deprecated
initKeyboardEvent and native IME/keypress delivery remain unimplemented.
See [KeyboardEvent](https://w3c.github.io/uievents/#interface-keyboardevent).
FocusEvent adds a genuine per-owner UIEvent subclass with constructor, branded
readonly relatedTarget, canonical descriptors and legacy createEvent factory.
The nullable related target must be a real accessible EventTarget. Dictionary
conversion, inherited UI fields, untrusted synthetic dispatch, protected trust,
ordinary prototype replacement/deletion and owner/origin checks use real native
records. Allocation/copy failure and unpublished wrapper rollback preserve the
existing record arena and identities; no simultaneous new payload is charged
to every base Event.

Rendered native controls deliver blur, focusout, focus and focusin after an
actual focus change, with the previous/new native targets, Window view, zero
detail and noncancelable composed flags. Only focusin/out bubble. Repeated focus
on the same control emits no transition. Native pointer caret placement precedes
callbacks, and disconnected targets are revalidated between deliveries. Original
App tests cover tab/pointer focus, retained editor state and callback removal.
Synthetic FocusEvent delivery changes no native focus or text. Existing control
focus() remains queued; full synchronous HTML focus/blur algorithms, arbitrary
non-control focus, nested-context/chrome/OS focus notifications and shadow
retargeting are outside this increment. There is no initFocusEvent method.
See [FocusEvent](https://w3c.github.io/uievents/#interface-focusevent).

Synthetic Mouse dispatch/default activation from synthetic events and general native
pointer payloads remain unsupported. The finite trusted anchor host seam is
described in B2 and used by its guarded native link routing.
Changing native host prototypes, arbitrary native subclassing/newTarget and
shadow-DOM retargeting are also unsupported. These finite interfaces establish
no full Event-family, web-platform or public-service conformance.

UIEvent prerequisite: new UIEvent(type, init) and the main document's legacy
UIEvent/UIEvents factory reuse real UiData and Event state. Each owner has a
canonical UIEvent constructor inheriting Event and a UIEvent.prototype inheriting
Event.prototype. Branded readonly view/detail/which getters and initUIEvent use
the actual slots; own shadows and ordinary prototype overrides remain observable
without changing dispatch state. The factory remains initially uninitialized.

UIEventInit converts the required type first, then gets inherited EventInit flags
in bubbles/cancelable/composed order. It gets detail and completes numeric IDL
long conversion before getting and validating nullable genuine Window view.
Unknown keys, including the optional informative which extension, are ignored.
Each declared dictionary member can opt into immediate primitive conversion and
native validation; other existing native dictionaries retain their prior paths.
Legacy initUIEvent preserves its positional view-before-detail conversion,
composed and timestamp; dispatch-time initialization remains a no-op after the
specified argument conversions. Supplementary Mouse state is retained.

Constructor/view validation uses the explicit owner through reentrant getters,
not a previous native operation's caller. Fresh/cached/saved foreign operations
still refuse before conversion. Checked registry-size work is admitted before
constructor/factory publication. Failed instance setup withdraws only the exact
new record, retaining valid earlier/reentrant effects and consuming issued IDs.
Original tests cover descriptors, order, defaults, numeric wrapping, abrupt
completion, real owner clocks, foreign denial, small profiles and recovery.

Genuine legacy Mouse events have their MouseEvent prototype and UIEvent
ancestry with borrowed getters. Active native keyboard events have UIEvent
ancestry and borrowed getters while their specialized immediate prototypes
remain opaque. Native click producers currently provide base Event state,
without fabricated UI data.
The actual UIEvent App/input journey uses owned HTTP for its child context,
one native key down/up pair and two trusted activations with real UI construction,
dispatch/cancellation, independent path copies and readable changed pixels.
It retains the original dispatch/guard/recovery cases.

Primitive native reads carry the actual Script/native caller independently of a
borrowed builtin's owner realm. The runtime validates the native owner before
performing one getter; existing Host implementations default to their original
getter. A retained genuine blank child can read its own Window aliases,
Document/body and Location primitives after the parent's origin changes. Parent
reads of foreign child fields and child reads of foreign main fields still refuse.
Original Page tests cover that divergence, direct UI construction with the child's
real Window, saved event denials before coercion, borrowed native/getter/callback
reentry, exceptions, policy refusal and later-job recovery. Existing App journeys
separately exercise same-origin child owners; they do not simulate a cross-origin
parent navigation. Child navigation, mutation, has/key enumeration and broader
Window APIs retain their existing separate support limits.

MouseEvent prerequisite: new MouseEvent(type, init) and the main document's
legacy MouseEvent/MouseEvents factory have genuine owner-realm Mouse→UI→Event
constructor/prototype chains. Readonly branded getters, initMouseEvent,
getModifierState and the own unforgeable isTrusted accessor reuse real event
records, owner clocks and atomic unpublished-result setup/rollback. Ordinary
shadows and prototype overrides remain observable without changing stored slots.

The constructor gets/converts inherited EventInit, UIEventInit, EventModifierInit
and MouseEventInit members in the specified inherited/alphabetic order. Detail,
button/buttons and coordinates convert before the next Get. Missing/undefined
members retain defaults. Screen/client doubles preserve fractions, signed zero
and finite values beyond legacy LONG range; nonfinite/Symbol values refuse before
later Gets. Button/buttons use signed/unsigned short normalization. View and
relatedTarget require genuine nullable Window/EventTarget references with explicit
owner-origin checks. No duck object acquires native capability.

Four existing modifier booleans and ten additional stored flags supply all fourteen
exact-case getModifierState names; unknown names return false. The selected
nonnormative supplemental which profile uses normalized button+1 after construction
or legacy initialization, while a fresh uninitialized factory remains0. The optional
which-init dictionary member is ignored. This is a stated finite compatibility
choice, not universal browser behavior for every button/event type.

Legacy initMouseEvent retains positional LONG coordinate conversions, then stores
their values as doubles. It resets its named fields/four modifiers while preserving
constructor-only buttons/extra modifier flags and layer slots. Base/UI initializers
preserve Mouse state. Main page/offset aliases use actual current owner scroll;
immutable initial child contexts retain their actual zero-scroll state. Undispatched
layer slots begin0; target-relative dispatched geometry is not claimed.

Original model tests cover real roots/descriptors/brands, exact conversions,
reentry/throw identity, real owner clocks, fresh/cached/saved foreign denial,
small-profile/store/ID/work refusal and recovery. The fifth owned-HTTP App/input
case uses two trusted base inputs to inspect real Mouse objects, modifiers, child
owners and legacy reinitialization. It requires synthetic Mouse dispatch to refuse
before delivery/activation or target-state mutation. Earlier guard/Custom/base/UI
cases remain. Native click is not relabeled from its type string; native keyboard
retains genuine UI ancestry and rejects Mouse-only operations. PointerLock,
region/sourceCapabilities, Pointer/Keyboard constructors and arbitrary
native subclassing/prototype mutation remain outside this finite slice.

Semantic references: [MouseEvent core](https://www.w3.org/TR/pointerevents4/#mouseevent-interface),
[CSSOM coordinates](https://drafts.csswg.org/cssom-view/#extensions-to-the-mouseevent-interface),
[modifier initialization](https://w3c.github.io/uievents/#event-modifier-initializers)
and [supplemental UI fields](https://w3c.github.io/uievents/#legacy-uievent-supplemental-interface).

Semantic references: [UIEvent](https://w3c.github.io/uievents/#interface-uievent),
[UIEventInit](https://w3c.github.io/uievents/#dictdef-uieventinit) and
[Web IDL dictionary conversion](https://webidl.spec.whatwg.org/#es-dictionary).

Semantic reference: [DOM Event, initialization and dispatch](https://dom.spec.whatwg.org/#interface-event).

CustomEvent constructor/prototype prerequisite: new CustomEvent(type, init)
uses a genuine owner-Window CustomEvent constructor inheriting Event and a
CustomEvent.prototype inheriting Event.prototype. The readonly branded detail
getter, canonical initCustomEvent and own unforgeable isTrusted accessor reuse
the existing record and owner clocks. Ordinary shadows remain observable;
borrowed canonical operations still read or change actual native slots.

The required DOMString type converts first, followed by inherited EventInit
bubbles/cancelable/composed members and detail in dictionary order. Detail is
actual any, defaults to Null and undergoes no string/numeric conversion; object,
function, symbol and native-reference identities remain real across realms.
Retaining a native reference grants no new authority to read that reference.
Checked combined type/direct-detail string bytes and fallible copies precede
record publication; object graphs retain the existing conservative exported-
argument lifetime. Failed per-instance setup withdraws only the exact newest
unpublished record and both retained strings. Prior/reentrant effects survive;
issued identities and work are not rewound.

The main document's singular, case-insensitive createEvent('CustomEvent') uses
the same genuine prototype and detail slots. The factory remains uninitialized
until initCustomEvent; construction initializes even an empty type. The branded
initializer converts positional arguments in order, returns without resetting
an event during dispatch, and otherwise updates that same event atomically.
Base initEvent preserves subclass detail; legacy initialization preserves
composed and timeStamp while resetting the specified cancellation/propagation
state. Genuine child constructors and their canonical owner roots are supported;
child-document factories remain outside this slice.

Existing synchronous dispatch, propagation, cancellation, trust, fatal cleanup
and host quotas remain authoritative. Fresh/cached access and saved canonical
holder/receiver calls retain owner checks before coercion. No activation grant,
native CustomEvent input, shadow-DOM retargeting or collector redesign is added.

Original Page/Runtime controls cover real roots/descriptors, brands and readonly
slots, ordered/reentrant conversion, defaults and any identity, initialized
constructor/factory behavior, dispatch/init cleanup, combined retention and
bounded creation/work/identity refusal, exact unpublished rollback and recovery.
The actual optimized App/input/offscreen journey uses two trusted base-input
clicks to construct and deliver untrusted CustomEvent objects. It verifies any
detail identity and mutation, canonical initialization through own shadows,
capture/target/bubble cancellation, independent path copies, child owner roots
and real child-realm detail. Readable output has paired document/DOM/layout/
canvas pixels and current zero interpreter errors. The original legacy
notification, stack guard/recovery, base/UI/Mouse cases remain. Full normal
verification remains a required integration gate; broader F6/service acceptance
remains deferred. This proof adds no device, audibility or physical-desktop
claim.

Semantic reference: [DOM CustomEvent and legacy initialization](https://dom.spec.whatwg.org/#interface-customevent).

PCM integration prerequisites: a snapshot explicitly distinguishes video wall
time from acknowledged PCM time. Sample-backed positions remain at the actual
observation until another acknowledgement; decoded readiness alone cannot resolve
play promises before a playing acknowledgement. Pause/load/late observations and
duplicate requests retain the existing source-scoped promise rules. These Page
and snapshot tests establish model behavior, not physical output.

The audio owner accepts an optional source-retirement token in addition to clip
and generation identity. It checks retirement before native operations, refill
and publication, retains prepared/queued headers through safe reset/unprepare,
and rejects late sources before replacing a current owner. Paused fenced owners
observe retirement without idle retries. A one-shot App teardown clear may retry
cleanup once; failed native storage remains truthfully owned until safe release.
Standalone unfenced behavior is preserved. Mock-backend ownership and genuine
App snapshot tests have a different scope from coordinated WinMM acceptance.

Finite authored A/V: the bound VIDEO also accepts the existing supported
Matroska VP8/PCM16 profile from local or bounded HTTP(S) sources. Its typed source installs both decoded tracks
and the source-bound PCM owner before default/explicit seek or retained Play.
Input bytes obey both host limits; local-root/CSP and cancellation checks remain.
HTTP(S) retains the complete final200/no-partial transaction and shared TLS,
redirect/CORS/credential/cookie/CSP/blocking/cancellation checks. One explicit
video/x-ivf or video/x-matroska response MIME selects exactly one original
decoder; URL suffixes never guess or rescue mismatched bytes. The original
video-only Context API remains IVF-only. Transfer obeys the media input cap;
Matroska also obeys captured audio byte limits before demux and its original
audio/video profile limits. A small audio cap does not shrink IVF transfers.
Muted Matroska refuses after its MIME is known, before decoding/installing PCM;
policy denials still prevent contact. No unchanged failure automatically retries.
Autoplay, loop, poster, muted Matroska and multiple players remain unsupported.

JavaScript load(), pause(), currentTime and play() share the existing source FIFO
and native Pause capture. PCM time advances only through accepted sample
observations. Play fulfillment waits for an actual playing acknowledgement and
its selected bitmap; seeked follows the real seek acknowledgement and requested
frame. Pause projects the official observed position synchronously, then accepts
the genuine later device Pause position without seeking back to an old sample.
Native Stop retains the live decoded source for replay; load/source replacement
and navigation retire its owned work. Removing src after selection retains the
selected resource until explicit load; removing the loaded VIDEO node remains
blocked by the existing structural guard.

Original Plan/App tests cover actual demuxed tracks, small limits/cancellation,
source replacement and late completions, pending seek/play/pause interleavings,
current-ack readiness, native Pause and actual worker error paths. The routine
local authored-A/V journey selects paused PCM positions and independent decoded
frames, checks EOF selection without claiming drain, switches the same element
to IVF and back, and verifies navigation teardown. It keeps audio output closed.
The controlled HTTP and TLS1.3 journey repeats those actual sample/frame and
source-lifetime checks with exact served MIME/body hashes and no Range/If-Range.
It reuses the existing transport fixture server and process-only public test CA;
operating-system trust stays unchanged. Routine CI opens no audio device.
Coordinated device acceptance remains separate from these routine checks;
physical audibility and F6 service acceptance are not established by them.


Every later feature records semantic references, original and selected conformance
tests, security review, native/device/live checks still pending and exact final
source/executable identity. Release notes describe only proven behavior.

## Same-document session entries

The native session-entry list is the authority for checked pushState and
replaceState, Page state/length, displayed URL synchronization and native
Back/Forward. Startup installs the destination entry before executing its
scripts; accepted nested pushes therefore remain ordered and are not overwritten
by a later startup publication. Branching removes only forward entries. Existing
reload, redirect, POST resubmission, local-root and document-group rules apply.

Native entry preparation admits actual vector capacity, URL/path buffers,
primitive state and saved request payload before replacing the active entry.
The per-App session allowance is64MiB, using the existing browser host byte policy;
other Page stores remain separate. Truncation refunds payload without pretending
that retained vector backing was freed. Prepared changes reject stale revisions;
scripted movement and fallible request copies debit the active job. The original
128-entry,1MiB state and4096 lifetime-update restrictions no longer control this
family. This does not reopen the deferred browser-wide inventory.

Checked data currently covers serializable primitives, including exact UTF-16,
BigInt, signed zero and NaN. Symbol/function data retains DataCloneError; object,
array and native-object serialization remains explicitly unsupported. Native
same-document traversal restores state and scroll without fetching the route or
replacing its live document and controls. PopStateEvent delivery and script
traversal methods remain separate unfinished behavior; no complete History or
website-navigation conformance is claimed.

See the [History API](https://html.spec.whatwg.org/multipage/nav-history-apis.html#the-history-interface).


## Image and script interface identity

The per-owner HTML interface registry includes `HTMLImageElement` and
`HTMLScriptElement`. Parsed, created and supported inert HTML nodes use those
canonical prototypes, with the actual HTMLElement ancestry and class tags.
The legacy `Image` factory is a distinct constructor sharing
`HTMLImageElement.prototype`; that prototype's constructor is
`HTMLImageElement`. Direct interface calls/construction throw TypeError.
Saved canonical constructors remain anchored after writable interface globals
are replaced or deleted. Child realms have distinct interface objects; existing
owner/origin and blank-frame factory boundaries still apply.

Existing native image dimensions, source reflection, resource queues and decoded
pixels, and actual script load/error event targets remain authoritative. This
slice adds interface identity over those implementations, not the full image or
script IDL, image decode promises, module execution or resource activation in
inert documents. Prototype additions do not create new loading permission.


## Live DOM Range boundary increment

The owner realm supplies canonical Range, NodeRange and AbstractRange interface
objects, class tags and branded accessors. Range construction and the displayed
document's createRange start at that document with both offsets zero. Supported
operations set either endpoint, select a node or its contents, set relative
boundaries, collapse, clone, detach (a no-op), read the common ancestor and
stringify selected Text. Offsets use Web IDL unsigned-long conversion and UTF-16
code units; an exact slice can contain an unpaired surrogate. Native node
identity and saved constructors remain authoritative after author shadowing.

Native insertion/removal/moves, replacement, main and promoted inert-fragment
insertion, character-data replacement, textContent, innerHTML and normalization
update boundaries before infallible tree publication. Removed descendants
retarget to the removal point; insertion shifts only offsets strictly after the
insertion point. Normalization relocates merged-text and parent boundaries with
UTF-16 offsets. Failed mutation preparation or storage admission preserves both
the old tree and range state. Shared native work/copy guards cover update scans.

Range records hold native handles, not interpreter values. The existing shared
host retained-byte allowance admits actual record-vector capacity with checked
arithmetic and fallible growth. There is no extra Range count policy: its typed
native handle domain has a checked 24-bit index, and backing/storage admission
normally applies first. Records remain retained until the page is torn down;
detach does not reclaim or invalidate a live range.

This increment supports accessible displayed-document nodes, including detached
main-document nodes and fragments. Child/inert construction/endpoints, Attr
endpoints, StaticRange, comparison/query methods, geometry, contextual fragments,
content deletion/extraction/cloning/insertion and surrounding remain outside the
implemented subset. Range identity is not a DOM selection, editor-selection or
site initialization claim. Original App acceptance uses trusted input, actual
text runs/pixels and stable neighboring editor/link hit targets.


## Canonical Element.id reflection

Element.prototype owns the shared enumerable/configurable id accessor. Its getter
reads authoritative native ID code units, including exact unpaired units in native
records and inert documents. Its setter performs ordered DOMString conversion and
uses the existing attribute transaction. Supported HTML/SVG scalar writes retain
native ID-index updates. Main and inert writes preserve Attr identity and exact
attribute observation.
Brand and saved-realm origin validation precede conversion. Author prototype
replacement/deletion and form named-control precedence retain ordinary lookup.
Reads prepay attribute scans, value work and copies and preserve state on refusal.
Raw main-ID writes, child writes, raw materialization and complete ID consumers
remain separate; this does not close the document foundation.


## Native form interface and named lookup

Real HTML forms use their realm's canonical HTMLFormElement constructor and
prototype, with HTMLElement ancestry, class tag and illegal direct construction.
The canonical prototype.submit operation uses the existing native submission
snapshot and navigation path. Supported named controls retain their existing lookup
precedence. Constructor and submit shadowing resolves real controls, while an
unshadowed submit reads the shared prototype operation. Untouched protected
native member priorities remain, including form text/tag-name reads. Author prototype overrides/deletion do not resurrect a
hidden per-instance native method. Own form expandos remain guarded.

Scalar named-control properties and the supported document form-name path compare
authoritative native code units. Raw ID/name records cannot impersonate literal
replacement-character keys or suppress ordinary prototype lookup. Native reads
use prepaid borrowed comparisons, preserving current ownership and tree order.
Duplicate control/form matches still refuse pending RadioNodeList/live collection
support. Raw property keys, past-names maps, complete document named access and
main raw-ID/name producers remain separate.

Native control initialization compares authoritative ID/form code units and retains
first-match nonform blockers and explicit misses. Existing ID-write and tree-move
refusals use a shared prepaid borrowed comparison. Raw projection aliases cannot
block unrelated changes or bypass a refusal when actual ID identity changes.
Raw main-ID/form producers, general owner resetting and parser form-owner magic
remain separate; submission authority and navigation guards are unchanged.

Borrowed submit validates the real HTML form brand. Inert forms have genuine
interface identity but no active native submission backend and explicitly refuse
submission. Existing local-file, exact-origin, CSP form-action, request and
queueing protections remain. Submitting still bypasses constraint validation,
submit events and submitter inclusion through the existing backend. This slice
does not add requestSubmit, reset/validation methods, full reflected form IDL,
indexed form collections or RadioNodeList behavior.


## Native textarea interface and canonical value

HTMLTextAreaElement is the genuine owner-realm interface for real textarea nodes,
with HTMLElement ancestry, class tag and illegal direct construction. Its shared
value accessor uses the admitted native editor for parsed, fragment-created,
cloned and detached controls. Setting value performs one DOMString conversion,
treats null as empty and preserves native newline normalization, dirty/default
state and the existing script-versus-user editing rules. Getter/setter names,
lengths and enumerable/configurable descriptors are canonical. Own/prototype
author shadows do not alter what a saved native accessor reads or writes.

Wrong brands refuse before coercion. Saved foreign access remains origin-checked;
inert textarea identity does not create native editor state, and value access
there explicitly returns NotSupportedError. Native quota/copy and unsupported
unpaired-text refusal preserve the editor and recover through a later valid set.
Existing default/interior-write, owner, form and resource guards remain. The
remaining textarea IDL is not claimed by this value/identity increment.

## Original script request and XMLHttpRequest increment

The shared asynchronous request core owns document epochs, immutable request
snapshots and cancellation generations. It reuses the original HTTP/TLS,
framing/decoding, cookies, blocking and tracing paths. Script method/header/body
representation is distinct from native form payloads. GET, HEAD, POST, PUT,
PATCH, DELETE and OPTIONS use real wire methods and bodies. HEAD retains the
actual reason phrase and headers without reading/decompressing a response body.
Form submission limits and navigation/replay guards remain independent.

CSP connect-src falls back to default-src, applies all relevant policies and is
checked on every redirect before contact. Safe CORS requests use real origin,
credential, redirect-taint and response checks; Set-Cookie is never script-readable.
Preflight-required requests and conservative privileged-header redirects are
refused rather than sent under a no-CORS shortcut. Worker/pending/completion
backing, policy/header/body snapshots and held responses have ownership leases;
refusal and cancellation preserve admitted work and retire stale generations.

XMLHttpRequest and its upload target have canonical owner-realm interfaces over
actual records. The main document adapter supplies ordered coercions, real
readonly state, ordinary prototype/expando behavior and protected Event/ProgressEvent
notifications. Opened-state changes and abort notify synchronously through the
existing reentrant dispatcher; network results return to the App thread. Requests
snapshot text/primitive or actual buffer/view bytes. Text decoding reuses the
original decoder and MIME override/UTF-8 request charset rules. Non-2xx HTTP
responses retain their actual status/body. Native App tests include rendered
response text/pixels, stable editor/link targets, POST/cookies, CSP no-contact,
reopening during headers, timeout, HEAD and document navigation cancellation.

This is a buffered adapter: header/loading/completion notifications currently
follow receipt of a complete bounded response, with actual received-body counts.
There is no incremental reader or stream facade. Synchronous XHR, explicit auth,
child-window dispatch, preflight, Blob/FormData/Document serialization, non-text
response types and XML response documents remain unsupported. In-flight timeout
changes are refused. Native [Headers values](#native-headers-values-for-shared-request-and-response-adapters)
and [buffered Response/Body values](#buffered-response-values-and-native-body-consumption)
have separate implemented contracts below. Network Fetch, Request and readable
stream bodies remain open; these value types do not broaden XHR transport authority.
The optional nonstandard XMLHttpRequest.prototype.fetch is absent.
Implementation checks do not establish a usable site or successful application
API requests. See the [XHR standard](https://xhr.spec.whatwg.org/) and
[Fetch standard](https://fetch.spec.whatwg.org/).


## touch-action declarations and computed values

The shared native parser supports auto, none, manipulation, axis/direction
keywords and pinch-zoom combinations, with one horizontal group, one vertical
group and canonical order. Invalid specified declarations are ignored. Inline
camel-case and CSS-name reads/writes, set/removeProperty and indexed enumeration
use the actual style attribute, including priority and legacy-null removal.

The renderer's computed snapshot resolves the noninherited value from real
ordered author declarations, custom variables, explicit inheritance and the
existing unlayered revert/revert-layer policy. Incomplete computation remains
unavailable rather than inventing auto. Native work, copy admission, invalidation
and owner guards remain active. Actual App acceptance covers a trusted mouse
mutation with live readback and preserved editor state, pixels and link targets.

This increment supplies CSS semantics. Touch/pen gesture acquisition, gesture
intersection through ancestors and pointer-stream suppression remain open;
mouse selection, control activation and link activation are unaffected. See
[Pointer Events](https://www.w3.org/TR/pointerevents4/#the-touch-action-css-property)
and the [compatibility value grammar](https://compat.spec.whatwg.org/#touch-action).


## HTMLAnchorElement native identity

Each Window realm installs its own canonical HTMLAnchorElement constructor and
HTMLElement-derived prototype over actual HTML anchor nodes. Parsed, created,
cloned, inert and imported anchors keep the correct owner and real node identity;
SVG anchors and unrelated elements do not acquire the HTML brand. Global deletion
or replacement does not retarget saved canonical constructors or native ancestry.
Ordinary prototype and own properties use the existing native object protocol.
Direct interface construction remains illegal; no dummy node is fabricated.

Existing hyperlink URL mutation/reflection and activation guards remain intact.
An original actual-App check uses a trusted anchor callback, saved interface
identity and a real changed target through the native guarded activation path.
This is an identity increment, not completion of every anchor IDL member or
unrestricted navigation. See the [HTML anchor interface](https://html.spec.whatwg.org/multipage/text-level-semantics.html#htmlanchorelement).


## Renderer-backed computed font size

Live computed declarations expose fontSize and font-size reads from the same
native cascade used for painting. Supported relative units, variable substitution,
explicit inheritance and the finite font shorthand feed a paid sparse snapshot.
Pixel serialization reuses the original stack number formatter and charges the
final string before allocating it; page zoom does not scale the CSS-pixel value.

Failed variable computation remains distinguishable from a missing/invalid
variable. Unknown parent/root-dependent sizes remain unavailable until a genuine
known declaration restores them. Incomplete snapshots and copy refusal do not
publish fabricated values or mutate native style. An original App check compares
real changed glyph pixels against a literal font twin, with stable neighboring
editor state and link targets.

This readback preserves the existing finite font grammar and 1–256 px native
renderer policy; it does not complete CSS typography, every computed property,
font-resource support or the deferred capacity work. See
[CSSOM resolved values](https://drafts.csswg.org/cssom/#resolved-values).


## Genuine AnimationEvent construction

Each owner realm supplies a canonical AnimationEvent derived from Event.
Constructor dictionaries use ordered Web IDL DOMString and finite-double
conversion. Native records retain readonly animationName, elapsedTime and
pseudoElement, including unpaired UTF-16 units and signed zero. Saved roots,
borrowed getter brands and owner/origin guards follow the existing event model.

The additional string payload is admitted before base record/backing publication
and refunded if base construction fails. Getter copies remain metered. Synthetic
dispatch uses the actual event target, propagation/cancellation and protected
untrusted state. Base initEvent preserves subclass fields. The original App
fixture dispatches from a trusted button callback and checks real DOM/glyph
changes with unchanged neighboring editor state and link targets.

This constructor/record increment does not implement CSS animation timelines,
keyframes or native animation lifecycle delivery. Legacy createEvent and
initAnimationEvent are not exposed. See the
[CSS AnimationEvent interface](https://drafts.csswg.org/css-animations/#interface-animationevent).


## Native iframe sandbox attribute list

A canonical HTMLIFrameElement prototype getter returns one stable live token
list per real main-document iframe; its setter forwards to that list's value.
Reading the property does not create the attribute or a child context. Raw
setAttribute/removeAttribute, value assignment and supported token operations
share the same native attribute and storage transaction. Ordered unique tokens,
raw string conversion, indices, item/contains and add/remove follow the existing
finite class-token backend. Borrowed methods use the receiver's attribute kind.
Class and sandbox lists have disjoint typed identities with real owner checks.

Wrong receivers fail before string conversion; invalid tokens and quota/copy
refusal preserve the attribute. Raw sandbox attribute writes use the same frame
guard as list writes. Unsupported sandboxed-frame insertion refuses before
publishing topology or a context, including allow-scripts/allow-same-origin
combinations. Connected frame-policy mutations remain refused with existing
window/document identities unchanged. Original App checks cover actual callback
metadata/DOM/glyph changes and stable neighboring editor/link targets.

This is attribute-list support, not implementation of sandboxed browsing
contexts or permission-relaxation flags. Inert lists and toggle/replace/iteration remain explicitly unsupported. No supported-token success or child
context is fabricated. See the [HTML sandbox IDL](https://html.spec.whatwg.org/multipage/iframe-embed-object.html#dom-iframe-sandbox).


## Attribute-specific supported-token queries

DOMTokenList supports converts the token to DOMString and queries the receiver's
actual attribute support set without mutating metadata or creating a context.
The HTML sandbox set comprises allowed tokens actually supported by the user
agent. Native sandboxed contexts and permission-relaxation flags remain
unsupported, so that set is currently empty: stored tokens are not evidence of
native support. Class attributes define no supported-token set and throw a
TypeError, including when a method is borrowed from a sandbox list. Wrong
receivers fail before conversion, while conversion exceptions retain identity.

The native frame admission policy remains unchanged. An actual App check probes
this truthful unsupported set alongside live stored metadata; owner, connected
mutation, quota and context-publication guards remain active. See the
[HTML supported-token definition](https://html.spec.whatwg.org/multipage/iframe-embed-object.html#dom-iframe-sandbox)
and [DOM token validation](https://dom.spec.whatwg.org/#concept-domtokenlist-validation).


## Genuine WheelEvent construction

Canonical owner-realm WheelEvent derives from MouseEvent and retains finite
signed deltaX/deltaY/deltaZ doubles, modulo-32-bit deltaMode and momentum.
DOM_DELTA_PIXEL/LINE/PAGE constants live on the constructor and prototype.
Ordered Web IDL conversion includes inherited UI/mouse members; branded readonly
getters, saved roots and owner/origin guards use the existing native event model.
Inherited initMouseEvent changes parent fields without overwriting wheel data.

Only wheel records allocate their additional backing; actual vector capacity is
prepaid before base publication and refunded on construction failure. Ordinary
Event arena size remains unchanged. Original checks cover conversion ordering,
negative zero, refusal/recovery, child ownership and actual App button callback
construction with real glyph/editor/link checks.

This increment does not deliver native wheel events or implement synthetic mouse
activation and target-relative dispatch geometry. The existing mouse-family
dispatch refusal remains active; construction does not scroll or grant trust.
Legacy createEvent(WheelEvent) and initWheelEvent are not exposed. See the
[WheelEvent interface](https://www.w3.org/TR/pointerevents4/#interface-wheelevent).


## Nested checked template fragments

Template innerHTML accepts nested HTML templates within the existing checked
fragment subset. Each nested element has an empty ordinary child list and a
separate stable inert content fragment in the same appropriate template document.
Template boundaries stop implied paragraph/list closing and ancestor end-tag
lookup; formatting checks use the current content scope. Existing unsupported
table/select/foreign recovery remains explicit rather than silently reshaping it.
Structural refusal diagnostics identify the tag and context without copying
attributes or fragment text.

Replacement stages all element/source identities, content fragments, parent and
child links, and the paid content-index growth before publication. Retained,
copy-work and identity admission remain active; failed replacement preserves old
content and saved identities. Clone and serialization follow real content links,
and repeated innerHTML replacements keep detached old nested contents usable.
Scripts/styles/images in these fragments remain inert. The original App fixture
clones a nested template and imports its ordinary content into the actual document
from a trusted button callback, checking painted glyphs, editor state and links.
This does not add arbitrary template insertion modes, full table/foreign recovery,
resource activation or complete main-document template adoption. See the
[HTML template element](https://html.spec.whatwg.org/multipage/scripting.html#the-template-element).


## Genuine ErrorEvent construction and synthetic delivery

Each implemented Window realm supplies a canonical ErrorEvent derived from Event.
Ordered Web IDL conversion retains a DOMString message, USVString filename
(replacing unpaired surrogates), modulo-32-bit line/column numbers and the exact
uncoerced error value. Branded readonly getters, saved constructor roots and
owner/origin access use the existing event model. Base initEvent preserves these
fields. Dictionary-export roots retain object/function error values.

Only error instances allocate the additional payload. Actual backing and owned
strings are prepaid before base publication, refunded on construction failure,
and included in unpublished-wrapper rollback. Getter copies remain metered.
Synthetic addEventListener delivery uses real target/phase/bubbling/cancellation
and protected untrusted state. The original App fixture dispatches from a native
button callback and checks actual exception identity, painted message glyphs,
neighboring editor state and link targets.

This increment does not install automatic global exception reporting, worker
error propagation or legacy Window.onerror handling. Legacy initErrorEvent and
createEvent(ErrorEvent) are not exposed. Constructor success does not establish
site bootstrap or resolve an underlying exception reported by page code. See the
[HTML ErrorEvent interface](https://html.spec.whatwg.org/multipage/webappapis.html#the-errorevent-interface).


## Inert selectors and genuine query snapshots

Inert Elements implement querySelector/querySelectorAll/matches/closest using the
same native CSS matcher as main-document nodes. Inert Documents and fragments
have their own canonical query methods, with defining-interface brands checked
before selector conversion. Conversion may mutate the tree; matching reads the
actual topology afterward. Existing child-origin and unsupported-child-query
guards remain active. Unsupported selector syntax remains a catchable SyntaxError.

A paid temporary read-only view carries actual attributes, namespace provenance,
parents and sibling slots into the shared matcher without a markup roundtrip.
Traversal preserves depth-first document order and ancestor/sibling semantics.
Template hosts have no ordinary descendants; their separate content fragments
must be queried explicitly. Only a genuine Document grants :root semantics.
Unknown foreign provenance and grammar outside the existing subset remain
explicitly unavailable. This adds no alternate selector engine or CSS grammar.

querySelectorAll publishes a static NodeList of real native handles after scratch,
copy and retained-result admission. Membership stays fixed while the referenced
nodes retain current parent/attribute state through moves, clones and imports.
Refusal publishes no partial list or tree change. The original App fixture removes
declarative metadata, queries nested content, moves it through an inert fragment,
clones/deep-imports ordinary content and checks painted glyphs with stable editor
and link targets. Later scoped mixed-resource/main-template copy and dynamic
SVG increments extend this path; remaining lifecycle/scheduling routes and
broader same-identity cross-document adoption remain separate P1 contracts.
See [DOM queries](https://dom.spec.whatwg.org/#dom-parentnode-queryselectorall)
and [Web IDL operation functions](https://webidl.spec.whatwg.org/#es-operations).


## Passive HTML containers through clone/import

The shared ordinary-container classifier admits the same supported passive
hyphenated HTML names already used by template preparation and promotion.
cloneNode and importNode preserve their real tag, ordinary attributes, descendants
and native HTMLElement ancestry. Deep copies have independent identities; import
assigns the real main-document owner while the inert source retains its owner.
Queries of imported main-document fragments use the checked fragment root and
existing iframe-topology guard rather than an Element-only root path.

This does not register custom elements, run upgrade reactions or expose Shadow
DOM. Native handler attributes, customized built-ins, unsupported namespaces and
protected native/resource children retain their admission guards. A later refused
member publishes no prefix of the staged import. Retained/copy refusal leaves
source and destination unchanged. Original App checks clone/import and connect
nested passive containers, then verify real glyphs, editor state and link targets.
Mixed native-control/resource and nested-template adoption remain unfinished P1
work; this is not completion of the full stamping package.


## Default native control import and activation

Deep import from inert template/fragment data admits the existing supported text
INPUT, checkbox, plain TEXTAREA, conventional SELECT/OPTION/OPTGROUP and BUTTON
families. Native defaults, editor state, checked state and option identities are
constructed by the shared form planner. Imported controls are detached genuine
main-document nodes; value/checked/selection edits survive connection, and form
owners use the projected actual tree. Native reset uses the preserved defaults.

The import moves owned staged nodes into a checked local view without a markup
roundtrip or duplicate text copies. Temporary backing is admitted and refunded;
all buffers are restored on success/refusal. The returned native state and DOM
records publish together after retained/backing checks. Button defaults share a
source-to-destination mapping with existing created-button admission, preserving
control/value/caption limits and avoiding local/global identity aliasing.

Native handler/customized-built-in/namespace guards remain intact. Explicit form
reassociation, radio/hidden/other INPUT import, standalone option/group import and
shallow inert TEXTAREA import requiring raw-value clone steps explicitly refuse.
Existing main-control clone rules and the JavaScript form.reset/options-collection
surface are unchanged. Unsupported members and quota/copy refusal leave no staged
DOM prefix or native control published. Original App checks operate actual imported
controls through pointer/text input, commit/reset, real pixels and link activation.
Mixed resources, nested-template adoption, namespace SVG and remaining lifecycle
routes stay separate unfinished P1 work.


## Plain image clone/import and real activation

Checked inert import and main-document clone admit plain childless HTML img
nodes with independent real identities, attributes and destination ownership.
Cloning retains DOM data without copying decoded pixels, currentSrc, pending
request generations, listeners or ordinary wrapper state from the source.
Eligible requests begin only through the existing guarded connection planner.
Native completion metadata/events come from actual loader/decoder results.

Responsive srcset/picture selection, active handler/customized-built-in transfer,
unsupported namespaces and malformed resource shapes retain explicit guards.
A late import refusal publishes no prefix. Connection preflight refusal preserves
fragment/native topology and queues no earlier candidate request. Existing image
attempt/resource, CSP/origin, cancellation and epoch checks are unchanged.
Detached implicit requests remain outside the implemented image policy.

The original App fixture imports images with a native input and link, confirms
no detached requests, fetches/decodes original loopback images, checks load-event
metadata and exact painted colors, clones a loaded image into a new request and
verifies editor state plus actual pointer link navigation. This is focused App
evidence, not a fresh site result or completion of responsive/resource support.


## Inline style clone/import and atomic mixed stamping

Plain-text HTML STYLE clone/import preserves actual attributes, source text and
destination ownership without copying an active sheet. Passive is metadata uses
the same admission as existing template style promotion; no custom registry or
upgrade behavior is fabricated. Real STYLE hosts are queryable, while their raw
text remains excluded from ordinary visible layout/traversal and protected
mutation routes still apply.

Created/imported style moves, fragment splices and replacements prepare one
batch over the existing inline source/CSP planner. Native preflight temporarily
journals only the changed sources' old contributions, projects every new accepted
source against cumulative byte/rule limits, and restores the source map/counters
on both success and refusal. Source-ownership and withheld-flag backing are
reserved and charged before publication. The tree and accepted/withheld sources
commit together without author callbacks. A later refusal publishes no earlier
sheet or tree prefix. Multiple styles in an inert promotion use the same batch.

CSP, duplicate-nonce evidence, MIME and existing finite media/text-shape guards
remain active; withheld styles remain real DOM without applying a fake cascade.
The original App fixture stamps styles with a native input and decoded image,
checks glyph/background colors, moves a style out and back, preserves editor
state and activates an actual link. This is package fixture evidence. It does
not establish the real site's shell, complete CSSOM/constructible sheets,
arbitrary loaded-resource adoption or completion of the remaining P1 package.


## Main-document template hosts and appropriate content owners

Importing a template, or cloning a main-document template, creates a real detached
main-document host rather than an inert surrogate with a main owner label. Its
content is a separate, independently cloned inert fragment in the destination's
appropriate template document. Nested templates and descendants use that same
appropriate owner. A shallow copy has empty content without materializing the
source subtree. Source templates and saved content/owner identities remain real
and independent through later destination/source edits.

Content/index preparation uses the existing inert clone and paid identity maps.
A checkpoint rolls back new entries, generated data and content/document links
on refusal, refunds payload and preserves actual reusable backing. New main-host
links publish before returning the native operation, without author callbacks
between preparation and main DOM publication. Bulk index insertion avoids
repeated per-key shifts. Created/imported hosts with empty ordinary child lists
can move through checked tree operations or retire during text replacement;
saved content stays inert and available after the host detaches.

The original App fixture connects an imported host without activating content,
then explicitly imports/stamps that content, checks nested appropriate ownership,
real glyphs and stable neighboring editor/link targets. Arbitrary cross-document
same-identity adoption and ordinary-child mutation of opaque/loaded template
hosts remain outside this increment. This is a connected P1 fixture result,
not a real-site shell result or completion of namespace/lifecycle work.


## Passive dynamic SVG and connected graphics

The main document and genuine inert HTML documents implement null-prefix
createElementNS for the supported passive SVG containers and shapes. Namespace,
case-sensitive names, expanded-name Attr records and native SVGElement ancestry
survive independent deep clone/import. Real class tokens, attribute removal,
insert/reorder/replace and fragment splices reuse the guarded DOM transactions.
Unsupported foreign provenance, resource elements, handlers and external SVG
references are refused before publication. No SVG-specific constructor is faked.

Connected SVG roots use current DOM snapshots with the original standard-library
SVG decoder, existing image input/pixel/decoded-storage allowances and checked
traversal. Source changes replace the raster; detachment discards it. Templates
stay inert. The actual App fixture stamps a cloned local-reference vector,
changes its fill through a trusted event, removes/restores it, and verifies exact
pixels alongside a stable native editor and real anchor hit/navigation. This
finite paint path does not implement external stylesheet SVG cascade/inherited
currentColor, general foreignObject, external SVG resources or a usable site.
See [SVG namespace](https://svgwg.org/svg2-draft/struct.html#Namespace),
[DOM element creation](https://dom.spec.whatwg.org/#dom-document-createelementns)
and [DOM node cloning](https://dom.spec.whatwg.org/#concept-node-clone).


## Inert tree mutation records and CharacterData

MutationObserver can register genuine inert documents, template content and their
nodes. Checked append/insert/remove and ordinary attribute-method writes prepare
ordered records against actual inert parent/child/name metadata, with source and
destination sibling snapshots and optional old values. Template contents remain
separate trees; they do not become host descendants for subtree observation.
Saved registrations keep native identities through supported promotion.

Inert Text, Comment and ProcessingInstruction data writes replace owned text or
publish a paid immutable parsed source. Clone data remains independent. Native
retained/copy refusal leaves data and generation unchanged and rolls back pending
records. Notifications use existing observer and Promise/microtask scheduling.
The connected original App fixture uses a trusted event to change inert data;
its observer callback imports mixed nested-template/style/native-control/SVG/link
content and checks exact vector/text pixels, stable editing and link activation.
Selected whole-content replacement now has the preflight described below.
Reflected property notifications and transient subtree registrations after
removal remain separate unfinished observation routes.


## Whole-content replacement notifications

Main element textContent/innerHTML and separate template innerHTML replacement
admit one child-list record with actual direct old/new node identities before
publishing the tree. Main DocumentFragment and inert HTML element/fragment
textContent replacements create genuine owned Text nodes and detach saved old
children without changing their data. Empty-to-empty replacement queues nothing.
Template content stays outside its host's descendant tree.

When observers match a markup write, preflight reuses the original checked parser
and shared template handle projection, paying its additional native work. The
setter then follows the same identity calculation without an intervening author
callback. Native observation storage is reserved before commit; unsupported
resources or native allocation refusal leave the tree and records unchanged.
Node text conversion precedes preflight, including reentrant DOM changes.

Focused fixtures cover real snapshots, parked/inert identity fallback and refusal
recovery. An original App fixture replaces an observed template from a trusted
callback, stamps mixed nested/style/native-control/SVG/link content from the
queued observer, and checks actual pixels, editing and anchor activation. Existing
unknown foreign, SVG text-shape and native-resource retirement guards remain.
This does not establish live-site initialization or finish property/transient
observation routes.


## Namespaced inert writes and attribute notifications

The canonical Element namespace methods reuse genuine main/inert attribute
storage and ordered conversions. setAttributeNS admits a new expanded name;
attached Attr alias writes still require the existing entry. URI/prefix/local
metadata and the first entry's ordered identity survive reads, updates and
removal. SVG writes retain passive/local-reference checks. Invalid namespace
qualification produces a native DOMException before any publication.

Supported Attr value writes and native Attr adoption/removal, class-token
add/remove/value and selected id/className/title reflections preflight a record
for the actual Element rather than the alias/token wrapper. Old values and
namespace filters follow actual attributes after argument conversion, including
reentrant detachment. Native refusal discards pending records. No synchronous
observer callback is introduced.

The original App fixture creates SVG in an inert document with genuine namespace
writes, imports it, and changes actual rendered pixels through an Attr alias and
queued observer callback. Native editing and anchor activation stay intact.
Other reflected-property routes and transient subtree registrations remain
separate unfinished work; this is not a live-site initialization claim.


## Inline style notifications and genuine inert slot data

Supported inline CSS property writes and setProperty/removeProperty prepare
attribute records for their actual Element's style attribute. Implemented HTML
string reflections use the same record path. Equal supported declaration values
and priorities do not rewrite the attribute or cause observer callback loops;
absent removals and invalid ignored values queue no records. Retained refusal
preserves both declarations and pending notifications.

Existing HTMLSlotElement.name accessors now read/write genuine inert attribute
data with defining-interface branding and ordinary DOMString conversion. Inert
Element/Text assignedSlot getters follow their real ordinary tree and preserve
foreign-parent refusal. Supported slot clone/import copies the passive element
and fallback children with correct owners. No shadow root or slot distribution
is fabricated; matching names in an ordinary tree still yield null assignment.

The actual App fixture delivers an inert slot-name update, imports mixed native
content, queues a real style callback after Promise work, and verifies text/vector
pixels, preserved editing and real anchor navigation. Scheduled-peer and same-
value callback cases exercise the native microtask queue. This does not establish
live-site initialization or complete transient subtree observation.


## Transient subtree mutation observation

Checked native removals, moves and whole-content replacement retain transient
observer sources for the actual detached roots. Descendant attribute/text/child
changes continue through those sources until notification. Multiple paths share
the original registration options and deduplicate records by observer. Updating
observe options clears only transients from that registration; disconnect clears
all sources for that observer. Delivery removes sources before callback code,
even when child-list records were not requested or takeRecords drained the queue.

Native transient metadata uses paid, geometrically growing backing and sorted
lookup. Temporary/replacement backing and notification admission precede DOM
commit; failed mutation drops unpublished sources and records. Cleared backing
remains honestly retained for reuse, without a new arbitrary transient count cap.
The focused fixtures cover propagation, deduplication, moves, replacement,
option changes and native backing/refusal recovery.

An original App fixture mutates a genuinely detached template subtree, then
imports mixed native content in its queued observer callback. It verifies cleanup
before callback mutation, real vector/text paint, stable editing and anchor
navigation. Other reflected-property and broader DOM observation routes remain
explicit subsets; this does not establish live-site initialization.
See [DOM transient registered observers](https://dom.spec.whatwg.org/#concept-node-add-transient-registered-observers).


## Native text-control placeholder reflection

HTMLInputElement and HTMLTextAreaElement define canonical placeholder accessors.
Supported genuine main and inert elements reflect their actual null-namespace
attribute, with DOMString conversion, defining-interface brands and real Attr /
MutationObserver behavior. Hint writes do not change current values, defaults,
child text, selection or neighboring native editors. Removal restores the empty
hint. Existing fresh/inert editor-admission boundaries remain explicit.

The original App fixture sets hints through a trusted callback, checks distinct
textarea hint-line pixels, types actual text, verifies the hint no longer paints
over that value, preserves a neighboring editor and activates a real anchor.
Textarea CR/LF/CRLF sequences render as hint line breaks; input hints render on
one line while raw attribute data stays intact. Retained refusal and Symbol
conversion leave editor/data/notification state unchanged, with recovery.

This shared increment addresses a connected startup metadata gap; it does not
establish real-site initialization, complete native textarea creation or claim
complete editing/form APIs.
See [textarea placeholder](https://html.spec.whatwg.org/multipage/form-elements.html#attr-textarea-placeholder)
and [input placeholder](https://html.spec.whatwg.org/multipage/input.html#attr-input-placeholder).


## Shared variable sizing and existing layout grammars

The guarded custom-property resolver now supplies supported width/min/max sizing,
display, box-sizing, flex factors/basis/flow and gap consumers. Literal and
substituted sizes use the same strict finite token grammar. A winning var()
declaration whose result is missing, cyclic or invalid computes to the initial
component instead of reviving an older declaration. CSS-wide values reset or
inherit the selected fields, including parent metrics for inherited em/rem sizes.

Original CSS fixtures cover supported values, cascade/fallback/cycles, token
boundaries and relative inheritance. The actual App fixture changes width and
gap custom properties through a trusted callback, checks changed native geometry
and exact pixels, preserves editing and activates the link at its new hit target.
Existing substitution/storage/work refusal guards are retained. This does not
add general typed math, viewport lengths, inline-flex or complete CSSOM/layout,
and does not establish real-site shell improvement.


## Actual computed display and foreground

Supported getComputedStyle display/color attribute and getPropertyValue reads
come from immutable scalar data captured by the actual renderer's cascade.
Declarations retain only their genuine target/owner and read the current cache,
so class, inline, variable and connection changes remain live. Opaque foreground
colors serialize as native rgb values; supported list/table-group display
keywords retain their identity rather than the layout's grouped classification.

Presentation backing, cascade work and complete output bytes are paid before
publication. Specialized transform-only computation keeps its no-property cost;
a later presentation read obtains a funded complete snapshot, preserving old
cache state and counters on failure. No second cascade or inline approximation
is introduced. An original App fixture checks that live reads agree with actual
text paint, changed geometry, stable editing and native anchor hit/navigation.
Other CSSOM/display/color families and full website usability remain unfinished.


## Shared typed sizing calculations

Supported sizing, flex basis, gaps, margin and padding use one original iterative
calc parser. Length-percentage sums/subtraction, number products/division and
parentheses preserve dimensional type. Mixed px/percentage/em/rem coefficients
resolve against real reference/font metrics; canceled percentage terms retain
indefinite-reference behavior. Shorthands split balanced components rather than
whitespace inside functions. Substitution cannot join separate number/unit tokens.

Invalid type combinations, zero divisors, nonfinite values and malformed syntax
stay refused. Existing declaration bytes and result ranges remain bounded; actual
substitution calculation work and temporary typed-token backing are charged.
An original App fixture changes calculated width/gap values and checks real
geometry, exact pixels, stable native editing and the moved link hit/navigation.
Viewport units, broader algebra/functions and other property consumers remain
unfinished. This is not evidence of live-site shell initialization.

## Common HTMLElement reflections and author overrides

The canonical HTMLElement prototype owns configurable, enumerable hidden, title,
lang and dir accessor pairs. Real HTML elements read and write their actual
main-document or inert-owner attributes. Ordinary own data/accessor descriptors
on supported HTML instances shadow lookup without replacing the native slots;
saved native getters and setters bypass those author shadows. Native attribute
writes retain observation, Attr identity and existing resource/owner guards.
Inert HTML elements use the same paid, owner-validated ordinary backing store.
Form named lookup and unrelated native descriptor restrictions remain distinct.

Hidden accepts its nullable boolean/number/string union. Original null/undefined,
false, zero, NaN and empty strings remove the attribute. ASCII-insensitive
until-found strings store and read the canonical keyword; other truthy values
store an empty attribute. Other objects use one String-hint conversion; Symbol
conversion fails without publication. Title and lang reflect DOMString; dir
reads canonical ltr/rtl/auto or the empty string while retaining authored raw text.
Title and lang preserve exact UTF-16 through native DOMString records, including lone
surrogates, across direct/Attr/null-namespace writes, observation and cloning. Each
getter pays for logical output, and each setter reuses the admitted attribute
transaction; copy/work/string/retained refusal preserves the record and fresh jobs
recover. Dir retains exact authored DOMString text in the attribute while its
getter returns only canonical known keywords or the empty string. Language
selection, bidi rendering and tooltip UI remain separate.
Brands and native origin access are checked before coercion. Supported child
HTML attributes are readable; child-document writes remain refused.

Hidden-until-found reveal, find-in-page and beforematch are not implemented;
existing hidden layout remains the fallback. SVG is not an HTMLElement receiver.
The adjacent contentEditable brand check now enforces that same HTML namespace
boundary instead of accepting SVG through the broader class-list predicate.
An original App fixture demonstrates reactive show/hide, native link pixels and
activation with preserved text-editor state; it is not a site initialization claim.

## Native Headers values for shared request and response adapters

Headers uses genuine private runtime data with canonical owner-realm prototypes,
required method arguments and receiver checks before observable conversion.
Record and iterable initializers complete Web IDL ByteString conversion before
header validation. Nested iterator conversion unwinds through the existing
iterator-close protocol. Invalid names, embedded NUL/newline values, Symbol
conversion and code points above 255 reject before publishing list changes.
Normalization removes HTTP whitespace at value boundaries and lowercases names.

Append/get combine ordinary duplicate values with comma-space separators;
set/delete replace/remove all matching entries. Set-Cookie values remain separate
for getSetCookie and sorted iteration. Entries/keys/values and forEach consult the
live sorted list. The iterator prototype inherits the shared Iterator prototype;
its private source edge participates in native reachability. Supported records,
iterables, ordinary properties and callee realms use existing runtime semantics;
native-host initialization and unsupported Proxy operations retain explicit guards.

Native list backing and text capacities spend the existing retained-value ledger,
with fallible growth and copy/work accounting. Conversion scratch stays charged
across reentrant callbacks and unwinds on errors. Mutation candidates are private
until admission succeeds; refusal preserves the real list. There is no extra
header-count limit copied from URLSearchParams. Immutable native lists reject
mutation after argument conversion; request/response adapter guards remain separate.

An original App test transfers authored Headers through the real XHR adapter and
verifies a loopback POST/body, combined header values, native cookie ownership,
forbidden Host/Cookie filtering, HttpOnly response filtering and rendered response
text. This is a Headers/transport integration result. Fetch, streaming bodies and actual-site request/body success are not supplied
by the Headers increment.

## Buffered Response values and native Body consumption

Response has canonical readonly status/type/URL/redirect/header/body-use slots over
private native data. Constructors implement ResponseInit dictionary conversion,
unsigned-short status conversion followed by the 200-599 check, ByteString reason
phrases, null-body status rejection and genuine response-guarded Headers. Authored
Set-Cookie fields are filtered by that guard. Header identity is stable; clones
have independent native lists. Response.error creates a status-zero network-error
value with immutable Headers; it does not perform or claim a network request.
Response.json serializes actual data with the existing original JSON engine.

Body strings undergo USVString conversion; ArrayBuffer and view bodies snapshot
the actual byte range after dictionary conversion, and URLSearchParams uses its
native post-conversion serializer. Supported content types are supplied only for
those genuine body kinds and do not replace explicit Content-Type values. Body
text/JSON always use UTF-8 with BOM handling, independently of XHR charset sniffing.
ArrayBuffer/Uint8Array results use the response's relevant realm and copied bytes.

Text, JSON, arrayBuffer and bytes return intrinsic Promises with genuine queued
reactions and ordinary thenable resolution. Invalid brands, used bodies and JSON
syntax errors reject with actual exceptions; resource/capability faults remain
fatal. Null bodies can be consumed repeatedly without becoming used. Non-null
branches become used once, clones remain independent, and clone-after-use rejects.
Immutable source bytes are shared across clones; consumed branches release them,
and the last branch refunds their actual backing even when consumption fails.
Native metadata, conversion scratch and copied outputs retain ordinary allocation,
work, copy and owner-realm checks. The new value is rooted across Promise creation.

ReadableStream access, Blob/FormData and static redirect are not implemented in
this buffered increment. A non-null body getter explicitly refuses rather than
claiming null or supplying an empty/fake stream; it remains non-null in that sense
after consumption. Network Fetch still needs its real shared transport adapter.
An original App fixture consumes actual loopback XHR JSON through Response and
queued Body jobs, updates real text pixels and keeps native editing/link targets.
This is a connected fixture result, not actual-site initialization or playback.

## Element ARIAMixin nullable string reflections

The Element prototype owns role and the 43 ARIAMixin nullable DOMString accessor
pairs. They are inherited by supported HTML and SVG elements, with correct getter
and setter names/lengths and configurable/enumerable descriptors. Reflection reads
real unqualified attributes, preserves raw strings and distinguishes absence from
present-empty. Original null/undefined remove the attribute; other values undergo
one String-hint conversion. False and zero become strings, distinct from hidden's
union behavior. Wrong brands and origin access reject before observable conversion;
unsupported child writes reject before conversion as well.

Supported ordinary own author data/accessor descriptors shadow lookup independently
of native slots. Saved canonical accessors still reach the real attributes, including
through supported main/detached/inert HTML and main SVG nodes. Parked-template
promotion preserves author accessors and live Attr identity; import/clone creates
independent real sources. Inert SVG reflects actual passive attributes without
opening its unrepresented ordinary descriptor store. Named-form reads retain their
native precedence while canonical and inherited assignments target real attributes.
ARIA metadata does not synthesize native disabled/checked control behavior.

Native reflected writes use existing atomic attribute storage, aliases, mutation
observation and resource/origin guards. Null removal of an absent attribute creates
no mutation record. Scalar DOM attribute-string limitations remain explicit; no
UTF-16 replacement or invented success is introduced. Relationship-element/FrozenArray
attributes, ElementInternals, ariaNotify and actual accessibility-tree/platform output
are separate unfinished contracts.

An original App fixture combines an author override with saved native accessors,
attribute observation/jobs, real label/state data, CSS selector repaint, preserved
native editing and link activation. Functional realm fixtures use the product
admission profile to include the expanded native IDL surface; explicit narrow
admission/copy-work cases keep their own profiles. The unchanged 1,024-function
adversarial profile still refuses incomplete intrinsic bootstrap and preserves main
native state. Parsed HTML iframe fallback now retains its actual Text child and its owner.
Empty frames keep their live child lists. Element-shaped legacy fallback still
refuses child-handle publication. Broader DOM/HTML/SVG conformance and site
acceptance remain open.

## Same-inert-owner Node replacement and loaded parked promotion

Node.replaceChild is inherited and callable normally or through a saved native
method on represented inert Elements/fragments, including main-slot identities
currently parked in an inert owner. Both Node arguments are validated before the
tree algorithm; wrong types do not coerce ordinary objects. Hierarchy/cycle and
membership checks precede owner-policy refusal. Named replacement hierarchy,
not-found and supported-policy errors use genuine DOMException values. Inert
Document structure and cross-document adoption remain explicitly unsupported.

Replacement returns the old child, preserves real node/attribute/list identity,
handles both existing-sibling orders and removes an incoming descendant from its
old subtree. Fragment children are spliced in order, leaving the donor empty;
an empty fragment removes the old child. Source/destination backing, moves and
parent writes are paid before publication. Allocation or copy refusal leaves
both trees, generation and retained storage unchanged.

Observer preflight uses real owner paths and snapshots. Moving an existing node
or draining a fragment queues its removal record, followed by the combined
replacement record. Transient subtree observation survives removal until its
checkpoint. Self replacement follows adopt/remove/insert observation, retaining
the final tree identity. Saved same-origin child-realm methods revalidate their
original owner on later calls; protected child DOM writes remain refused.

Promotion now accepts both generated snapshots and unchanged loaded sources in
reserved parked main slots. It copies loaded data once, refunds the main-slot
payload actually replaced and preserves nonce/resource checks. Post-swap preflight
failure restores the original slot and inert ownership; a later valid promotion
can recover. No unrelated attribute write is needed to make a parked leaf usable.
Unreserved inert handles still require real import/copy, not fabricated main IDs.

An original App component-style fixture replaces within template content, observes
the genuine queued mutation, deep-imports the result, paints text/link pixels and
accepts native input and link activation. This finite owner increment does not
establish exact captured-call causality, complete adoption/document replacement,
full platform lifecycle or actual-site readiness. Main-tree same-node replacement effects are described in the following section.

## Main-tree self-replacement removal effects

Replacing a supported main-tree node with itself retains final links and identity,
but performs its observable adoption/removal/insertion effects. The native observer
plan queues removal followed by insertion with the original sibling boundaries.
Live Range points inside the removed subtree collapse to the parent/index, and
parent-offset adjustments use the real remove/insert rules. Native focus within
the removed subtree is cleared and the App receives the removal acknowledgment;
control editor values, caret/selection data and node/list identities remain real.

Existing hierarchy, resource, namespace and subtree checks precede these effects.
Range work is admitted before any boundary/focus/generation change. Refusal leaves
those states unchanged; a later job can recover. The final unchanged vectors need
no redundant removal/reinsertion allocation, while mutation generation and relevant
layout/focus invalidation still occur. Wider custom-element, resource and focusing
lifecycle remains bounded by the existing supported mutation paths.

An original RED case reproduced the missing records on the old early return.
Focused observer/Range/refusal cases and an actual App trusted-keyboard/editor/
focus-style/link case qualify the correction. This is not actual-site acceptance.

## SVG CSS geometry core and actual inline viewport

The original SVG image renderer handles x/y/cx/cy/r/rx/ry/width/height where those
properties apply to the supported element. It combines presentation attributes,
its supported embedded selectors and inline declarations with source/specificity
and important priority. Inapplicable geometry declarations do not disable path
painting. Geometry is not inherited by default; explicit inheritance and initial/
unset values are separate. Percentages use the appropriate SVG viewport axis or
normalized diagonal, and absolute plus em/rem values use computed font context.
Auto radii resolve from their counterpart; negative geometry follows the SVG2
initial-value handling rather than becoming a fabricated resource failure.

Numeric token syntax is separate from the existing coordinate refusal. Source,
node, segment, coordinate, transform, work/layer and cancellation guards remain;
the negative-radius fixture now checks transparent output, with a distinct actual
coordinate-refusal witness. Unimplemented expressions, transform/d geometry and
wider units/rollback retain the renderer's finite boundaries. No limit increase,
SVG library, remote resource loader or page stylesheet rewrite was added.

App source snapshots retain separate intrinsic dimensions and rendered viewports.
After real shared layout, a changed inline SVG viewport is rasterized using that
used box and viewBox/preserveAspectRatio mapping. Its bitmap size cannot become
the next natural size; media resizing back to auto geometry uses the intrinsic
cache. Source and viewport invalidation share the existing render deadline and
decoded-image allowance, and failed replacement discards stale paint. Root sizing
from supported external HTML CSS participates through actual layout. External
SVG descendant geometry and variables still require the shared computed cascade
bridge and are not claimed complete by this increment.

SVGElement.prototype owns its CSSInlineStyle getter/forwarding setter and distinct
native functions, keeping SameObject style identity and ordinary own shadows.
Saved SVG/HTML accessors enforce their own interface receiver brands before value
conversion; unrepresented inert/child stores remain guarded. The supported CSSOM
property subset and passive/local SVG paint-server policies remain explicit.

An original App reduction reproduced aspect distortion before used-viewport
rasterization. It now checks real letterbox pixels, media auto-size restoration,
CSSOM resize repaint, preserved editing and native link activation. Decoder cases
cover geometry/cascade/unit/priority and guarded cancellation behavior. These are
finite platform/fixture results, not a completed actual shell or SVG conformance.

## SVG geometry in the shared document cascade

The document CSS cascade now retains typed SVG geometry alongside its immutable
computed-property snapshot. It uses the same matched external/embedded selectors,
priority/order, inline declarations and inherited custom-property environment as
HTML layout. Geometry presentation attributes precede CSS; unitless SVG font-size
hints precede matched font rules. Computed font-relative lengths use final element
and HTML-root font sizes; percentages remain unresolved until a viewport is known.
Geometry is noninherited by default, with explicit inheritance using parent
computed values even when the property does not affect that parent's own shape.

Unsupported or resource-refused winners remain unavailable per property. Missing
or invalid substituted variables use the geometry initial value, invalid ordinary
syntax does not displace an earlier winner, and a later valid declaration can
recover an unavailable property. Sparse snapshot backing and variable work use the
existing CSS admission paths; a failed computation publishes no replacement.

Inline SVG painting now consumes this typed geometry snapshot through a separate
internal renderer context. Source serialization preserves author attributes and
preorder identity; a context/source node-count mismatch refuses. Static image
SVG retains its own image-local cascade. Inline rendering obtains the document
cascade before intrinsic image sizing, with the same media/checked state as the
subsequent layout and the existing image deadline. The prepared stylesheet cache
is reused; selector computation currently occurs for intrinsic paint and layout.
Cached source identity includes computed geometry and fonts, so external sheet,
variable, media or DOM changes invalidate paint even when SVG markup is unchanged.
Geometry context backing joins existing source-cache admission; decoded bitmap
admission, local references and cancellation remain checked.

The original App fixture first reproduces ignored external descendant geometry,
then verifies native dynamic SVG pixels, inherited-variable changes, media resize,
refusal/stale-paint removal and recovery with edited input and real link activation.
An unsupported winner retains the existing unavailable-image indication. Clearing
an obsolete SVG failure status requires successful source and viewport stages;
other status messages are preserved. The context also carries the finite shared paint family described below.
CSS math, rollback, wider units and full SVG/font/viewport semantics are unfinished.
No author DOM/style attributes are rewritten. Focused computation, renderer-context and connected App/refusal
checks and the selected affected CSS/SVG cases do not establish actual-site shell
readiness or complete SVG conformance.

## Native Error console provenance and saved namespace operations

The implemented console log/info/warn/error methods are real cached function
values in the native holder's owner realm. Saved, unbound, call/apply and bound
uses retain that owner and its origin validation. Namespace calls ignore their
JavaScript this value; interface method receiver checks remain in place.

An explicit native printer boundary receives bounded text, with no access to the
interpreter's object arena. The runtime describes genuine Error identities using
its private native-origin record when retained, including original fault position
and source excerpt. Constructed Errors can expose an existing own string message;
author accessors, ordinary object fields, proxies and coercion hooks are never run
for inspection. Unavailable or inaccessible details remain opaque. Original Error
identity, author descriptors and exception/rethrow behavior are unchanged.

The formatter counts bounded output, reserves it under the current copy budget,
then renders without cloning an unbounded script string. Admission failure emits
no console record; ordinary native calls keep their original argument values.
The existing diagnostic/log retention and native-origin sampling bounds remain.
An original App fixture combines a trusted native event, intrinsic Promise job,
original native fault, console record and real text/background update while
preserving edited input and native link activation.

This finite printer does not implement the full console Formatter, arbitrary
object inspection, all namespace descriptors or automatic global ErrorEvent
reporting. It cannot infer that an earlier opaque site object was a native Error,
identify an unknown deployed binding, or establish actual-site success without a
source-bound product checkpoint. The relevant specifications are the
[Console printer](https://console.spec.whatwg.org/#printer) and
[Web IDL namespaces](https://webidl.spec.whatwg.org/#namespace-object).

## Shared SVG fill, stroke and color

The original inline SVG painter receives inherited fill/stroke/color from the
same document selector, priority and variable cascade as geometry. HTML ancestors
can supply inherited paint; SVG presentation attributes precede matched and inline
CSS. The finite paint grammar reuses the original renderer's colors (including
alpha), none/currentColor and supported local paint servers. CurrentColor resolves
against the final consuming element's color, including gradient stops. Missing
variables use inherited values for this family. Unsupported winners remain
unavailable per property; later supported declarations can recover.

CSSStyleDeclaration now supplies fill and stroke source values through its real
property/index/priority/mutation paths. Existing CSSOM URL refusal remains, and an
external stylesheet's unrepresented paint URL never causes a request or retains
old pixels. Native author attributes are not rewritten into computed values.

Only SVG content and its actual HTML ancestors compute paint state; unrelated
HTML branches and inert template content add no paint allocations. Paint uses the
supplied CSS job budgets rather than the custom-property map's independent scratch
allowance. The existing checked allocation, image/cache admission and deadline
remain. Shared geometry, paint and local-server string backing is charged once by
identity across root caches, including shared inherited values.

Original reductions cover alpha/currentColor, priority, real local gradient
definitions, unavailable color through gradient currentColor, copy refusal and
recovery, and shared cache backing. An actual App fixture first reproduces ignored
external fill, then verifies external variables, inherited HTML fill, CSSOM/class
changes, important priority, refusal/recovery, edited input and native link use.
Parsed connected stylesheet text mutation retains its existing guard; this paint
fixture uses actual class and CSSOM mutations. It does not qualify all component
stylesheet lifecycle behavior.

The family remains finite: wider color syntaxes, context paint, server fallback,
additional shared stroke/fill rules and opacity properties, geometry math/rollback,
nested SVG and wider graphics remain unfinished or guarded. Native CSSOM source
readback is separate from complete computed paint readback. These checks do not
establish actual-site initialization or complete SVG conformance. See
[SVG paint and color](https://www.w3.org/TR/SVG2/painting.html#SpecifyingPaint).

## Resource-governed iterative Array flattening

Array.prototype.flat and flatMap share an explicit frame worklist. Its depth no
longer borrows the parser/evaluator nesting counter. Both initial storage and
subsequent growth use fallible, unpublished reservations; actual backing and
movement/write work are admitted before frame publication. Frame capacity joins
shared retained-value accounting, including during nested author callbacks. Each
invocation refunds only its owned workspace on normal or abrupt completion.

The source, mapper/receiver/depth values, partial species target and suspended
array frames have explicit root registrations. A detached parent frame and a
partial output containing functions remain rooted across a getter's reentry.
Temporary registrations unwind with the operation. Callback allocations and
observable writes to an exposed species target remain intact after an exception.

Finite and Infinity depths, one-level flatMap, holes/inherited indices, captured
lengths and live reads retain their semantics. Empty sources still validate the
mapper and perform species construction, then return without a frame allocation.
Repeated array identities are expanded normally; finite cyclic input retains its
actual boundary identity, and infinite cyclic traversal stops on real work or
storage admission rather than returning truncated output. Native-stack checks
remain around actual reentrant Script execution. The existing output/property,
retained-value, source/species and checked-index boundaries remain.

Original witnesses construct256 nested arrays iteratively and prepare128-level
component behaviors through real template import, native controls, pixels and
link activation. A shallow operation from72 supported Script callers reproduces
the old borrowed64 refusal; its test worker supplies8MiB while keeping native
stack checks. The96-level heap-expression case already passed before this fix.
Focused checks also cover species/mapper abrupt effects, live inherited holes,
exact small-budget growth/copy refusal and recovery, nested workspace lifetime,
root publication and cyclic work. These are Windows runtime/App checks, not a
fresh captured-site or complete Array conformance result.

The directly related review found paid iterative JSON parse/revive/stringify
frames without this borrowed64 restriction. Array string joining has actual
native recursive conversions protected by measured stack checks; its small depth
witness is test-only. Parser/lexer and RegExp structural/storage work remains
separate, as do the wider capacity inventory and architectural work. The global
parser guard is unchanged. See the
[FlattenIntoArray algorithm](https://tc39.es/ecma262/multipage/indexed-collections.html#sec-flattenintoarray).

## Loaded flow-template SVG and connected component focus

Balanced supported SVG in loaded HTML templates under represented ordinary flow
contexts retains real SVG namespace identity, including nested templates and
custom HTML containers. Previously the template ancestor marked the SVG island
as unrepresented foreign content, so inert queries refused before preparation.
Namespace stamping still requires the existing balanced supported shape tree;
unsafe/unrepresented foreign content, links, unsupported ancestor contexts and
recovery remain guarded. Templates stay inert, and import/connection still use
normal namespace, resource and CSP admission. This does not implement the full
HTML template/table/foreign-content tree builder.

Document.prototype supplies a configurable enumerable readonly activeElement
accessor over committed main-document native focus. Connected focused elements
retain actual identity; otherwise the real body, document element or null is
returned. Supported inert documents and read-only blank child prefixes use their
actual document fallback. Saved accessors retain Document brands, holder and
receiver owner/origin validation. Supported main Document author shadows and
prototype deletion remain distinct from the saved native getter.

Settled page and keyboard events now consume their latest queued focus request
after their observer/Promise jobs and layout/scroll processing. The existing native
control focus path performs eligibility checks and editor/focus-event updates;
the next native edit reaches the requested control without a separate idle poll.
Hidden, disabled or detached targets are refused, and a consumed invalid request
does not later resurrect after the target is restored. This preserves committed
focus readback rather than implementing all synchronous/reentrant focusing steps,
ShadowRoot retargeting or interactive child-frame focus.

An original connected DOM-layer fixture combines nested inert preparation, saved
Node-method wrappers and WeakMap identity, declarative-attribute removal, deep
import, style cloning/placement, author/native ARIA forwarding, a Text-node
MutationObserver scheduler, Promise jobs, event registration, real controls/icons,
focus, immediate typing and link activation. Observer callbacks remain deferred.
The source first reproduced the namespace refusal and then immediate typing at
the old button; the corrected fixture passes without a manual focus repair.
A keyboard fixture checks a valid redirect and hidden/disabled/detached rejection.

The fixture models required DOM calls for the selected noPatch path. It does not
execute a matching deployed polyfill, provide native Custom Elements/Shadow DOM,
or demonstrate a usable actual site. Functional child FocusEvent and offsetParent
owner cases use the browser profile to reach their assertions after the expanded
native interface bootstrap; explicit narrow quota fixtures remain unchanged.
Focus and parsing semantics follow the
[HTML focus management APIs](https://html.spec.whatwg.org/multipage/interaction.html#focus-management-apis)
and [template parsing](https://html.spec.whatwg.org/multipage/parsing.html#parsing-main-intemplate).


Ordinary HTML name materialization now shares the namespace, local-tag and null-namespace attribute policy with public producers. Native import copies exact raw names without changing source owners; parked and cloned template contents preserve Attr identity through promotion and update exact live queries. Work, copy and retained-storage refusal preserve source and destination state with fresh recovery. Native controls, slots, frames, resource elements and foreign or namespace-qualified raw names remain guarded. Original native/App and production checks qualify this slice; child writes and frozen browser qualification remain separate. F01 stays partial.


Common HTML metadata materialization now admits exact null-namespace class, title, lang
and dir values on supported HTML elements. Imports retain independent attributes and
source ownership; parked and cloned template contents preserve exact metadata through
promotion. Class tokens, live class queries and CSS alias exclusion use actual code
units; title/lang retain raw values and dir keeps invalid-value reflection semantics.
Imported native controls retain real editor values. Work, copy and retained-storage
refusal preserve source/destination state and fresh recovery. Original native/App and
production checks qualify this slice, including actual class-based painting. Foreign or
namespace-qualified raw metadata and unsupported active raw attributes retain their
guards; frozen browser qualification remains separate. F01 stays partial.


Canonical Element.className now reflects the actual class DOMString through inherited
getter/setter metadata, saved accessors and ordinary prototype
override/deletion/restoration. Genuine receiver and captured-origin checks precede
conversion; inert HTML property writes now update native attributes, stable Attr
identities and exact observation values. Main and inert public producers retain raw
units through import and template promotion, with class tokens, live queries and CSS
alias exclusion using the actual record. Paid reads and work/copy/string/storage refusal
retain fresh recovery. Original native/App and production checks qualify this base
reflection, including class-based pixels and preserved editor/link/page identity.
Initial child writes, main raw SVG writes, SVGAnimatedString-specific interfaces, inert
classList and frozen browser qualification remain separate. F01 stays partial.

Canonical Element.classList now has inherited getter/setter metadata, stable supported main HTML/SVG token-list identity, saved accessors and ordinary prototype changes. Its PutForwards setter validates the genuine receiver and captured origin before reading the current classList property, then forwards the original value to the selected object value setter. Author getters, setters, proxies, abrupt effects and late native DOMString conversion retain their order; false Set results are ignored even for strict callers. Native attribute identity, observations and resource refusal/recovery remain shared with DOMTokenList.value. Original native/App and production checks qualify this slice, including class-based paint with preserved editor/link/page identity. Default inert, child and other token-list owner storage and frozen browser qualification remain separate. F01 stays partial.

Inert HTML classList now binds stable native token-list handles to actual owner attributes, including elements without a main-document slot. Exact value, indexed and membership reads, iteration, add/remove/replace/toggle and PutForwards writes share the existing native readers, serializers and attribute transactions. Scoped immutable node views copy no attribute payload or token cache; token metadata and selected output share admitted scratch headroom. Real Attr identities, observation old values, independent import/clone state and saved token lists survive supported template promotion. Original native/App and production checks cover raw-versus-replacement identity, conversion/validation ordering, work/copy/string/storage refusal, cancellation and fresh recovery, with actual class-based paint and preserved editor/link/page identity. Child and inert foreign token-list owners, broader SVG mutation and frozen browser qualification remain separate. F01 stays partial.

Genuine inert XML and other namespace Elements now use the existing bounded native ordinary-property store. Exact UTF-16 and Symbol keys, descriptors and author accessors preserve real Element identity and native attribute state; clone/import omit author properties, while supported adoption preserves them. Canonical native accessors bypass author shadows, and classList PutForwards can select an author getter without prematurely converting the original value. Native quota refusal/recovery and captured child-origin checks remain enforced. Original native/App and production checks qualify this slice, including HTML paint driven through genuine XML author accessors with preserved editor/link/page identity. Default inert foreign token-list storage, broader foreign materialization and frozen browser qualification remain separate. F01 stays partial.

Inert Element classList now binds the shared native token backend across null, XML, MathML and SVG namespaces, including owners without a main-document slot. Exact reads, iteration, mutations and PutForwards use actual attributes and preserve Attr identity, observation and saved handles through supported adoption; clone/import token state remains independent. Inert className and token writes select the null-namespace class attribute without overwriting a same-spelling namespaced attribute. Original native/App and production checks cover namespace/case separation, raw UTF-16 identity, conversion ordering, captured origin, scratch headroom and resource/cancellation refusal with fresh recovery. Actual App paint consumes XML token values through supported HTML materialization while preserving editor/link/page state. Initial child token storage, broader foreign materialization, main SVG raw writes and frozen browser qualification remain separate. F01 stays partial.

Inert Element.id and common HTMLElement reflection setters now select null-namespace content attributes using the shared native namespace-aware transaction. Same-spelling namespaced attributes remain independent for id, title, lang, dir, hidden and tabindex; native Attr identities, exact ID lookup, observation and clone state follow the selected attribute. Original native/App and production checks cover raw UTF-16 values, native conversion reentry, owners without a main-document slot, captured origin and resource/cancellation refusal with fresh recovery. Actual App paint uses the reflected native values through supported HTML consumers with preserved editor/link/page identity. Generic setAttribute/removeAttribute retain their qualified-name behavior; broader foreign materialization, child mutation and frozen browser qualification remain separate. F01 stays partial.

Nullable ARIA string reflection now reads exact native DOMString values through the shared optional attribute reader, preserving absent-null versus present-empty state and raw UTF-16 identity. Genuine inert Element setters retain nullable conversion, actual null-namespace Attr identity, observations and independent clone/import state through supported adoption; same-spelling namespaced attributes remain separate. Original native/App and production checks cover registered mappings, native author-shadow bypass, conversion reentry, receiver/Symbol/origin ordering, bounded raw reads and resource/cancellation refusal with fresh recovery. Actual App paint uses reflected inert values through supported HTML consumers with preserved editor/link/page state. Displayed raw ARIA writes and raw ARIA materialization, child writes, accessibility-tree integration and notifications retain their separate boundaries. F01 stays partial.

Displayed HTML now preserves exact registered ARIAMixin and role DOMStrings through canonical nullable setters, generic attribute producers and attached Attr value/nodeValue/textContent writes. The same HTML/null-namespace qualification admits existing independent import and template promotion consumers, preserving actual Attr identity, owners, observations and literal CSS separation. Native control value state remains independent of ARIA metadata. Original native and in-process App checks cover registered mappings, transfer isolation, genuine paint/editor/link state and atomic resource/cancellation refusal with recovery. Foreign or namespace-qualified raw metadata, unknown ARIA attributes, child writes, accessibility-tree integration and notifications retain their existing boundaries. F01 stays partial.

Public HTML label for values now retain exact DOMString units through generic attribute producers, attached Attr aliases and native attribute-node replacement. The shared HTML-label/null-namespace qualification also admits independent label imports and template promotion while preserving native Attr identity, owners and observations. Existing native label association compares the actual for and ID records, retaining first tree-order noncontrol blockers, explicit misses and disconnected-node boundaries. Original native checks exercise exact versus replacement-character targets, conversion effects, origin validation, quota/cancellation refusal and transfer recovery. In-process App pointer input activates the actual exact target after public writes, import and promotion while preserving neighboring editing, link hits and page identity. Raw for values on other or foreign elements, qualified raw attributes, label IDL APIs and wider form-owner resetting remain separate. F01 stays partial.

Canonical HTMLLabelElement prototypes now expose enumerable/configurable htmlFor accessors over actual HTML-label attributes. Main and inert labels retain exact DOMString conversion of null and undefined, absent-empty reads, null-namespace Attr identity, original observations and independent clone/import/adoption state. Saved native accessors preserve real brands and captured origin checks before author conversion; ordinary own/prototype shadows and deletion keep the underlying native attribute separate. Bounded reads, writes, cancellation and no-main-slot creation retain refusal/recovery evidence. Original in-process App pointer checks use htmlFor writes through actual native label targeting, import and template promotion with checkbox paint, neighboring editor/link state and page identity. Direct construction, child writes, label control/form IDL and wider form-owner resetting retain their existing boundaries. F01 stays partial.

HTMLLabelElement.control now exposes a canonical readonly accessor over actual main and inert HTML label trees. Exact null-namespace for and ID records select the first tree-order ID match only when it is labelable; absent for selects the first labelable descendant, with hidden inputs and foreign elements excluded. Built-in meter, output and progress elements participate even without native widget state. Detached, cloned, imported, adopted and promoted roots retain actual element identity; author properties do not become association authority. Native origin validation, bounded attribute comparisons, admitted traversal scratch, cancellation and fresh recovery remain checked. Original in-process App checks compare the accessor with actual pointer activation while retaining checkbox paint, neighboring editing, link hits and page identity. Child-tree association, form-associated custom elements, the label form getter, native input-type changes, native-control template promotion, wider form-owner resetting and full label activation semantics remain incomplete. F01 stays partial.

HTMLLabelElement.form now exposes a canonical readonly accessor for the actual associated control owner. Existing displayed native controls retain their stored form owner; pure inert and ordinary controls without native widget state use bounded native connected or detached tree association. Connected explicit form values compare exact null-namespace IDs, require the first match to be an HTML form, and retain nonform and replacement-character blockers. Detached explicit references fall back to the nearest actual HTML form ancestor. Meter and progress remain labelable without a form owner; output participates in form association. Native reads ignore author control/form getters and retain descriptor shadows, clone/import/adoption and supported label-promotion identity, origin, resource and cancellation refusal/recovery. Original in-process App checks compare form and control accessors with native pointer activation, checkbox paint, adjacent editing and link state. Child trees, form-associated custom elements, broader parser/form-owner resetting, native input-type changes, native-control template promotion and full activation semantics remain incomplete. F01 stays partial.

HTMLInputElement.labels now returns a canonical readonly, stable-identity live NodeList over actual same-root HTML-label associations. Exact for and ID values, first nonlabelable blockers, implicit descendants, current tree order and disconnected roots use native association records. Hidden input reads return null while an already retained list updates to empty, then recovers its original identity when the inert type changes back. Native collection item, keys, iteration, callbacks and argument-conversion reentry read current membership; author properties never become association authority. Identity creation and refresh retain origin checks, metadata quotas, bounded native work/copies, simultaneous outer and nested traversal scratch admission, cancellation and fresh recovery. Clones, actual adoption, independent import and supported label promotion retain native identity, with in-process App pointer/paint/editor/link evidence. Other built-in labels interfaces, child and unsupported parser backings, form-associated custom elements, native input-type changes, native-control template promotion and full label activation remain required unfinished work. F01 stays partial.

HTMLButtonElement.labels and HTMLTextAreaElement.labels now expose distinct canonical readonly accessors over the shared bounded live label-list runtime. Their native NodeLists retain stable identity, exact same-root associations, current tree order, ordinary author descriptor precedence and native collection iteration. A hidden type content attribute does not suppress either non-input list. Clone/adoption/import and supported label promotion retain actual source authority; captured-origin, metadata, work/copy, simultaneous traversal storage and cancellation refusals retain fresh recovery. In-process App checks retain native pointer, paint, editor, link and page identity evidence. Meter, output, progress and select labels interfaces, child and unsupported parser backings, form-associated custom elements, native input-type changes, native-control template promotion and full label activation remain unfinished. F01 stays partial.

Meter, output, progress and select now have canonical native HTML interfaces and distinct readonly labels getters, completing the seven ordinary built-in labelable interfaces in the supported main and inert trees. Shared operation metadata keeps per-realm descriptor installation and genuine receiver validation aligned. Stable live NodeLists retain exact native associations, current tree order, author descriptor precedence, clone/adoption/import identity, supported label promotion and collection iteration. Non-input hidden content never suppresses these lists. Native origin, metadata, bounded work/copies, combined traversal storage and cancellation retain refusal/recovery checks. App evidence retains actual select state and popup interaction alongside label updates, paint, editor, link and page identity. Whole-control main import for meter, output and progress remains guarded with atomic refusal; label imports and supported select imports retain actual associations. Other interface IDL, child and unsupported parser backings, custom-element internals, native input-type changes, native-control template promotion and full label activation remain required unfinished work. F01 stays partial.

Public HTML OUTPUT for attributes now admit exact DOMString values through setAttribute, explicit null-namespace writes and attached Attr value/nodeValue/textContent aliases. Native attribute identity, exact observer old values, namespace collisions, author conversion effects and CSS replacement-character exclusion retain the original UTF-16 units. Output input-reference tokens remain independent of the output element's own labels, labeled control and form owner. Work/copy/storage/cancellation and captured-origin refusals preserve native records and fresh recovery. In-process App evidence follows actual button writes through exact attribute-based paint while preserving neighboring editing, link hits and page identity. The canonical output htmlFor DOMTokenList binding, other output value/default-value IDL and whole-output main import/template promotion remain required unfinished work. F01 stays partial.

HTMLOutputElement.htmlFor now exposes a canonical SameObject DOMTokenList over its actual null-namespace for attribute. Exact UTF-16 value and token reads, mutation, iteration and observation share the existing bounded native token runtime. Its PutForwards setter performs ordinary Get before forwarding the original value to the selected object value setter, preserving author overrides, proxy behavior, conversion effects, receiver brands and captured-origin checks. Stable handles survive inert cloning and adoption without consuming displayed-node slots; the list remains independent of classList and the output element own label association. Focused native and in-process App checks cover exact attribute-based paint and interaction. Other output IDL, whole-output main import and promotion, child lifecycle and complete browser qualification remain required; F01 remains partial.

HTMLOutputElement.form now exposes a canonical readonly accessor over the shared native form-owner lookup. Native widget owners remain authoritative; ordinary and inert output elements resolve exact null-namespace form/ID records, first-match blockers, connected explicit owners and detached nearest-form ancestry without consulting author getters or htmlFor tokens. Inert clones, adoption and HTML-namespace output elements in XML documents retain independent owner trees without displayed-node slots. Work, copy, traversal storage, cancellation and captured-origin refusals retain recovery. In-process App checks follow actual output tree moves through form and label getters, exact CSS paint and nearby editing/link behavior. Native Reflect.set retains its existing explicit refusal. Other output IDL, whole-output main import/promotion, child lifecycle and complete browser qualification remain required; F01 stays partial.

HTMLOutputElement now exposes canonical exact DOMString name accessors and readonly type metadata returning output independently of the type content attribute. OUTPUT name writes through IDL, public attributes, explicit null-namespace writes and attached Attr aliases share the existing paid native transactions and exact live getElementsByName lookup. Author descriptors, conversion reentry, actual Attr identity, observer old values and CSS replacement-character exclusion retain native semantics; name does not select the output form owner, labels or htmlFor list. Inert clone/adoption and HTML-namespace metadata in XML owners require no displayed-node slots. Native work/copy/storage/string/cancellation and captured-origin refusals retain recovery; in-process App evidence follows actual name-based paint and stable nearby interaction. Output value/defaultValue/reset, constraint validation, complete form.elements participation, whole-output main import/promotion, child lifecycle and full browser qualification remain required; F01 stays partial.

HTML OUTPUT textContent now uses the native whole-child replacement transaction for connected and detached main-document elements. Exact DOMStrings create a fresh Text node, empty values remove all children, and saved descendants, live NodeLists, Range boundaries and childList records retain native identity. Ordinary parent replacement can retire a passive OUTPUT subtree. Shared replacement traversal admits stack, descendant and caption backing with input text and overlapping growth before publication, charging copies and cancellation. Native control/resource removal and whole-output import/promotion retain their existing guards. Focused native and in-process App checks cover actual replacement, paint and neighboring interaction. Output value/defaultValue/reset and complete descendant-text reading remain required; F01 stays partial.

Native main-tree textContent now concatenates actual Text/CDATA descendants in tree order, including hidden and script/style/object/noscript source text, independently of paint activity. HTML template contents remain in their separate fragment; iframe fallback reads retain represented-topology guards and never walk child documents. Main and inert readers admit traversal/part backing, overlapping growth, returned text and temporary UTF-16 join units together, with paid copies and cancellation. Exact split surrogate units, saved children, native observers and resource ownership remain unchanged by reads. Focused native and in-process App checks cover actual source-derived text paint and neighboring interaction. Output value/defaultValue/reset and remaining unsupported child/parser lifecycles stay required; F01 remains partial.

HTMLOutputElement now exposes canonical exact DOMString value/defaultValue accessors. value reads actual descendant text and replaces real children while preserving the native default-value override; defaultValue replaces children only before an override exists, then updates the independent default without child mutation records. Empty overrides remain distinct from absence, inert clones start with independent default state, and adoption preserves existing native authority. Conversion reentry, genuine brands, captured-origin checks, bounded storage/work/copies, cancellation and failed child transactions preserve refusal/recovery behavior. Focused native and in-process App pointer/paint checks cover current/default text and independent form/label/token associations. Form reset, whole-output main import/promotion, child lifecycle, validation and full form.elements participation remain required; F01 stays partial.

Native reset-button actions now restore associated HTML OUTPUT elements from their actual default value and clear each native override only after successful child replacement. Selection uses current exact form ownership after uncanceled reset listeners, including external associations, with bounded traversal and retained scratch. Reset shares genuine child-list/Range transactions, preserves protected resource/control refusals, and does not emit input/change events. Observer delivery follows widget reset within the same action allowance. Scripting-disabled actions use the same bounded native text transaction without an interpreter. Focused native and in-process App pointer/paint checks cover default/empty modes, cancellation, ownership, quotas, recovery and independent editor/link/page state. Full form.reset IDL, form.elements, constraint validation, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

Public HTML OUTPUT form content attributes now admit exact DOMStrings through setAttribute, explicit null-namespace writes and attached Attr value/nodeValue/textContent aliases. The shared paid transaction preserves actual attribute identity, conversion reentry and observer old values; existing exact native owner lookup, label.form and reset selection consume the current attribute without a second authority. HTML name normalization, namespace isolation, sparse native state, resource/cancellation refusal and fresh recovery remain bounded. Focused native and in-process App pointer/paint checks cover current ownership, matching versus unrelated reset buttons and independent editor/link/page state. The output.form IDL property stays readonly, and native widget reassociation, whole-output main import/promotion, full form.reset/form.elements/validation and child lifecycle remain guarded or unfinished; F01 stays partial.

HTML OUTPUT now exposes canonical readonly willValidate and validationMessage getters and canonical checkValidity/reportValidity methods. OUTPUT is not submittable and therefore is not a constraint-validation candidate: these getters return false/empty and the methods return true without invalid events, focus, scrolling, child records or default-value changes. Genuine native branding, captured origin authority, work/deadline admission, normal author shadow/delete/restore behavior and inert clone/adoption remain enforced. Unobserved methods retain bounded private data-slot definitions; first observation creates a stable callable without changing ordinary descriptors or the function cap. Focused native and in-process App pointer/paint/editor/link checks cover refusals and recovery. Mutable custom-validity error state, ValidityState, other controls, full form.reset/form.elements, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

HTML OUTPUT now has native custom-validity error storage, canonical setCustomValidity(DOMString) and a live readonly ValidityState with stable native identity. Custom errors normalize CRLF/CR while preserving exact UTF-16; customError and valid reflect that one authority, and inapplicable OUTPUT constraints stay false. OUTPUT remains a noncandidate: validationMessage is empty and checkValidity/reportValidity return true without invalid events or UI effects. Required arguments, once-only conversion/reentry, abrupt completion, genuine brands, captured origin, author shadows and work/copy/string/storage/deadline refusals retain their native boundaries. Clone state is fresh; adoption preserves identity and custom error. Focused native and in-process App pointer/paint/editor/link evidence remains scoped. Other controls, form.reset/form.elements, whole-output main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLInputElement, HTMLButtonElement, HTMLSelectElement and HTMLTextAreaElement now expose canonical readonly form getters with concrete native brands. They share the existing exact owner lookup: admitted native widgets retain their actual owner authority, while inert/ordinary controls use current connected explicit IDs and detached ancestor rules without an owner cache. Readonly assignment, inert author shadow/delete/restore, native widget form-descriptor refusal, namespace isolation, captured origin and bounded work/deadline refusal/recovery remain enforced. Focused native and in-process App pointer/paint/editor/link checks cover real owner reads and unrelated state preservation. Native widget reassociation, child lifecycle, broader form APIs and whole-control import/promotion remain guarded or unfinished; F01 stays partial.

HTMLSelectElement and HTMLTextAreaElement now expose canonical readonly type getters. Select returns select-one or select-multiple from the exact null-namespace lowercase multiple attribute presence, independently of its value, size or widget admission; textarea returns textarea. Native and inert concrete brands, namespace isolation, captured origin, author override/delete/restore and work/copy/string/deadline refusal/recovery apply without copying attribute values. Focused native and in-process App pointer/paint/editor/link checks remain scoped. Multiple/listbox widgets, broader select/textarea APIs, whole-control import/promotion and child lifecycle remain unfinished; F01 stays partial.

HTMLFormElement now exposes a canonical readonly length getter that counts actual listed built-in controls with the form as owner, including fieldset/object/output and unsupported widget states, excluding image-button inputs. It traverses the actual root, respects template parking and native widget ownership, and uses exact connected IDs or detached ancestors for controls without widget state. No count cache or author property supplies authority. The outer traversal uses a bounded parent/sibling walk without scratch allocation; nested exact-owner lookup retains its own paid scratch boundary. Concrete brands, captured origin, existing native named-property precedence and work/copy/storage/deadline refusal/recovery remain enforced. Native and in-process App pointer/paint/editor/link evidence stays scoped. HTMLFormControlsCollection, RadioNodeList, broader form names/lifecycle, form-associated custom elements, main import/promotion and child lifecycle remain unfinished; F01 stays partial.

HTMLFieldSetElement now has a canonical native/inert interface and readonly form/type getters. Form ownership uses the existing exact current association helper with connected explicit IDs and detached ancestors; type returns fieldset. Concrete namespace/local-name brands, illegal direct construction, author override/delete/restore, captured origin and bounded work/copy/string/storage/deadline refusal/recovery retain native authority. Existing disabled-fieldset/first-legend widget behavior remains separate from metadata reads. Focused native and in-process App pointer/paint/editor/link checks are scoped evidence. Fieldset collections and broader form lifecycle, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical name DOMString reflection over the actual null-namespace attribute. Native and inert setters convert once, preserve exact UTF-16, use current Attr identity after conversion and retain native observer, live-name and selector behavior. Author overrides remain ordinary descriptors; canonical borrowed accessors keep concrete brand and captured-origin checks. Reads and writes preserve work/copy/string/storage/deadline refusal and recovery, with scoped native and App pointer/paint/editor/link evidence. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical disabled Boolean reflection over its own actual null-namespace attribute. Assignment uses Boolean truth without author primitive conversion; native writes reuse the existing paid descendant-control refresh, including nested fieldsets and the first-legend exception. Inert reflection, actual Attr identity and observer old values, concrete brands, author overrides and captured origin share the existing attribute transaction. Presence reads inspect paid attribute names without copying values or requiring string/storage allowance; work/copy/storage/deadline refusals preserve attributes and native control state before recovery. Native and App pointer/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes canonical readonly willValidate and validationMessage getters plus ordinary checkValidity/reportValidity methods for its noncandidate validation branches. They return false, empty text and true respectively, without invalid dispatch, focus/scroll changes, author argument conversion or descendant-control validation. Native and inert concrete brands, captured origin, author override/delete/restore and work/deadline refusal/recovery remain enforced. Empty/scalar results require no value copying or additional string/retained storage; native and App pointer/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement now exposes a stable live ValidityState and canonical setCustomValidity using the shared native custom-error index. Exact DOMString conversion, newline normalization and joint input/payload/index storage admission match OUTPUT; each concrete interface retains its own borrowed-accessor brand checks. Only customError and valid vary for these noncandidate controls; all other validity flags stay false, validationMessage stays empty and check/reportValidity remain true without invalid events or UI changes. Clones start with fresh errors, adoption preserves identity, and native form reset preserves existing custom errors independently for FIELDSET and OUTPUT. Readonly flags, author overrides, captured origin and work/copy/string/storage/deadline refusal/recovery retain native authority. Native and App pointer/reset/paint/editor/link evidence stays scoped. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLFieldSetElement form content attributes now admit native public/null-namespace and attached Attr writers through the shared attribute transaction, including exact UTF-16 values. Current exact IDs determine the fieldset owner independently of author properties; changing the fieldset owner does not reassociate descendant controls or redirect their native reset. Conversion reentry, actual Attr identity, observer old values, namespace/raw-value guards, captured origin and bounded work/copy/string/storage/deadline refusal/recovery retain existing authority. Inert clone/adoption and native App pointer/reset/paint/editor/link checks remain scoped evidence. Fieldset collections, broader forms, main import/promotion and child lifecycle remain required; F01 stays partial.

HTMLLegendElement now has a canonical native/inert interface and readonly form getter. Only an actual direct HTML fieldset parent supplies ownership, using the shared current fieldset association helper; other parents return null. Exact namespace/local-name brands, illegal direct construction, descriptor and enumeration behavior, author override/delete/restore, captured origin and bounded work/copy/storage/deadline refusal/recovery retain native authority. Exact owner IDs, inert clone/adoption and native App pointer/reset/paint/editor/link evidence remain scoped. Broader form collections/lifecycle, main import/promotion and initial child lifecycle remain required; F01 stays partial.

HTMLProgressElement now exposes canonical value/max numeric reflection and readonly position over actual native/inert attributes. Shared original decimal-prefix scanning supplies HTML ASCII-whitespace, finite-result and positive-zero parsing without changing ECMAScript number grammar. Missing value is indeterminate, invalid/negative values fall back to zero, current values clamp to the actual positive maximum, and nonpositive max assignments leave attributes unchanged. Finite-double conversion runs once before the shared attribute transaction, preserving exact Attr identities, observer old values, author effects, concrete brands, captured origin and bounded refusal/recovery. Inert clone/adoption, actual App pointer/attribute-driven CSS paint/editor/link checks and scoped production evidence retain existing labels and control authority. Native progress-bar painting, broader form collections/lifecycle, identity-preserving main adoption and initial child lifecycle remain required; F01 stays partial.

HTMLMeterElement now exposes six canonical numeric properties over actual native/inert attributes. Shared HTML floating-point parsing supplies finite defaults and ordered minimum/maximum, actual-value, low/high and optimum clamping; midpoint calculation remains finite across extreme ranges. Setters retain finite unclamped attribute values after one numeric conversion through the shared attribute transaction. Exact Attr identities and UTF-16 values, observer old values, concrete brands, captured origin, author effects and bounded refusal/recovery retain native authority. Inert clone/adoption, actual App pointer/attribute-driven CSS paint/editor/link checks and scoped production evidence retain existing labels and control state. Native gauge painting, broader form collections/lifecycle, identity-preserving main adoption and initial child lifecycle remain required; F01 stays partial.

Passive progress/meter cloning and main-document import now share the checked ordinary-element transaction. Shallow and deep copies retain exact actual attributes, independent Attr identities, fallback children and canonical numeric metadata; connected fallback text replacement retains real child/list/observer behavior. Existing dynamic connection retains source identities, labels and current metadata through shared promotion. Template parking, captured-origin checks and bounded work/copy/storage/node/deadline refusal/recovery preserve native state. Actual App pointer/attribute-driven CSS paint/editor/link evidence stays scoped; active-content, unrelated raw-name and native-control guards remain enforced. Native gauge painting, identity-preserving main adoption, broader form collections and initial child lifecycle remain required; F01 stays partial.

HTMLTimeElement now exposes a canonical dateTime DOMString accessor over actual native/inert datetime attributes. Empty defaults, exact UTF-16 values, once-only author conversion, concrete brands, current Attr identity, namespace distinctions and observer old values use shared bounded attribute transactions. Refused reads and writes preserve native state; captured-origin checks precede conversion. Ordinary clone/import and identity-preserving native adoption retain actual metadata and child handles. Actual App pointer actions update attribute-driven CSS paint while preserving fallback text, editor selection and link hits; scoped production probes exercise the built executable. Date parsing and visible-text formatting are outside this reflection behavior; broader HTML interfaces and initial child lifecycle remain required, and F01 stays partial.

HTMLDataElement now exposes a canonical value DOMString accessor over actual native/inert attributes. Exact UTF-16 values, empty defaults, once-only conversion, current Attr identity and observer old values share the bounded attribute transaction without creating form-control state. Checked ordinary-element clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve native records and existing active-content, namespace, raw-name and origin guards. Actual App pointer actions update attribute-driven CSS paint while preserving fallback text, editor selection and link hits; scoped production probes exercise the built executable. Broader HTML interfaces and initial child lifecycle remain required; F01 stays partial.

HTMLQuoteElement now provides one canonical constructor and prototype for both q and blockquote in each owner realm, with exact HTML namespace/local-name brands and ordinary native/inert element lifecycle. Clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement preserve actual attributes and children. Shared interface aliases allocate one constructor callable per owner; NamedNodeMap.getNamedItem uses the existing deferred data-method mechanism without changing descriptors, identity, arity or brands. Work/copy/storage/node/deadline refusal and recovery preserve existing origin, raw-name and active-content guards. Actual App pointer actions verify attribute-driven CSS paint, editor selection and link hits; scoped production probes exercise the built executable. Quotation cite URL reflection, user-agent quotation rendering, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHeadingElement now provides one canonical constructor and prototype shared by h1 through h6 in each owner realm. Exact HTML namespace/local-name brands preserve each rank through native/inert clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement. All six aliases allocate one constructor callable per owner and retain existing exact raw-name attributes and live name queries. NamedNodeMap.getNamedItemNS uses the existing deferred data-method mechanism while preserving namespace matching, ordinary descriptors, stable identity, arity and receiver checks. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions verify heading attribute-driven CSS paint, editor selection and link hits; scoped production probes exercise the built executable. Document outline/accessibility integration, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHeadingElement now exposes its required align DOMString accessor over actual native/inert attributes for all six heading ranks. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual align metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve existing origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, outline/accessibility integration, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDivElement now exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Concrete DIV and heading accessor brands remain distinct. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback text replacement retain actual align metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve existing origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLegendElement now exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Concrete LEGEND, DIV and heading accessor brands remain distinct. Inert cloning preserves independent attributes; alignment writes preserve actual fieldset ownership and first-legend control state. Work/copy/storage/string/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native LEGEND connection lifecycle, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

Document.importNode now imports HTML LEGEND sources into inert documents through the existing bounded snapshot transaction. Native, pure inert and parked parsed sources retain exact attributes, independent Attr/child identities and the actual destination owner. Imported form metadata follows the destination direct parent; source fieldset ownership and native control state remain unchanged. Dictionary conversion and saved-child origin checks precede snapshot publication. Work/copy/storage/deadline refusal and no-slot recovery preserve source records and publish no partial identities. Other native control/resource families and focused native sources keep their existing guards. Actual App pointer actions use imported metadata to update real selector paint while preserving editor selection and link hits; scoped production probes exercise the built executable. Native LEGEND connection, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

Native HTML LEGEND cloneNode and main-document import now publish detached copies through the existing bounded copy transaction. Native shallow/deep and inert/parked imports retain exact metadata, independent Attr/child identities and the actual main-document owner. Copied form metadata starts null while original fieldset ownership and first-legend native control state remain unchanged. Work/copy/storage/node/deadline refusal preserves source records and commits with fresh-job recovery. Active handler/resource guards and the existing native connection/inert promotion boundaries remain enforced; the shared ordinary-element admission is unchanged. Actual App pointer actions exercise both native clones and main imports through real attribute-selector paint while preserving editor selection and link hits; scoped production probes exercise the built executable. LEGEND connection, template transfer, align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLParagraphElement now provides the canonical owner-realm interface for exact HTML p elements and exposes its required align DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Paragraph, DIV, heading and LEGEND accessor brands remain distinct. The existing EventTarget.removeEventListener data method uses bounded deferred callable allocation while retaining stable identity, descriptors, receiver checks and actual listener removal. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update align-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native align presentation hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLUListElement now exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. UL type, paragraph align and native control type authority remain distinct. Existing ordinary clone/import, identity-preserving adoption, dynamic connection, template parking and fallback replacement retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLUListElement now exposes its required compact boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without invoking author conversion hooks; current Attr identity, exact UTF-16 backing, observer old values and independent type metadata use the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access. Getter work charges attribute-name scanning without string-copy or retained growth. Existing clone/import, identity-preserving adoption, dynamic connection and template parking retain actual attributes and children. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLSpanElement now provides the canonical owner-realm interface for exact HTML span elements across native and inert creation. Constructor/prototype descriptors, saved bindings and concrete native identity share the existing HTML interface registry. Ordinary clones and imports preserve independent actual attributes and children; supported adoption, reserved-slot connection and shallow native template parking preserve actual owners and records. Deep native template parking and native host prototype mutation retain their existing guards. Work/copy/storage/node/deadline refusal and recovery preserve origin, namespace and active-content boundaries. The existing EventTarget.dispatchEvent data method uses bounded deferred callable allocation while retaining frozen data descriptors, stable identity, receiver checks and real cancelable dispatch. Actual App pointer actions update span attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDListElement now provides the canonical owner-realm interface for exact HTML dl elements and exposes its required compact boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without invoking author conversion hooks; current Attr identity, exact UTF-16 backing, observer old values and independent metadata share the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access. Getter work charges attribute-name scanning without string-copy or retained growth. The existing EventTarget.addEventListener data method uses bounded deferred callable allocation while preserving frozen descriptors, stable identity, concrete receiver checks and actual listener registration/removal. Existing clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual owners, attributes and children. Work/copy/storage/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Deep native template parking, native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLIElement now provides the canonical owner-realm interface for exact HTML li elements and exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, exact UTF-16, unrestricted values, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. List-item, UL and native control type authority remain distinct. The existing Node.contains data method uses bounded deferred callable allocation while preserving frozen descriptors, stable identity, concrete receiver/argument checks and real inclusive descendant queries. Existing ordinary clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual metadata and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin, namespace and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLLIElement now exposes its required value signed-long accessor over actual native/inert attributes. Reads use allocation-free HTML integer-prefix parsing, returning zero for absent, invalid or out-of-range values without rewriting the source. Assignments perform once-only number conversion and reuse shared 32-bit wrapping before writing the shortest decimal integer through the bounded attribute transaction. Native type and form-control value authority remain distinct. Current Attr identity, exact raw UTF-16 backing, observer old values, independent clone/import state and supported adoption, connection and shallow template parking retain real metadata and owners. Paid attribute-name/payload scans, conversion ordering, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update value-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now provides the canonical owner-realm interface for exact HTML ol elements and exposes its required type DOMString accessor over actual native/inert attributes. Empty defaults, unrestricted strings, exact UTF-16 backing, ordinary null conversion, current Attr identity, observer old values and once-only conversion use the bounded attribute transaction. Ordered-list, unordered-list, list-item and native control type authority remain distinct. Existing Node.hasChildNodes, getRootNode and isSameNode data methods use bounded deferred callable allocation while retaining frozen descriptors, stable identity, concrete brands and actual tree/identity queries. Existing clone/import, identity-preserving adoption, reserved-slot connection and shallow native template parking retain actual metadata, owners and children. Work/copy/storage/string/node/deadline refusal and recovery preserve namespace, origin and active-content guards. Actual App pointer actions update type-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its required reversed boolean accessor over actual native/inert attribute presence. Boolean conversion sets an empty null-namespace attribute or removes it without author conversion hooks. Current Attr identity, exact UTF-16 records, observer old values and independent clone/import metadata use the bounded attribute transaction. Exact namespace/case and concrete receiver checks precede access; getters charge attribute-name scanning without string-copy or retained growth. Supported adoption, reserved-slot connection and shallow native template parking retain real owners, attributes and children. Work/copy/storage/string/node/deadline refusal and recovery preserve origin and active-content guards. Actual App pointer actions update reversed-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its required start signed-long accessor over actual native/inert attributes. Reads reuse allocation-free HTML integer-prefix parsing and return one for absent, invalid or out-of-range content without rewriting it. Assignments perform once-only number conversion and shared signed 32-bit wrapping before the shortest decimal write through the bounded attribute transaction. The reflected default remains one with reversed attribute presence; actual list numbering remains a separate presentation requirement. Current Attr identity, raw UTF-16 backing, observer old values, independent clone/import state and supported adoption, connection and shallow template parking retain actual metadata and owners. Paid name/payload scans, concrete brands, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update start-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLOListElement now exposes its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks, preserving current Attr identity, exact UTF-16 backing and observer old values through the bounded transaction. Ordered-list type, start and reversed retain their independent actual attributes. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter name scanning needs no string copy or retained growth. Supported clone/import, adoption, reserved-slot connection and shallow native template parking retain real owners, metadata and children. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLPreElement now provides the canonical owner-realm interface for exact HTML pre elements and exposes its legacy width signed-long accessor over actual native/inert attributes. Reads reuse allocation-free HTML integer-prefix parsing with zero defaults for absent, invalid or out-of-range values without rewriting content. Assignments perform once-only number conversion and shared signed 32-bit wrapping through the bounded attribute transaction. PRE width and native image/canvas/control width authority remain distinct. Current Attr identity, raw UTF-16 backing, observer old values and independent clone/import metadata retain actual owners and children through supported adoption, connection and shallow native template parking. Paid name/payload scans, concrete brands, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving text, editor selection and link hits; scoped production probes exercise the built executable. Native preformatted presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

The listing and xmp HTML local names now share the existing HTMLPreElement owner-realm interface with pre. Exact factories, saved constructors and aliases retain actual local names while using the same canonical width accessor and signed-long conversion. Actual Attr handles, observer old values, independent copies, supported adoption/imports, text descendants and shallow native template parking preserve metadata and owners. Work/copy/storage/string/node/deadline refusal retains real state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise both aliases in the built executable. Native preformatted presentation, initial child lifecycle, checked parser coverage and broader HTML interfaces remain required; F01 stays partial.

HTMLUnknownElement now supplies the canonical nonconstructible interface for the fixed historical HTML names applet, bgsound, blink, isindex, keygen, multicol, nextid and spacer. Native and inert factories retain actual names, namespaces and owners while sharing the proper HTMLElement prototype chain. Saved constructors, ordinary inherited attribute handles and observation, concrete brands and captured-origin refusal retain real identity and authority. The interface does not admit unsupported native resource copies. Actual App pointer actions update class-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise all eight names in the built executable. Native presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

The fixed historical HTML names acronym, basefont, big, center, nobr, noembed, noframes, plaintext, rb, rtc, strike and tt now expose HTMLElement directly. Exact native and inert HTML names retain the canonical prototype, inherited actual attributes and namespace identity without acquiring another concrete interface. Saved globals and captured-origin access preserve real authority; author prototypes do not grant native brands. An actual center App fixture verifies pointer-driven class-attribute paint, neighboring editor selection and link hits, while built-executable probes cover all twelve factory names. This interface selection does not implement obsolete presentation, parser recovery or initial child lifecycles; broader foundation acceptance remains required, with F01 partial.

Standard HTML elements whose interface is HTMLElement now expose that canonical prototype directly: abbr, address, article, aside, b, bdi, bdo, cite, code, dd, dfn, dt, em, figcaption, figure, footer, header, hgroup, i, kbd, main, mark, nav, noscript, rp, rt, ruby, s, samp, search, section, small, strong, sub, summary, sup, u, var, wbr. Exact native and inert HTML factories preserve names, namespaces, owners and inherited actual attributes; unrelated concrete interfaces retain their brands. Saved constructors and captured-origin access preserve native authority, and author prototypes grant none. Concrete HTML classification now requires the HTML namespace; an explicit native SVG fixture rejects HTMLFontElement color access before author conversion. SVG factory probes retain SVG identity. Parser namespace admission remains a separate requirement. An actual section App fixture covers pointer-driven class-attribute paint, neighboring editor selection and link hits; built-executable probes cover each factory name. This interface selection does not complete presentation, parsing or child lifecycles for those elements. Broader foundation acceptance remains required; F01 stays partial.

HTML names without an applicable built-in interface now use the canonical nonconstructible HTMLUnknownElement interface. Exact native and inert factories preserve names, namespaces, owners and actual inherited attributes. Valid custom names retain HTMLElement; uppercase HTML-namespace local names and the reserved hyphenated names follow exact-name fallback. Known specialized names with partial interfaces, including frame, frameset and marquee, keep their existing native routing. Foreign namespaces, real brands, saved constructors, captured-origin refusal and unsupported native-copy boundaries remain intact. An actual mystery-element App fixture covers pointer-driven class-attribute paint, neighboring editor selection and link hits; scoped built-executable probes cover fallback and namespace distinctions. Presentation, parsing, child lifecycles and the remaining concrete interfaces are still partial; F01 remains partial.

HTMLModElement now supplies one canonical interface for ins and del, with a dateTime DOMString accessor over their actual datetime attributes. Native and inert values retain exact UTF-16, null-namespace identity, attached Attr aliases, observer old values and once-only conversion. Real brands and captured-origin checks precede author conversion; work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Ordinary bounded text, clone/import and identity-preserving adoption retain real metadata and child handles. Native App pointer actions for both elements verify attribute-driven paint with preserved fallback text, editor selection and link hits; scoped executable probes cover each alias and the shared interface. Node.normalize uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical method behavior. The cite URL reflector and its shared URL/document-encoding dependency, initial child lifecycle and broader foundation acceptance remain required; F01 stays partial.

HTMLMapElement now exposes a canonical name DOMString accessor over actual map attributes. Exact native/inert UTF-16 values, attached Attr aliases, null-namespace distinctions, once-only conversion and observer old values use bounded attribute transactions. Real brands and captured-origin checks precede author conversion; refusal preserves state with fresh-job recovery. Ordinary bounded text, clone/import and identity-preserving adoption retain native metadata and child handles. Live collection named lookup observes actual changed map names without replacing lone surrogates. Actual App pointer actions verify attribute-driven paint and preserved fallback text, editor selection and link hits; scoped executable probes cover the built source. Node.cloneNode uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical method behavior. Image-map association and interaction, initial child lifecycle and broader foundation acceptance remain required; F01 stays partial.

HTMLMapElement.areas now exposes a canonical readonly SameObject HTMLCollection over actual descendant HTML area elements. Native and inert roots preserve tree order and live insertion, movement, removal and named lookup, including exact UTF-16 native IDs and inert names. Namespace filtering excludes foreign and uppercase HTML local names. The same collection follows inert adoption and retained node identities. Canonical descriptors, author overrides and saved constructors preserve real authority; captured-origin checks precede cached access. Charged first-touch filter creation refuses work, copy, string, retained-storage and deadline exhaustion without publishing a collection, while cached reads need no new filter allocation. Actual App pointer actions mutate native area descendants and verify the live collection through attribute-driven paint, preserved text, editor selection and link hits; scoped executable probes cover the built source. Existing unsupported native area-copy and name-write boundaries remain intact. Image-map association, geometry and interaction, native area lifecycle expansion, initial child lifecycles and broader foundation acceptance remain required; F01 stays partial.

HTMLAreaElement now has a canonical nonconstructible interface and alt DOMString accessor over actual native and inert attributes. Exact UTF-16 values, once-only conversion, attached Attr aliases, observer old values and null-namespace distinctions share bounded attribute transactions. Concrete brands and captured-origin checks precede conversion; work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Inert cloning preserves independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use alt readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Node.insertBefore uses deferred ordinary-method storage to preserve the existing small-profile child-realm limit and canonical mutation behavior. Other area attributes, image-map association, geometry and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.coords now reflects the actual native and inert coords attribute as a plain DOMString, preserving exact UTF-16 units and the original spelling without coordinate parsing or normalization. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use coords readback for adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Other area attributes, coordinate interpretation, image-map association and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.shape now reflects the actual native and inert shape attribute as a plain DOMString. Original spelling, unknown values and exact UTF-16 units remain metadata rather than interpreted image regions. Canonical descriptors, real brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use shape readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Other area attributes, image-map region interpretation, association and activation, native lifecycle expansion and initial child lifecycles remain required; F01 stays partial.

HTMLAreaElement.download now reflects the actual native and inert download attribute as a DOMString, preserving the original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use download readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Download selection, requests, file handling and activation remain separate unfinished consumers, alongside other area attributes, image-map association, native lifecycle expansion and initial child lifecycles; F01 stays partial.

HTMLAreaElement.rel now reflects the actual native and inert rel attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy and unrelated raw-attribute refusals remain intact. Actual App pointer actions use rel readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. The relList token list, link-relation processing, other area attributes, image-map association, native lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.ping now reflects native and inert ping metadata through a USVString boundary. Property assignment replaces unpaired surrogates while preserving valid pairs; property readback normalizes raw attribute units without rewriting the underlying Attr or emitting records. Raw attribute APIs retain exact DOMString values. Canonical descriptors, concrete brands, once-only conversion, namespace distinctions, observer old values and captured-origin checks use bounded attribute transactions with atomic refusal and fresh-job recovery. Actual App pointer checks preserve adjacent map paint, fallback text, editor selection and link hits; scoped executable probes cover the built source. This does not implement hyperlink auditing, ping requests, URL-list processing, image-map association, remaining area consumers or native area lifecycle expansion; F01 stays partial.

HTMLAreaElement.relList now exposes a SameObject DOMTokenList over the actual native and inert rel attribute, with independent typed handles and the existing exact token operations. Ordinary PutForwards reads the current author-visible relList before forwarding the original value to its value property; concrete brands and captured-origin checks precede author effects. Attached Attr aliases, exact UTF-16 token identity, live iteration, namespace distinctions, inert clone/adoption isolation, mutation records and bounded refusal/recovery use the existing attribute transactions. Actual App pointer checks preserve adjacent map paint, fallback text, editor selection and link hits; scoped executable probes cover the built source. The supported-token set is empty because area link-relation processing is unfinished: noreferrer, noopener and opener are not advertised before their processing models exist. Inherited native area-instance enumeration remains refused. Image-map association, other area consumers and native area lifecycle expansion remain unfinished; F01 stays partial.

HTMLAreaElement.target now reflects the actual native and inert target attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use target readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Browsing-context selection, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.hreflang now reflects the actual native and inert hreflang attribute as a DOMString, preserving original spelling and exact UTF-16 units as language metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use hreflang readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Language selection, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.type now reflects the actual native and inert type attribute as a DOMString, preserving original spelling and exact UTF-16 units as metadata. Canonical descriptors, concrete brands, once-only conversion, attached Attr aliases, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions. Work, copy, string, retained-storage and deadline refusal leave records unchanged with fresh-job recovery. Inert clones retain independent metadata; active-handler copy, inherited-enumeration and unrelated raw-attribute refusals remain intact. Actual App pointer actions use type readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Resource-type processing, area hyperlink activation, image-map association, remaining URL and policy consumers, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.referrerPolicy now reflects the actual native and inert referrerpolicy attribute with ASCII-insensitive known-value matching and canonical lowercase readback. Missing, invalid, whitespace-padded and unpaired-surrogate values read as the empty string while the raw DOMString and attached Attr spelling remain exact. Canonical descriptors, concrete brands, once-only conversion, namespace distinctions, observer old values and captured-origin checks share bounded attribute transactions; reading never rewrites the source or emits records. Work, copy, string, retained-storage and deadline refusal preserve state with fresh-job recovery. Inert clones retain independent metadata; existing native area lifecycle and inherited-enumeration refusals remain intact. Actual App pointer actions use policy readback to update adjacent map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Request policy selection, Referer header processing, area hyperlink activation, image-map association, remaining URL consumers and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.href now exposes a canonical native and inert USVString accessor over actual href attributes using the shared original URL parser and current document base. IDL writes replace unpaired surrogates; raw attribute producers preserve exact DOMString records, while reads serialize the projected URL without changing source spelling or emitting records. Area and Anchor descriptors retain separate concrete brands, captured-origin checks and once-only conversion. Work, copy, string, retained-storage and deadline refusal remain atomic with fresh-job recovery. Inert clones retain independent URL metadata; active-handler copy and initial child lifecycle refusals remain intact. Actual App pointer actions update neighboring map paint from href readback while preserving fallback text, editor selection and link hits; executable probes cover the built source. Hyperlink activation, image-map association, request policy processing and native area lifecycle expansion remain unfinished; F01 stays partial.

HTMLAreaElement URL components now share the original HTTP(S) hyperlink parser and current document base: readonly origin plus protocol, username, password, host, hostname, port, pathname, search and hash. Canonical Area accessors retain concrete brands separate from Anchor descriptors, captured-origin checks and once-only USVString conversion. Component writes serialize into the actual href attribute with stable Attr identity and observer old values; invalid and missing URL setters retain the shared no-op rules; non-HTTP(S) records retain explicit atomic capability refusals. Work, copy, string, retained-storage and deadline refusals remain atomic with fresh-job recovery, and reads do not mutate source records. Native and inert paths share these bounds. Actual App pointer actions update neighboring map paint from component writes while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Non-HTTP(S) URL records, hyperlink activation, image-map association, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement and HTMLAnchorElement now expose canonical native toString methods over their internal href getters. Direct calls, String conversion and ordinary concatenation use current native/inert attribute and base authority without consulting an author href getter. The two methods retain separate concrete brands, per-realm identity and captured-origin checks; ordinary own-method, prototype and Symbol.toPrimitive overrides retain their normal precedence. Work, copy, string and deadline refusals preserve source records with fresh-job recovery, and reads require no retained-storage growth. Actual App pointer actions use stringification readback to update neighboring map paint while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. Existing URL parser capability limits remain explicit, including non-HTTP(S) records. Hyperlink activation, image-map association, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement.noHref now exposes the legacy boolean reflector over actual native and inert nohref attributes. Reads test exact unnamespaced attribute presence without copying attribute values; truthy writes retain or create the actual empty attribute and falsey writes remove it. Boolean conversion invokes no author string or primitive hooks. Canonical camel-case descriptors, concrete Area brands, current Attr identity, observer old values, raw UTF-16 aliases, exact namespace and captured-origin checks share bounded attribute transactions. Work, copy, storage and deadline refusals preserve state with fresh-job recovery; presence reads need no string-copy or retained-storage growth. Actual App pointer actions update adjacent map paint from boolean readback while preserving fallback text, editor selection and link hits; scoped executable probes cover the built source. This obsolete API reflects metadata only. Image-map association, hyperlink activation, request policy processing, native area lifecycle expansion and initial child lifecycles remain unfinished; F01 stays partial.

HTMLAreaElement now supports detached native cloneNode and inert-to-main importNode through the shared bounded copy planner. Copies retain independent attributes, exact supported UTF-16 metadata, live token-list identity, destination-document URL authority and copied nonce-unsafety state. Deep copies preserve actual text/comment topology and owners without connecting the result. Active handlers and unsupported imports retain explicit refusals; existing unrelated raw-attribute guards remain intact. Work, copy, retained-storage, node and deadline refusals publish no partial nodes or source changes and recover with fresh allowances. Saved child-realm methods enforce captured origin before copying. Actual App pointer actions use detached-copy readback to update adjacent map paint while preserving source attributes, fallback text, editor selection and link hits; scoped executable probes cover the built source. This admission does not broaden native area connection/adoption or create image-map association, hyperlink activation or request-policy behavior. Initial child lifecycles and remaining area consumers stay unfinished; F01 remains partial.

HTMLAreaElement now uses reserved native identities for supported author-created inert leaves, inert clones and parsed template leaves. Direct main-document insertion and template-fragment round trips retain the same node, Attr and token-list handles while rebinding the actual owner and resolving href against the current destination. Native detached leaf parking preserves the concrete interface and real Map.areas order on reconnection. Existing active-handler, namespace and resource admission remains checked before activation. Work, copy, storage and deadline refusals preserve trees, owners and observations with fresh-job recovery; saved child-realm methods recheck captured origin. Actual App pointer checks cover create, append, reorder and removal through live collections and adjacent paint, editor selection and link hits. Completed template leaf parking now advances the shared DOM commit generation so ordinary and Area leaves publish genuine child-list observer records. Native main-document author-created connection was already supported and remains a control. Image-map geometry, area activation/navigation, request policy and initial child lifecycles remain separate; F01 stays partial.

HTMLParamElement now exposes a canonical name DOMString accessor over actual parsed, native-created and author-created inert attributes. Exact UTF-16 values, attached Attr aliases, null-namespace distinctions, once-only conversion and observer old values use bounded shared transactions. Genuine brands and captured-origin checks precede author conversion; work, copy, storage and deadline refusals preserve records with fresh-job recovery. Its canonical prototype is registered in each owner realm, while the constructor data identity is allocated on observation through charged native metadata; prototype/global identity, descriptors, mutation and deletion retain ordinary behavior without changing the frozen small-profile limits. Actual App pointer actions cover IDL, Attr and removal changes with adjacent paint, editor selection and link hits, and scoped executable probes cover the built source. Broader Param copy/connection lifecycles and embedded-object behavior remain separate; F01 stays partial.

HTMLParamElement now reflects value, type and valueType as unrestricted DOMStrings over the actual value, type and lowercase valuetype attributes. One shared metadata path preserves exact UTF-16, attached Attr identity, observer old values, inert owners, exact namespace distinctions and once-only conversion. Canonical accessor brands and captured origin precede conversion; ignored extra arguments, missing/undefined/null values and abrupt conversion retain Web IDL behavior. Work, copy, storage and deadline refusals preserve existing records and recover on a fresh job. Actual App pointer actions cover all three fields through IDL, Attr and removal changes with adjacent paint, editor selection and link hits; scoped executable probes cover the built source. Broader Param copy/connection lifecycles and embedded-object behavior remain separate; F01 stays partial.

HTMLParamElement now supports detached native cloneNode and main-document import of supported inert Param trees through the shared bounded copy planner. All four reflected metadata fields retain exact UTF-16 with independent Attr records and correct destination owners; supported deep text/comment topology remains independent. The planner preserves nonce-unsafety flags without source mutation, rejects active handlers and unsupported foreign import metadata before publication, and retains work, copy, storage, node and deadline refusal with fresh-job recovery. Saved child-realm copy methods recheck captured origin. Actual App pointer checks compare detached copies and original attributes while preserving adjacent paint, editor selection and link hits; scoped executable probes cover the built source. Broader Param connection/adoption and non-leaf template parking, other copy receivers and embedded-object behavior remain separate; F01 stays partial.

HTMLParamElement now supports checked identity-preserving inert-to-main connection and native leaf parking in template content. Append, insert, reorder, remove and fragment round trips retain all four exact metadata fields, saved Attr identities and destination owners. Existing active-handler, namespace, origin, work, copy, retained-storage and deadline guards apply before publication, with fresh-job recovery and real child-list records. Actual App pointer checks exercise native-created, inert-created and parsed-template routes while preserving adjacent paint, editor selection and link hits; scoped executable probes cover the built source. Broader Param adoption and non-leaf parking, other receivers and embedded-object behavior remain separate; F01 stays partial.

HTMLDataListElement now provides the canonical owner-realm interface for exact HTML datalist elements across parsed, native-created and inert namespace factories. Its native prototype, constructor data identity, descriptors, inherited attributes and class tag retain real node identity; author prototypes and foreign or differently cased local names do not gain the native brand. Constructors remain illegal for direct author construction. Its canonical prototype is installed eagerly; the constructor data value is allocated on observation through charged native metadata, preserving the unchanged small-profile function limit. Captured-origin checks protect inherited operations, and saved constructors remain authoritative after global mutation or deletion. Actual App pointer actions inspect the native datalist and option content while preserving adjacent paint, editor selection and link hits; scoped production probes exercise the built executable. Input list association and suggestion UI, broader copy/connection lifecycles and initial child behavior remain separate; F01 stays partial.

HTMLDataListElement.options now exposes a readonly SameObject live HTMLCollection of all descendant HTML option elements, including disabled options. Native and inert roots retain exact namespace filtering, tree order, current ID/name lookup and actual node identity through supported mutations and inert adoption. The canonical getter preserves author overrides and saved interface identity while enforcing concrete receiver brands, captured origins and bounded atomic refusal before publishing a new filter. Actual App pointer checks and built-executable probes inspect real option membership and preserve adjacent paint, editor and link controls. Input list association, suggestion UI, broader option copy/connection lifecycles and initial child behavior remain separate; F01 stays partial.

HTMLDataListElement now supports detached native shallow copies, deep copies of ordinary text/element/comment descendants, and checked inert-to-main imports. Copies preserve independent exact attributes, Attr owners, real ordinary topology, canonical interfaces and separate live options collections. Existing option-transfer guards remain in force for native deep copies and inert imports with option descendants. Origin, handler, foreign-attribute and resource refusals preserve the source and publish no partial copy, with fresh-job recovery and actual App pointer controls. Native deep cloning and inert import of option descendants, datalist connection/template parking, input association and suggestion UI remain separate; F01 stays partial.

HTMLDataListElement now supports checked identity-preserving leaf connection and template parking. Native-created, inert-authored and parsed-template leaves preserve actual node and Attr identities, owner documents and the SameObject options collection across insertion, reordering, removal and checked fragment roundtrips. Real observer records, origin checks, bounded atomic refusal and fresh-job recovery retain existing handler, namespace and non-leaf parking guards. Actual App pointer actions exercise native, inert and parsed-template routes while preserving adjacent paint, editor selection and link hits. Non-leaf datalist transfer, option lifecycles, input list association, suggestion UI and initial child behavior remain separate; F01 stays partial.

HTMLOptionElement now provides the canonical owner-realm interface for exact HTML option elements across parsed SELECT and datalist trees, native creation and inert namespace factories. Native prototypes, constructor data identity, descriptors, inherited attributes and class tags retain real node identity; author prototypes and foreign or differently cased names do not gain the native brand. The prototype is installed eagerly and the constructor data value is allocated on observation through charged native metadata, preserving the unchanged small-profile function limit. Captured-origin checks protect inherited operations, and saved constructors remain authoritative after global mutation or deletion. Actual App pointer controls inspect datalist option content while preserving adjacent paint, editor selection and link hits. The legacy Option factory, broader option IDL and transfer lifecycles, input association and suggestion UI remain separate; F01 stays partial.

HTMLOptionElement.disabled now provides the canonical boolean accessor over the actual null-namespace disabled attribute on native and inert HTML options. It reflects own attribute presence, uses boolean truthiness without author string conversion, preserves live Attr identity and observer old values, and retains exact raw attribute backing through supported aliases and inert copies. Native SELECT option disabled state updates through the existing checked attribute transaction; datalist options gain no fabricated control state. Concrete brands, captured origins, bounded getter work and atomic setter refusal retain fresh-job recovery and actual App pointer controls. Other option IDL, ancestor disabledness, the legacy Option factory and broader transfer lifecycles remain separate; F01 stays partial.

HTMLOptionElement.form now exposes a canonical readonly accessor for the qualifying ancestor SELECT element's actual form owner on native and inert HTML options. The checked ancestry walk stops at HTML datalist, hr, option and repeated optgroup boundaries, preserves namespace distinctions and ignores author property overrides. Existing native parser associations and supported inert explicit-owner changes retain exact ID behavior; standalone and datalist options return null. Captured-origin checks, bounded work, deadline refusal and fresh-job recovery preserve actual node and observer state. App pointer controls inspect parsed ancestor and explicit owners while retaining adjacent paint, editor selection and link hits. Native SELECT reassociation writes, other option IDL, the legacy Option factory and broader transfer lifecycles remain separate; F01 stays partial.

HTMLOptionElement.index now provides the canonical readonly position in the actual qualifying SELECT option list for native and inert HTML options. A shared checked ancestor walk preserves form-owner behavior, and bounded tree-order counting excludes nested SELECT, datalist, hr, option and repeated optgroup subtrees with exact namespace and case distinctions. Standalone and datalist options return zero; supported inert insertions and removals update the observed position without fabricated collection or selection state. Brand and captured-origin checks, charged work and temporary traversal storage, copying and deadline refusal preserve observer silence and fresh-job recovery. Actual App pointer controls inspect native grouped positions while retaining adjacent paint, editor selection and link hits. Other option IDL, the legacy Option factory, broader transfer lifecycles and selection algorithms remain separate; F01 stays partial.

HTMLOptGroupElement now provides the canonical owner-realm interface for exact HTML optgroup elements across parsed SELECT trees, native creation and inert namespace factories. Native prototype and constructor data identities, descriptors, inherited actual attributes and class tags preserve the genuine node brand; author prototypes and foreign or differently cased names gain no native authority. The prototype is eager while constructor observation uses charged deferred metadata and preserves the unchanged small-profile function limit. Captured-origin checks protect inherited operations, and saved constructors remain authoritative after global mutation or deletion. App pointer controls inspect actual grouped option content while preserving adjacent paint, editor selection and link hits. Group disabled and label reflection, broader selection algorithms, transfer lifecycles and initial child behavior remain separate; F01 stays partial.

HTMLOptGroupElement.disabled now provides the canonical boolean accessor over the actual null-namespace disabled attribute on native and inert HTML groups. It reflects own attribute presence, uses boolean truthiness without author conversion hooks, and preserves live Attr identity, raw attribute aliases and observer old values. Native SELECT descendant disabled state updates through the existing checked attribute transaction while each option retains its own distinct disabled flag; unrelated options and controls keep their actual state. Exact concrete brands, captured origins, bounded getter work and atomic setter refusal retain fresh-job recovery and actual App pointer controls. Group label reflection, broader selection algorithms and transfer lifecycles remain separate; F01 stays partial.

HTMLOptGroupElement.label now provides canonical DOMString reflection over the actual null-namespace label attribute on native and inert HTML groups. Exact UTF-16 values, once-only author conversion, conversion reentry, live Attr identity, raw aliases and observer old values use the shared checked attribute transaction. Native SELECT group rows update while option text, values, selection and unrelated controls retain their actual state. Concrete brands, captured origins, work, copying, storage and deadline guards preserve atomic refusal and fresh-job recovery, with actual App pointer, paint, editor and link checks. Broader group-label presentation algorithms and transfer lifecycles remain separate; F01 stays partial.

Detached OPTGROUP copies now preserve actual independent label and disabled attributes without requiring an unrelated native SELECT control plan. Native shallow cloneNode and native/inert document importNode retain exact UTF-16 records, real owner documents, distinct Attr identities and source isolation. Empty and passive descendant copies reuse the existing bounded copy runtime; OPTION descendants, active handlers, unsupported namespace transfers and native control lifecycles keep their independent admission checks. Work, copying, storage, node-count, origin, deadline and nonce provenance checks preserve atomic refusal and fresh-job recovery, with actual App pointer, paint, native SELECT source rows, editor and link evidence. Connection, adoption and broader option-state copying remain separate; F01 stays partial.

Passive HTML OPTGROUP subtrees now connect, reorder and detach through existing checked DOM tree transactions outside SELECT and other protected native contexts. Native creation, inert factories, factory-built template fragments and template parking retain real node and Attr identities, exact label/disabled backing, destination ownership, source isolation and observer records. Native SELECT rows, option values and selectedness remain unchanged by these ordinary connections. Work, copying, storage, origin and deadline refusals preserve atomic recovery, with actual native and inert App pointer, paint, editor and link checks. Mutation inside SELECT/OPTION, option-state transfer and SELECT/OPTION-owned child-text writes retain their separate lifecycle guards; F01 stays partial.

Checked HTML fragments now admit passive OPTGROUP elements outside SELECT, including balanced text/comment or passive-element children and omitted sibling group endings when the current group can close directly. Native container replacement and parsed template cloning/connection retain actual HTML interfaces, label and disabled records, Attr identity and destination ownership; exact UTF-16 labels assigned after parsing survive these routes. Resource and malformed-scope refusals preserve existing native and template trees, observer records and recovery, with actual App pointer, paint, editor and link checks. Connecting inert-only identities created without native parking capacity, intervening ancestor recovery, standalone OPTION subtrees, SELECT nested inside a standalone group and unpaired-surrogate markup remain unsupported; native SELECT/OPTION lifecycle guards remain separate. F01 and F05 stay partial.

Passive HTML OPTGROUP records outside SELECT now participate in checked Document.adoptNode transactions. Detached and connected groups, including passive text/comment/element descendants, keep the actual node, Attr, saved child-list and author-property identities while owner documents and removal records follow the transfer. Exact UTF-16 labels and disabled backing survive adoption, return and reconnection, with native and App pointer/paint/editor/link checks. Existing native control, SELECT/OPTION ancestor, resource-descendant, focus, form-ID, origin, work, copying, storage and deadline checks retain their atomic refusals and fresh recovery. Native option-state adoption and broader lifecycle integration remain separate; F01 stays partial.

Passive native HTML OPTGROUP content outside SELECT now supports checked textContent replacement and clearing, CharacterData writes, passive child insertion/reordering/removal and text-only innerHTML replacement. Real text and child identities, exact UTF-16 data, detached source isolation, canonical group metadata, Attr handles, saved child lists and observer records follow the ordinary transactions. Fresh ancestry validation keeps SELECT/OPTION-owned content protected; native-control/resource descendants, non-HTML groups, rich OPTGROUP fragment parsing and resource/deadline refusals retain their guards and atomic recovery. Actual App pointer, paint, editor and link checks cover visible content updates; native SELECT state remains unchanged. F01 and F05 stay partial.

Canonical HTMLOptionElement.label now returns the exact present attribute, including an empty value, and otherwise computes HTML-aware descendant text with ASCII whitespace normalization and exact UTF-16 preservation. The shared bounded text collector excludes HTML/SVG script content and HTML image subtrees. Label setters and attached Attr aliases update the real native option display and SELECT caption atomically, using text fallback for missing or empty display labels and the existing 256-character display bound. Values, selection, disabled state and group membership remain unchanged. Brand/origin checks, conversion reentry, observer identity, namespace isolation and work/copy/storage/deadline refusal and recovery are covered in native and inert trees. Actual App pointer, paint, editor and link checks cover visible option row updates. F01 stays partial; native option text/value mutation and broader lifecycle integration remain separate.

Canonical HTMLOptionElement.value now returns the exact present value attribute, including empty strings and raw UTF-16, or shared HTML-aware descendant text when absent. IDL setters, public attribute routes and attached Attr aliases atomically update native option values, selected control values and required-placeholder state. Unselected edits preserve the current control value, caption and selection; selected edits retain exact scalar control characters without emitting input/change events. Native values keep the existing 4096 UTF-16-unit bound and combined 128 KiB Forms admission, including both option and selected-editor growth. Brand/origin checks, conversion reentry, namespace and observer identity, resource refusal/recovery, native form submission and native reset are covered; the scripted form.reset() method remains separate. Actual App pointer, paint, editor and link checks cover native value updates. F01 stays partial; option text/selectedness mutation and broader lifecycle integration remain separate.

Canonical HTMLOptionElement.text now reads shared HTML-aware descendant text and replaces actual children with exact DOMString data. The native textContent alias uses the same checked replacement, retaining detached Text identities, saved child lists, observer records and live Range updates. Missing value attributes and missing/empty display labels drive prepared native fallback values, bounded labels, selected captions/editors and required-placeholder state; explicit attributes and unselected control state remain authoritative. Native 4096-unit value, 256-character display and combined 128 KiB Forms limits are checked before DOM publication, including simultaneous value/label/editor growth and scratch storage. Brand/origin, conversion reentry, raw UTF-16 and resource/cancellation refusal retain atomic recovery. Actual App pointer, paint, editor and link checks cover real text replacement. SELECT-wide replacement, native option CharacterData writes, rich option innerHTML and broader selectedness/lifecycle integration retain separate guards; F01 stays partial.

Canonical HTMLOptionElement.defaultSelected now reflects the exact null-namespace selected attribute as a boolean. Actual native single-select options retain independent default and dirty state: selected-attribute presence changes synchronize clean selections, captions and editors through a prepared transaction; script value/index choices and native user choices mark the chosen option dirty. Native form reset uses current defaults and clears dirtiness. Exact Attr/public/namespace producers share presence semantics, observer identity and old values, with work/copy/storage/cancellation refusal preserving native state and fresh-job recovery. Combined 128 KiB form payload and actual selection/attribute scratch are checked before publication. Actual App pointer checks retain native selection, caption, paint, editor and link behavior. Option.selected, inert selectedness exposure, multi-select and broader lifecycle integration remain separate; scripted form.reset remains unavailable and F01 stays partial.

Canonical HTMLSelectElement.disabled now reflects actual null-namespace disabled presence through its defining prototype accessor. Main and inert attributes, raw UTF-16 Attr aliases, namespace distinctions and boolean conversion share the existing checked attribute transaction. Native control refresh preserves option disabled flags, selected value/index, caption and default state; script selection remains allowed while disabled and no input/change event is fabricated. Own reflection remains separate from effective fieldset disability, including the first-legend exemption. Brand/origin and author overrides retain canonical behavior; work/copy/storage/cancellation refusal preserves attributes and native state with fresh-job recovery. Actual App pointer actions retain paint, native control state, editor and links. Multi-select, broader SELECT interface and lifecycle integration remain separate; F01 stays partial.

Canonical HTMLSelectElement.required now reflects actual null-namespace required presence through its defining prototype accessor. Exact main/inert boolean reflection, raw UTF-16 Attr aliases, namespace distinctions, observation and once-only boolean conversion use the shared checked attribute path. Native required state publishes with the attribute and feeds existing placeholder validation; disabled controls bypass validation, scripted selections retain their actual values and native reset keeps the current required attribute. Selection, option flags, captions and defaults remain unchanged by reflection and no input/change event is fabricated. Brand/origin and author overrides retain canonical behavior; work/copy/storage/cancellation refusal preserves native validation and observer state with fresh-job recovery. Actual App pointer checks retain the native required flag, paint, editor and links. Broader constraint-validation interfaces, multi-select and lifecycle integration remain separate; F01 stays partial.

Canonical HTMLSelectElement.name now reflects the exact null-namespace name through its defining prototype accessor. Native and inert DOMString reflection preserves raw UTF-16 Attr identity, aliases, observations, namespace distinctions and live named lookup. Once-only author conversion, reentry, abrupt completion, concrete brands, saved accessors and origin checks use shared transactions. A prepared native submission name publishes with the attribute under existing 8192-byte name and 128 KiB aggregate form limits; native serialization uses scalar encoding and reset retains the current name. Selection, option defaults, editor positions, captions and input/change events remain stable. Work/copy/storage/cancellation refusal is atomic with fresh-job recovery. Actual App pointer checks retain native names, paint, editor and links. Multi-select and broader lifecycle integration remain separate; F01 stays partial.

Canonical HTMLTextAreaElement.name now reflects the exact null-namespace name through its defining prototype accessor. Native and inert DOMString reflection preserves raw UTF-16 Attr identity, aliases, observations, namespace distinctions and live named lookup. Once-only author conversion, reentry, abrupt completion, concrete brands, saved accessors and origin checks use shared transactions. SELECT and TEXTAREA share prepared native submission-name admission under existing 8192-byte name and 128 KiB aggregate form limits, retaining other control defaults, option and group payloads. Native serialization uses scalar encoding and reset retains the current name. Dirty live editors, reset defaults, cursor positions, flags and input/change events remain stable. Work/copy/storage/cancellation refusal is atomic with fresh-job recovery. Actual App pointer checks retain native names, paint, editor and links. Inert textarea value editors and broader lifecycle integration remain separate; F01 stays partial.

Canonical HTMLTextAreaElement.disabled now reflects actual null-namespace disabled presence through its defining prototype accessor. Native and inert boolean reflection preserves exact raw UTF-16 Attr aliases, namespace distinctions, observation, author overrides and saved accessor authority without invoking author string conversion. The existing checked native disability refresh retains first-legend exemptions and inherited fieldset disability separately from own-attribute reflection. Native disabled controls bypass required validation and submission; script editing remains allowed and native reset retains the current disabled attribute. Dirty live editors, reset defaults, cursor positions, names, flags and input/change events remain intact. Work/copy/storage/cancellation refusal preserves native and DOM state with fresh-job recovery. Actual App pointer checks retain effective native disability, paint, editor and links. Broader textarea editing and lifecycle interfaces remain separate; F01 stays partial.

Canonical HTMLTextAreaElement.required now reflects actual null-namespace required presence through its defining prototype accessor. Exact native and inert boolean reflection retains raw UTF-16 Attr aliases, namespace distinctions, observation, saved accessors, author overrides and truthiness without author conversion. Native required state publishes only after checked attribute publication and feeds existing empty-value validation; read-only and disabled textareas bypass that validation. Native reset keeps the current required attribute while restoring the actual default and clearing dirty value state. Dirty live editors, default text, cursor positions, names, other flags and input/change events remain stable during reflection. Work/copy/storage/cancellation refusal preserves native validation, DOM and observer state with fresh-job recovery. Actual App pointer checks retain native required flags, paint, editor and links. Broader constraint-validation and textarea lifecycle interfaces remain separate; F01 stays partial.

Canonical HTMLTextAreaElement.readOnly now reflects actual null-namespace readonly presence through its defining prototype accessor, preserving the distinct JavaScript property and attribute names. Native and inert boolean reflection retains exact raw UTF-16 Attr aliases, namespace distinctions, observation, saved accessors, author overrides and truthiness without author conversion. Native read-only state publishes after checked attribute publication, feeds existing user-editing guards and bypasses required validation while retaining form submission and script editing. Native reset retains the current readonly attribute and restores the actual default. Dirty editors, defaults, cursor positions, names, other flags and events remain stable during reflection; work/copy/storage/cancellation refusal preserves native validation, DOM and observer state with fresh-job recovery. Actual App pointer checks retain flags, paint, editor and links; native pointer/keyboard/text-input checks exercise typing, paste and backspace refusal, re-enable editing and trusted input events. Broader textarea editing and lifecycle interfaces remain separate; F01 stays partial.

Canonical HTMLTextAreaElement.cols and rows now reflect exact null-namespace attributes as positive unsigned dimensions with defaults of 20 and 2. Reads use original HTML integer-prefix parsing without normalizing attributes; setters perform once-only numeric conversion, unsigned wrapping and default fallback for zero or values above 2147483647. Native and inert attributes retain raw UTF-16 prefixes, Attr identity, observer old values, exact namespaces, saved accessor authority and author overrides. Native textarea intrinsic and control sizing share the same original parser, retaining existing display caps, available-space limits and CSS dimension overrides. Dirty editors, defaults, cursor positions, names, flags and input/change events stay intact. Brand/origin and work/copy/storage/cancellation refusal preserve DOM and native state with fresh-job recovery. Actual App checks cover native size changes, pointer dispatch, paint, editor and link state. Broader textarea lifecycle and editing interfaces remain separate; F01 stays partial.

Canonical native HTMLTextAreaElement.textLength now reports the UTF-16 length of the actual LF-normalized API value through a read-only prototype accessor. Direct counting over the admitted editor avoids allocating or copying a value string, including at the existing native value bound. Dirty live values remain distinct from default text and submission wrapping; native reset to the admitted default, dirty copies, detach/reconnect and real user edits expose their actual lengths. Concrete brands, saved canonical accessors, author overrides and origin checks retain authority; work and cancellation refusal preserve DOM, editor, cursor, observer and retained state with fresh-job recovery. Actual App typing, multiline paste and backspace checks observe current lengths during trusted input events and preserve paint, neighbor editors and links. Inert textareas and initial child controls without admitted native value state explicitly refuse rather than fabricating a length. Broader textarea lifecycle and editing interfaces remain separate; F01 stays partial.

Canonical native HTMLTextAreaElement.defaultValue now reads actual child text and performs checked whole-child replacement through a writable prototype accessor. The same transaction serves textarea.textContent: scalar DOM text retains authored CR/LF while the admitted native reset default and pristine editor use normalized LF. Dirty live values, cursor and selection remain unchanged; actual native reset adopts the new default. Saved Text identities detach, live child lists and child-list observations reflect the committed replacement, and script default updates emit no input or change events. Brands and origin checks precede once-only conversion; reentrant and abrupt conversion preserve their actual effects. Native UTF-16, combined form payload, copy, work, retained storage, node and cancellation bounds refuse before DOM/native publication with fresh-job recovery. Actual App pointer-driven default changes, typing and reset check editor paint, trusted input, neighbor state and link hits. Inert and initial child textareas without admitted native default state, raw unpaired UTF-16 defaults and broader child/CharacterData mutation algorithms remain explicit boundaries. F01 stays partial.

Canonical native HTMLTextAreaElement.maxLength now reflects the actual null-namespace maxlength attribute through a writable prototype accessor. Missing, invalid or out-of-range content returns -1; accepted HTML integer prefixes use a nonnegative signed long. Setter values undergo once-only signed 32-bit conversion, with a real IndexSizeError DOMException for negative results. Checked canonical, public attribute, attached Attr and element Attr replacement writes publish the native UTF-16 edit limit only after the DOM transaction commits. Initial and fragment-created editors share this parsing; lowering the limit preserves existing live text, reset defaults, selection and dirty state while limiting future trusted typing and paste. The independent native capacity remains 4,096 UTF-16 units even when reflected metadata is larger. Native and inert reflection, saved accessor authority, namespace isolation, raw attribute preservation, observation, dirty copies, reconnection, resource refusal and fresh-job recovery have focused coverage. The App fixture checks trusted input events, actual editor paint, neighbor state and link hits. Initial child-window reflection and NamedNodeMap mutation remain unavailable; minlength and validity are separate boundaries. F01 stays partial.

Canonical native HTMLTextAreaElement.minLength now reflects the actual null-namespace minlength attribute through a writable prototype accessor. Missing, invalid or out-of-range content returns -1; accepted HTML integer prefixes use a nonnegative signed long. Setter values undergo once-only signed 32-bit conversion and negative results throw a real IndexSizeError DOMException. Native and inert content retain exact raw attribute backing; public setters, attached Attr updates and element Attr replacement use the checked attribute transaction with actual observations and identities. Minimum metadata preserves live text, defaults, selection, dirty state and the independent maximum edit limit, including after fragment creation, dirty copying and reconnection. Brands and captured origins precede author conversion; work, copy, storage and cancellation refusal preserve state with fresh-job recovery. The App fixture checks trusted typing and paste below a large minimum while the existing maximum still limits UTF-16 insertion, plus editor paint, input events, neighbor state and link hits. This increment supplies reflection only: tooShort validity, minlength submission enforcement, initial child-window reflection remain separate requirements. F01 stays partial.

Canonical native HTMLTextAreaElement.wrap now reflects exact DOMString content through a writable prototype accessor. Missing content reads as the empty string; authored case, unknown keywords and raw UTF-16 remain intact. Native form submission recognizes ASCII-case-insensitive hard without trimming; missing or invalid keywords retain soft submission. Hard wrapping counts Unicode scalar columns at the positive HTML cols width, with a default of 20 and safe handling of huge widths, and inserts line breaks only into submitted data. Existing line breaks, complete Unicode scalars and trailing line boundaries are preserved. API values, textLength, defaults, cursor, selection and dirty state remain unchanged. GET and URL-encoded POST use the same bounded CRLF and percent encoding, including request-size refusal and recovery. Canonical and Attr writes, exact namespaces, inert and copied metadata, dirty fragments, reconnection, conversion authority, observation, resource and captured-origin refusal have focused coverage. The App fixture checks trusted typing, mode changes, real native submission serialization, editor paint, input events, neighbor state and link hits; executable probes include received localhost POST bodies. Initial child-window reflection, minlength validity and broader form encodings remain separate requirements. F01 stays partial.

Canonical native HTMLTextAreaElement.autocomplete now exposes a writable prototype accessor with an exact authored DOMString setter and a processed HTML autofill hint getter. The expectation-mantle grammar accepts on/off, normal and contact field names, optional contact kind, shipping/billing and section identifiers, and the final webauthn credential token in their specified order. ASCII whitespace is collapsed and ASCII token letters are lowercased; unknown, misordered or missing hints read as the empty string. Containing-form autocomplete defaults do not change that IDL result. Raw UTF-16 section identifiers and exact null-namespace attribute identity are preserved. Native and genuine inert metadata, canonical authority, once-only conversion, attribute producers, observation, copies, reconnection, and resource refusal/recovery have focused coverage. The App fixture verifies trusted typing, unchanged native submission data, selection, dirty state, painting, neighbor state and link hits. Executable probes exercise the same metadata boundary. This increment does not provide autofill UI, stored user data, credential services or initial child-window reflection. F01 stays partial.

Canonical native HTMLSelectElement.autocomplete now exposes a writable prototype accessor with exact authored DOMString storage and the same processed HTML autofill hint getter as textarea. Both interfaces share one original bounded grammar and output builder, including exact raw UTF-16 section identifiers and ordinary empty-string results for invalid hints. Native and genuine inert attribute identity, all supported Attr producers, observation, concrete brands, saved accessor authority, once-only conversion, captured-origin refusal and resource recovery have focused coverage. Hint mutations preserve selected options, values, captions, names and native control state. Actual App pointer actions verify processed metadata, selected value and caption, neighbor editor selection, rendered pixel changes and link hits. Executable probes cover the native metadata boundary. Autofill UI, stored user data, credential services and initial child-window reflection remain separate requirements. F01 stays partial.

Canonical native HTMLInputElement.autocomplete now exposes a writable prototype accessor with exact authored DOMString storage and the shared processed HTML autofill hint getter. Hidden inputs use the anchor mantle: on/off hints read as the empty string while detail tokens retain the common scope/contact/credential grammar. The mantle comes from the exact null-namespace type state through a charged borrowed scan, without allocating a type-string copy or consulting author getters. Other input states use the expectation mantle. Metadata witnesses cover all 22 current input states while preserving actual native value, checked, dirty, caption and editor state; this does not establish complete behavior of those states. Native and genuine inert attribute identity, Attr writers, observation, canonical authority, once-only conversion, raw section identifiers, independent inert copies, captured-origin refusal and resource recovery have focused coverage. Actual App trusted typing, unchanged submission data, editor selection, painting, neighbor state and link hits remain covered. Executable probes exercise ordinary and hidden hint metadata. Autofill UI, stored user data, credential services and initial child-window reflection remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.autocomplete now reflects its exact authored DOMString through writable prototype accessors. The getter returns off only for the exact ASCII-case-insensitive off keyword, and on for missing or invalid values, without trimming or accepting the separate control hint-token grammar. Native and genuine inert attribute identity, raw UTF-16 storage, Attr writers, observer old values, once-only conversion, saved canonical authority, live named-control precedence, origin refusal and resource recovery have focused coverage. Changing form metadata preserves dirty editor values, selection, control hint IDL, submission data and actual App paint and link behavior. Autofill services, stored user data and initial child-window reflection remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.name now reflects exact authored DOMString metadata through writable prototype accessors. Missing names read as the empty string; whitespace, case and raw UTF-16 backing remain unchanged, and null and undefined use ordinary string conversion. Supported document named lookup tracks current scalar names while preserving form owners and control submission names; this does not complete HTMLFormControlsCollection or duplicate-name collection behavior. Native and genuine inert attributes, independent inert copies, Attr identity and observation, concrete brands, saved accessors under control-name masking, once-only conversion, origin refusal and bounded recovery have focused coverage. Actual App trusted editing, unchanged form data, editor selection, pixels and neighboring link hits remain covered. General native-form own descriptors, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.noValidate now reflects exact lowercase null-namespace novalidate attribute presence through a writable boolean accessor. Assignments use hook-free Boolean conversion: truthy values create an empty attribute and falsy values remove it. The existing native submission path reads that actual presence to bypass its supported required-control checks; removing it restores those checks without changing required flags, checked state, form ownership or successful-control data. Native and genuine inert attributes, exact namespace/case, raw attribute presence, Attr writers, observer old values, independent inert copies, saved accessors under named-control masking, concrete brands, captured origin and bounded refusal/recovery have focused coverage. Actual App pointer toggles preserve trusted typed text, selection, pixels and neighboring links while changing native submission acceptance. Full constraint validation, general native-form own descriptors, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.acceptCharset now reflects exact authored accept-charset DOMString metadata through writable prototype accessors. Missing values read as the empty string; case, whitespace, arbitrary labels and raw UTF-16 backing remain unchanged, and null and undefined use ordinary string conversion. Native submission continues to serialize supported control data as UTF-8 and report UTF-8 for a successful hidden _charset_ control, independently of the reflected label; reflection does not select a legacy encoder or change the hidden control value. Native and genuine inert attribute identity, exact namespace/case, Attr writers and observer old values, independent inert copies, named-control precedence, saved canonical authority, once-only conversion, captured origin and bounded recovery have focused coverage. Actual App trusted typing, submission data, editor selection, pixels and neighboring link hits remain covered. General native-form own descriptors, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.target now reflects exact authored target DOMString metadata through writable prototype accessors. Missing values read as the empty string; reserved keywords, case, whitespace, arbitrary names and raw UTF-16 backing remain unchanged, and null and undefined use ordinary string conversion. This increment reflects metadata without adding browsing-context selection or changing native submission routing. Native and genuine inert attribute identity, exact namespace/case, Attr writers and observer old values, independent inert copies, named-control precedence, saved canonical authority, once-only conversion, captured origin and bounded recovery have focused coverage. Actual App trusted typing, submission data, editor selection, pixels and neighboring link hits remain covered. General native-form own descriptors, target navigable selection, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.rel now reflects exact authored rel DOMString metadata through writable prototype accessors. Missing values read as the empty string; duplicate tokens, case, whitespace, arbitrary keywords and raw UTF-16 backing remain unchanged, and null and undefined use ordinary string conversion. This increment reflects metadata without adding relList or link-relation processing. Native and genuine inert attribute identity, exact namespace/case, Attr writers and observer old values, independent inert copies, named-control precedence, saved canonical authority, once-only conversion, captured origin and bounded recovery have focused coverage. Actual App trusted typing, submission data, editor selection, pixels and neighboring link hits remain covered. General native-form own descriptors, relList, opener/referrer relation effects, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.relList now exposes a stable live DOMTokenList over the actual null-namespace rel attribute, independent of classList. Exact UTF-16 token reads, iteration, mutations, Attr identity, observation and independent inert copies use the shared original token machinery. Ordinary PutForwards reads the current relList property before forwarding the original value to its value setter; named controls can mask the property and receive that write. Saved canonical accessors retain concrete brands, captured origin and bounded refusal/recovery. Native and inert lists advertise no supported processing tokens while opener/referrer relation effects remain unimplemented. Actual App trusted pointer actions, attribute-driven pixels, neighboring editor selection and link hits remain covered. General native-form own descriptors, link-relation effects, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

Canonical native HTMLFormElement.method now reflects the known get, post and dialog keywords through writable prototype accessors. Missing or invalid values read as get, ASCII case is normalized, and whitespace is not trimmed. Setters retain exact authored DOMString content, including raw UTF-16, with ordinary null/undefined conversion. Native GET and POST continue through the existing submission implementation; dialog reflection does not remove the explicit refusal of DIALOG submission. Native and genuine inert attribute identity, exact namespace/case, Attr writers and observer old values, independent inert copies, named-control precedence, saved canonical authority, once-only conversion, captured origin and bounded recovery have focused coverage. Actual App trusted typing, native POST data, editor selection, pixels and neighboring link hits remain covered. General native-form own descriptors, DIALOG submission, initial child-window reflection and broader form lifecycle remain separate requirements. F01 stays partial.

HTMLHRElement now provides the canonical owner-realm interface for exact HTML hr elements and its legacy align DOMString accessor over actual native/inert attributes. Assignments perform once-only DOMString conversion through the bounded attribute transaction. Exact UTF-16 backing, current Attr identity, observer old values and independent clone/import metadata retain actual owners through supported adoption, connection and shallow native template parking. Parsed void leaves retain null children; metadata does not fabricate separator content or presentation. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update align-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now reflects its legacy color DOMString independently of align over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing are reflected without color parsing or normalization. Once-only conversion, actual Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain real metadata and owners. Align and its actual Attr handle remain unchanged by color writes or removal. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update color-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy noShade boolean accessor over actual lowercase noshade attribute presence. Boolean assignments create an empty null-namespace attribute or remove it without invoking author conversion hooks; the camel-case property remains distinct from lowercase author keys and differently cased inert attributes. Current Attr identity, raw UTF-16 backing, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color and their actual Attr handles remain unchanged by noShade writes and removal. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update noshade-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy size DOMString accessor over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing reflect without numeric parsing, clamping or normalization; numeric assignment uses ordinary string conversion. Once-only conversion, current Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color, noShade and their actual Attr handles remain independent of size writes or removal, including retained author effects during conversion. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update size-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLHRElement now exposes its legacy width DOMString accessor over actual native/inert attributes. Arbitrary strings and exact UTF-16 backing reflect without numeric parsing, clamping or normalization; numeric assignment uses ordinary string conversion. Once-only conversion, current Attr identity, observer old values, independent copies and supported adoption, connection and native template parking retain actual owners. Align, color, noShade and size keep independent actual Attr handles across width writes and removal. Borrowed accessors reject image, canvas, input, select and other wrong brands before conversion; exact namespace/case, captured origin and resource refusal preserve state with fresh recovery. Actual App pointer actions update width-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native separator presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLBRElement now provides the canonical owner-realm interface for exact HTML br elements and its legacy clear DOMString accessor over actual native/inert attributes. Once-only string conversion, exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners. Parsed void leaves keep null children; metadata does not manufacture line-break contents. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Actual App pointer actions update clear-attribute-driven adjacent CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Node equality and document-position methods use bounded deferred data slots while retaining native identity and data descriptors. Legacy clear presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLMenuElement now provides the canonical owner-realm interface for exact HTML menu elements and its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks. Exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners and children. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter scanning needs no string copy or retained growth. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, neighboring editor selection and link hits; scoped production probes exercise the built executable. Native menu presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLDirectoryElement now provides the canonical owner-realm interface for exact HTML dir elements and its legacy compact boolean accessor over actual native/inert attribute presence. Boolean assignment sets an empty null-namespace attribute or removes it without author conversion hooks. Exact UTF-16 backing, current Attr identity, observer old values and independent supported copies, adoption, connection and shallow native template parking retain actual owners and children. Concrete brands, exact namespace/case, captured origin and work/copy/storage/string/node/deadline refusal preserve state with fresh recovery. Getter scanning needs no string copy or retained growth. Actual App pointer actions update compact-attribute-driven CSS paint while preserving text, neighboring editor selection and link hits; scoped production probes exercise the built executable. Node namespace-query methods use bounded deferred data slots while retaining native identities, data descriptors and real namespace contexts. Native directory/list presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now provides its canonical interface and legacy color reflection over actual native/inert attributes. The LegacyNullToEmptyString setter maps original null to an empty string while undefined and object conversion retain ordinary DOMString behavior. Exact UTF-16 backing, once-only conversion, concrete brands, captured origin, actual Attr identity and observer old values remain bounded. Real text descendants, independent deep copies, supported imports/adoption and shallow native template parking preserve actual owners and metadata. Work/copy/storage/string/node/deadline refusal retains state with fresh recovery. Actual App pointer actions update attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native presentational hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now reflects its legacy face DOMString independently of color over actual native/inert attributes. Null and undefined use ordinary string conversion; the separate color member keeps its legacy null-to-empty behavior. Once-only conversion, exact UTF-16 backing, actual Attr handles and observer old values, concrete brands, captured origin and bounded refusal/recovery retain actual state. Real text descendants, independent copies, supported imports/adoption and shallow native template parking preserve owners and metadata. Actual App pointer actions update face-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native font presentation, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.

HTMLFontElement now reflects its legacy size DOMString independently of color and face over actual native/inert attributes. Size preserves unrestricted strings and ordinary null/undefined conversion rather than applying integer parsing or clamping. Once-only conversion, exact UTF-16 backing, actual Attr handles and observer old values, concrete brands, captured origin and bounded refusal/recovery retain real state. Real text descendants, independent copies, supported imports/adoption and shallow native template parking preserve owners and metadata. Actual App pointer actions update size-attribute-driven CSS paint while preserving neighboring editor selection and link hits; scoped production probes exercise the built executable. Native font presentational hints, initial child lifecycle and broader HTML interfaces remain required; F01 stays partial.
