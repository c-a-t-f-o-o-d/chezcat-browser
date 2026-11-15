# Building and checking ChezCat

The supported development target is x86_64-pc-windows-msvc. Install Microsoft's
C++ linker/build tools and Windows SDK, the Rust 1.99.1 toolchain pinned by
rust-toolchain.toml, and Python 3.14.8. No Cargo package download is needed for
normal builds/tests. Initial toolchain/action installation can require network
access; browser verification remains offline.

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
rustc --version
.\Build.cmd
python tools/capture_tests.py --toolchain 1.99.1
python tools/verify_offline.py
```

The final command runs the normal Windows suite, optimized build, PE checks and
all eleven local interaction journeys with bounded child-process cleanup. Its
JSON report records the source input map and executable hashes. A serial diagnostic
retry never erases a normal failure. Results and build outputs are ignored files.
Loopback fixture HTTP servers are local tests, not public-service probes.

The journeys cover form/query/destination/Back/second-input behavior against local
pages, provider persistence/None, CSS resources, native interfaces/events, real
video frames, audio metadata, A/V metadata and source replacement/reload. They are
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
data notices, LICENSE, LICENSE_SCOPE.md, licensing/COVERED_HISTORY.txt and exact
build metadata. A source ZIP is not a ready
Windows runtime bundle. The release packager deliberately refuses an absent,
unapproved/draft or hash-mismatched project license. Do not package a development
workspace recursively. Build identity includes Git commit, clean/dirty status,
compiler/target and file hashes; identity is not proof of security or bit-for-bit
reproducibility across different Windows SDK/linker installations.

For private review only, --review-source on build_release_bundle.py creates a
clearly named unapproved source ZIP without adopting licensing terms. Normal
--source and runtime modes require the approved tracked license and hash. A ZIP
built/tested outside Git records its exported SOURCE_REVISION with dirty status
unknown; it cannot authorize a runtime release package.
