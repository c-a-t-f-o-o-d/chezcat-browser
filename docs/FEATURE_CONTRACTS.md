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

## F4 — network media transaction

Outcome: one original supported clip plays from controlled HTTP(S) origins using
existing source/decoder machinery. Define full-fetch/range behavior before coding.

Acceptance: allowed/denied CORS and credentials, redirects, CSP, TLS, MIME, bounded
encoded/decoded input, cancellation/replacement/errors/retry policy, valid range
responses if used, and actual frames/time. Routine CI uses loopback servers only.
Transport200 alone does not pass playback. No stream-admission/token/DRM bypass.

Every later feature records semantic references, original and selected conformance
tests, security review, native/device/live checks still pending and exact final
source/executable identity. Release notes describe only proven behavior.
