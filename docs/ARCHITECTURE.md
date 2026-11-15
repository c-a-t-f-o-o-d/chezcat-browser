# Architecture and boundaries

ChezCat owns the path from source bytes to pixels. App commits navigation and
history state, loader/resource workers fetch or read bounded inputs, HTML/CSS
parsing builds document/style state, and the JavaScript runtime invokes explicitly
branded host capabilities. Layout produces paint items for the software renderer.
Windows APIs provide window/input/presentation and low-level audio output.

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
IVF/VP8 intra, native controls, src replacement and genuine JavaScript
load/play/pause/currentTime operations. Sources retain separate in-root local-file
authority or an immutable exact-origin HTTP(S) transaction context. Network media
uses complete bounded GET responses and the existing decoder/player, without
ranges or a separate runtime. CORS/cross-origin media remains unsupported.
Standalone PCM and Matroska A/V have separate supported contracts. Network media,
script play promises and complete HTMLMediaElement behavior are future work.

Work, retained data, collection sizes and decoder allocations have explicit
bounds, with generous adjustable policies where implemented. Cancellation is
cooperative and outer process watchdogs remain necessary. These controls do not
constitute a whole-process memory sandbox. Parser, codec, FFI and aggregate-memory
coverage require continued negative testing and review.
