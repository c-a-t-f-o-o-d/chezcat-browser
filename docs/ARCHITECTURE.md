# Architecture and boundaries

ChezCat owns the path from source bytes to pixels. App commits navigation and
history state, loader/resource workers fetch or read bounded inputs, HTML/CSS
parsing builds document/style state, and the JavaScript runtime invokes explicitly
branded host capabilities. Layout produces paint items for the software renderer.
Windows APIs provide window/input/presentation and low-level audio output.

Browser profile, downloads and saved-history authorities are separate from page
state. App views retain owned snapshots and weak access handles; a view cannot
extend an authority's lifetime or perform final history shutdown. Saved-history
settings carry an original policy token and check the latest policy inside the
existing locked file transaction. Row selections retain the complete observed
entry, so a refreshed same-ID replacement cannot become the selected action.
The browser coordinator must explicitly perform final history shutdown; dropping
an App never silently clears history or performs disk I/O.

A supported CSS background URL retains the winning declaration's stylesheet owner
and final redirected base. Backgrounds reuse the image worker and resource policies
with a separate binding slot and bitmap map, keeping replaced-image state and
natural dimensions independent. Block decorations share normal layout, clip,
paint and input-occlusion rules. Initial and callback-selected background work
participates in the real App document-load checkpoint.

The application is one Windows process with worker threads. There is no OS
renderer sandbox or separate untrusted-process boundary. Rust safety does not
prove FFI, cryptography, resource policy or availability correctness.

Networking uses original HTTP/TLS/certificate/cookie/URL code. Resource type,
origin, CSP, credentials and local-root policies remain explicit. Unsupported
protocol or policy behavior is refused or documented; TLS revocation/CT and broad
browser conformance are not claimed. Cookie/public-suffix data has a separate
provenance and refresh lifecycle.

The interpreter is an original bounded language implementation with realms,
objects, closures, promises/jobs and browser host routing. Native handles require
real owner/receiver provenance. Main/child contexts keep distinct intrinsic roots;
child DOM support is a deliberately limited read-only subset. Node/Element base
ancestry does not fabricate every specialized immediate prototype. JavaScript
UTF-16 strings, modules, fetch/XHR and many APIs remain gaps.

Local media uses original container/codec/sample code and real decoded frames or
samples. Source retirement tokens, checked generations and worker cancellation
prevent stale work from publishing. Native presentation/audio APIs do not replace
decoding. The authored element currently supports one connected parser video,
finite IVF/VP8 or Matroska VP8/PCM16, native controls, src replacement and genuine JavaScript
load/play/pause/currentTime operations. Sources retain separate in-root local-file
authority or an immutable document-owned HTTP(S) transaction context. Network
media uses complete bounded GET responses and the existing decoder/player,
without ranges or a separate runtime. Missing crossorigin keeps exact-origin
policy; explicit authored modes use dedicated CORS/credential checks, sticky
redirect state and normal cookie protections. Dynamic crossOrigin APIs remain
unsupported.
Standalone PCM and Matroska A/V have separate supported contracts. The finite
VP8 profile adds visible version-zero single-vector interframes with immutable
YUV references and transactional entropy/filter/segment updates. Every requested
frame replays from its preceding keyframe under shared work and packet limits;
failed or canceled work cannot publish reference state or stale pixels. Matroska
video, including the supported authored Matroska profile, follows the existing
acknowledged PCM sample clock. Authored audio is limited to that finite PCM16
profile; streaming, MSE, multiple players and complete HTMLMediaElement behavior
remain unsupported.

Work, retained data, collection sizes and decoder allocations have explicit
bounds, with generous adjustable policies where implemented. Cancellation is
cooperative and outer process watchdogs remain necessary. These controls do not
constitute a whole-process memory sandbox. Parser, codec, FFI and aggregate-memory
coverage require continued negative testing and review.
