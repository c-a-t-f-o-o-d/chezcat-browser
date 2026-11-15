# Initial feature contracts

Scope is agreed before implementation. Completion means all contracted normal,
error, lifecycle and security acceptance is met; missing criteria cannot be hidden
by shrinking the contract afterward. These contracts authorize no live check or
public release by themselves.

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

The service objective remains actual accessible non-DRM playback with readable
controls and real decoded video/audio output. Authored fixtures
and interface prerequisites establish finite progress; they do not establish
YouTube or other public-service acceptance.

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

MouseEvent, KeyboardEvent and CustomEvent constructors, their immediate
specialized prototypes and native-input immediate prototypes remain unsupported.
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

Genuine legacy Mouse and active native keyboard events have supported UIEvent
ancestry and borrowed getters; their immediate prototypes remain opaque. Native
click producers currently provide base Event state, without fabricated UI data.
The actual UIEvent App/input journey uses owned HTTP for its child context,
one native key down/up pair and two trusted activations with real UI construction,
dispatch/cancellation, independent path copies and readable changed pixels.
It retains the original dispatch/guard/recovery cases. Retained own-child native
global reads after a parent-origin change remain a separate unsupported profile;
constructor owner tests use the real Window captured while same-origin.

Semantic references: [UIEvent](https://w3c.github.io/uievents/#interface-uievent),
[UIEventInit](https://w3c.github.io/uievents/#dictdef-uieventinit) and
[Web IDL dictionary conversion](https://webidl.spec.whatwg.org/#es-dictionary).

Semantic reference: [DOM Event, initialization and dispatch](https://dom.spec.whatwg.org/#interface-event).

Legacy notification prerequisite: the main document's singular, case-insensitive
createEvent('CustomEvent') creates a genuine existing-model event. Its readonly
detail stores the actual any value, preserving object/function identity. The
branded initCustomEvent converts arguments in order, uses declared defaults,
returns without reinitialization during dispatch, and otherwise updates the same
event atomically. Base initEvent preserves subclass detail. A real initialized
flag distinguishes a fresh factory event from an initialized empty type.

Existing synchronous dispatch, propagation, cancellation, trust, fatal cleanup,
host quotas and owner-realm machinery remain authoritative. Initializer methods
retain their genuine Function realm and validate owner access on both fresh and
cached reads. Direct detail strings use checked retained-byte accounting and
fallible copies; object graphs retain the existing conservative exported-argument
lifetime. Legacy CustomEvent adds no specialized constructor/prototype,
child factory, activation grant or collector redesign. Its immediate prototype
stays opaque while the genuine base Event ancestry and borrowed operations are
available. A global CustomEvent constructor remains absent.

Original Page tests cover defaults/brands, identity, readonly state, conversion
effects and failures, dispatch/empty-type initialization, string/creation bounds,
cross-realm method/detail behavior, cached access and fatal recovery. The offline
dispatch journey preserves its prior guard/standalone/prototype checks and adds
two real native button activations, actual listener detail/cancellation guards,
current zero-error diagnostics and paired readable output/pixel changes. It makes
no service request or physical-desktop claim.

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
