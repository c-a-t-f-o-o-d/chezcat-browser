# Building and checking ChezCat

The supported development target is x86_64-pc-windows-msvc. Install Microsoft's
C++ linker/build tools and Windows SDK, the Rust 1.99.1 toolchain pinned by
rust-toolchain.toml, and Python 3.14.8. No Cargo package download is needed for
normal builds/tests. Initial toolchain/action installation can require network
access; browser verification remains offline.

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
rustc --version
python tools/accept_license.py --show
python tools/accept_license.py --accept-license 34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749 --license-scope personal
.\Build.cmd
python tools/capture_tests.py --toolchain 1.99.1
python tools/verify_offline.py
```

Read the complete terms before the affirmative acceptance command. The shown
command records personal-scope acceptance; an authorized organization user chooses
`--license-scope organization`. Source builds and tests check a valid receipt;
they do not accept silently. To inspect the current receipt without acceptance,
run `python tools/accept_license.py --check`.

The default receipt is outside the source tree and browser profile, under the
user's local application data in `ChezCat/License/receipt.json`. An explicit
`--receipt-dir <absolute-external-directory>` or the
`CHEZCAT_LICENSE_RECEIPT_DIR` environment variable selects an external location;
selecting a directory is not consent. Keep receipts out of source, runtime ZIPs,
logs and uploaded artifacts. Matching exact license text can reuse its receipt;
changed text requires affirmative acceptance again. See LICENSE_ACCEPTANCE.md.

The application supports `ChezCat.exe --license` without browsing or networking.
Its affirmative CLI is `ChezCat.exe --accept-license <full-license-sha256>
--license-scope personal|organization`, with optional
`--license-receipt-dir <absolute-external-directory>`. The same receipt governs
ordinary CLI, GUI, headless and automation use before browsing/network activity.
Help and license viewing do not require acceptance.

The final verification command runs the normal Windows suite, optimized build,
PE checks and all required local interaction journeys with bounded child-process
cleanup. Its
JSON report records the source input map and executable hashes. A serial diagnostic
retry never erases a normal failure. Results and build outputs are ignored files.
Loopback fixture HTTP servers are local tests, not public-service probes.

The journeys cover form/query/destination/Back/second-input behavior against local
pages, provider persistence/None, CSS resources, native interfaces/events, real
video frames, audio metadata, A/V metadata, source replacement/reload and real
animation wrappers/cancellation/microtasks/navigation. They are
App/input/offscreen coverage. Device audibility, physical desktop input and native
presentation while locked are separate, unclaimed checks.

Resource files are necessary generated build inputs, not application executables.
After intentionally changing resources/chezcat.rc or assets/chezcat.ico, run:

```powershell
python tools/build_icon_resources.py --portable
```

Commit the matching .res/.o/checksum inputs and verify version/icons again. The
normal build uses those resources without an external icon generator. Development
reference scripts may optionally require Node, Pillow, OpenSSL or other tools;
those are not production dependencies and are not required by verify_offline.py.

A runtime bundle needs ChezCat.exe, pages/, assets/, trust/, notices/, relevant
data notices, LICENSE, the containing-revision LICENSE_SCOPE.md, license
acceptance/contributor privacy policies, the 1.1 change summary and exact build
metadata. License receipts are private local state and are never runtime
payloads. No historical commit list is required or included. A source ZIP is not a
ready Windows runtime bundle. The release packager deliberately refuses an absent,
unapproved/draft or hash-mismatched project license. Do not package a development
workspace recursively. Build identity includes Git commit, clean/dirty status,
compiler/target and file hashes; identity is not proof of security or bit-for-bit
reproducibility across different Windows SDK/linker installations.

For private review only, --review-source on build_release_bundle.py creates a
clearly named unapproved source ZIP without adopting licensing terms. Normal
--source and runtime modes require the approved tracked license and hash, plus
one exact `Scope: containing-source-revision` and `License-SHA256: <digest>`
designation in the committed scope notice. A parentless public source snapshot
is supported. The containing commit or exported SOURCE_REVISION supplies its
source identity; obsolete ancestry IDs cannot authorize the package. A ZIP
built/tested outside Git records its exported SOURCE_REVISION with dirty status
unknown; it cannot authorize a runtime release package.


## Selecting local build and verification storage

`CARGO_TARGET_DIR` selects a reusable Cargo cache. `Build.cmd` validates that
location and copies the release executable from the same selected target.
The source checkout and root `ChezCat.exe` keep their existing roles.

`CHEZCAT_VERIFICATION_ROOT` optionally selects an absolute, separate local
verification directory; without it, normal tools retain `results/verification`.
Normal test logs and the fixed offline journeys inherit this selection, including
their nested checkers. Small `latest-*.json` pointers remain in the checkout and
external references use confined `verification:` names. Raw reports stay private;
public summaries still project only their bounded approved fields.

Set these values and `TEMP`/`TMP` only for owned child processes. No compiler,
credential store, real license receipt or profile is relocated. Keep one writer
per output tree and retain failed/ambiguous evidence. Output selection does not
relax clean-source, exact executable, offline/locked, license or test requirements.
Frozen compatibility scorecards retain their pinned invocation/storage contract.
