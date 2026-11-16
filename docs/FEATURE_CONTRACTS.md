# Initial feature contracts

Scope is agreed before implementation. Completion means all contracted normal,
error, lifecycle and security acceptance is met; missing criteria cannot be hidden
by shrinking the contract afterward. These contracts authorize no live check or
public release by themselves.

## T1 — shared TLS/trust and data coverage baseline

Status: in progress for bounded diagnostics, a reviewed Mozilla-derived trust
subset and ordinary document/resource coverage. Original TLS 1.2/1.3 and X.509
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
existing item/forEach and readonly indexed/length behavior remain. Out-of-range
indices return undefined and item returns null; HTMLCollection named aliases are
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

The HTML tokenizer still omits parsed comments and DocumentType nodes. CP2 adds
native-created Comments separately; no full Document/template-content model is
implied.
Template childNodes explicitly declines rather than exposing inert AST input as
a fabricated fragment. Hidden child Documents keep their existing unsupported
collection boundary; ordinary list reads do not activate inert queries/resources.
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

Status: Prerequisite. Focused original Page, App and language controls pass;
normal native/build/executable integration is required for each checkpoint.
The unchanged selected legacy workload reaches a later select-containing
innerHTML fragment refusal, with initialization and visible improvement unproved.

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

The tokenizer still omits parsed comments, and checked fragment parsing still
refuses comments. No Comment constructor/prototype family, full CharacterData,
DocumentType, template-content tree or child-context factory is claimed.
Loaded-resource, origin and cancellation boundaries remain.

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
this is no full-suite conformance claim. The select fragment is this workflow
blocker, while independently demonstrated responsive CSS can form the next
bounded page-usability pack before larger Forms work.

Semantic references: [DOM createComment](https://dom.spec.whatwg.org/#dom-document-createcomment),
[Comment](https://dom.spec.whatwg.org/#interface-comment), and
[ECMAScript 2025 ToString](https://262.ecma-international.org/16.0/#sec-tostring).

## CP3 — responsive stylesheet-to-layout behavior

Status: measured rendering improvement in the unchanged Foundation6.9 fixture;
full normal/build/executable verification remains required for integration.
At1280x800, zoom100 and normal JavaScript, the original panels now share a row;
at640x800 they retain their original stacking. Vue's prototype initialization
failure is separate, so no reactive interaction or full-framework pass is claimed.

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

Status: shared-profile prerequisite implemented after the first measured
responsive rendering improvement; full normal integration remains required.
No tab or new-window capability is claimed by this prerequisite. Provide new/switch/close
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
run after retargeting; future native routing must reject closed/reused WindowIds
and use nonblocking signals. The adapter alone does not provide native lifetime
acceptance. Original held-transfer/two-App, wake reentry/retarget, admission,
cancel/retry/no-overwrite, received-response, closed-authority and legacy popup
controls cover this prerequisite; full normal integration remains required.

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
internal App ownership slice. Native userdata, inspector bindings, reentrant
window calls, scheduling, tab-strip input and link/new-window dispositions remain
subsequent acceptance; full normal integration is required for each checkpoint.

## A1 — About and running-build identity

Status: finite source/UI implementation, with normal build and actual executable
acceptance required for integration. Use the existing browser-owned native panel
and menu input paths. Keep the underlying document/source, history, scroll and
resource workers owned by the same page; About Escape closes the panel rather
than issuing Stop. Closing returns to that page without reload or navigation.

Display the embedded full product SemVer, including prerelease, source revision
when known, source origin/clean-dirty-unknown state and build kind. Keep product
and license versions separate. Metadata is a build-time description: an exported
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

Status: planned after the first B1/B2 milestones. Prefer one per-user x64 MSI,
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

The executable journey checks actual script/native controls and promises, all24
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

KeyboardEvent construction, its specialized prototype and native-input
immediate prototypes remain unsupported. Synthetic
Mouse dispatch/default activation and native pointer payloads remain unsupported.
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
