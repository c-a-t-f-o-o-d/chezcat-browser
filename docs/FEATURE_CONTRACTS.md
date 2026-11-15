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

Outcome: the existing bound authored IVF/VP8-intra player responds to real
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
authored element audio, range streaming, MSE, or public-service acceptance.

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

Every later feature records semantic references, original and selected conformance
tests, security review, native/device/live checks still pending and exact final
source/executable identity. Release notes describe only proven behavior.
