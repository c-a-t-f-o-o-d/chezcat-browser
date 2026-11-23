<div align="center">

# Building and checking ChezCat

*A reproducible starting point for local builds and scoped verification.*

[Home](../README.md) · [Roadmap](ROADMAP.md) · [Capabilities](CAPABILITIES.md) · [Compatibility](COMPATIBILITY.md)

</div>

---

## Prerequisites

| Requirement | Supported setup |
| :--- | :--- |
| **Target** | Windows x64, `x86_64-pc-windows-msvc` |
| **Native tools** | Microsoft C++ linker/build tools and Windows SDK |
| **Rust** | `1.99.1`, pinned by `rust-toolchain.toml` |
| **Python** | `3.14.8` for development/verification scripts |
| **Production dependencies** | No third-party Cargo packages are required for normal builds/tests |

Initial toolchain/action installation may require network access. Browser
verification uses offline/local fixtures; this does not mean every setup step is
network-free.

## Quick start

### 1 · Install the pinned compiler and read the terms

```powershell
rustup toolchain install 1.99.1 --profile minimal --component clippy --no-self-update
rustc --version
python tools/accept_license.py --show
```

### 2 · Accept only after reviewing the exact terms

The following is an affirmative **personal-scope** choice, not a required copy-and-
paste step before reading. Use organization scope only with the necessary authority.

```powershell
python tools/accept_license.py --accept-license 34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749 --license-scope personal
```

Source builds/tests check a valid receipt; they do not silently create acceptance.
To inspect an existing receipt without accepting again:

```powershell
python tools/accept_license.py --check
```

### 3 · Build and keep the resources together

```powershell
.\Build.cmd
```

`Build.cmd` writes `ChezCat.exe` beside the source checkout. Keep matching pages,
assets, trust data, and notices with the executable. Rust and Python are development
tools, not application runtime dependencies. A source ZIP is not a ready-to-run
Windows runtime bundle.

### Faster local iteration

```powershell
.\build_dev.cmd
```

This wrapper uses `Build.cmd` with LTO disabled, 16 code generation units, and
incremental compilation enabled for this invocation. It keeps the pinned toolchain,
license check, offline/locked build, and normal `ChezCat.exe` output.

Development artifacts are reused in `target\build-dev`, or a `build-dev` child of
your configured `CARGO_TARGET_DIR`. The first run fills this separate cache;
subsequent edits can reuse it. Both scripts replace `ChezCat.exe` in the checkout,
so run them sequentially. Use `.\Build.cmd` for normal release optimization before
performance comparisons or release qualification.

## Choose the right verification scope

| Work | Check | Evidence boundary |
| :--- | :--- | :--- |
| **Ordinary edits** | Focused affected tests and relevant security negatives | Proves the changed behavior, not a complete integration pass |
| **Native suite** | `python tools/capture_tests.py --toolchain 1.99.1` | Normal Windows test results for the actual source/configuration |
| **Release qualification** | `python tools/verify_offline.py` | Normal tests, optimized build, PE checks, and required local App journeys |
| **Physical devices / public sites** | Separate explicitly scoped checks | Not inferred from routine offline verification |
| **Named release** | [Release procedure](RELEASING.md) | Fully accepted milestone or a specific release request |

The full verifier needs the location of the existing receipt explicitly,
including when it is in the default Windows directory. If you keep it elsewhere,
select that external directory instead.

```powershell
$env:CHEZCAT_LICENSE_RECEIPT_DIR = Join-Path $env:LOCALAPPDATA 'ChezCat\License'
python tools/accept_license.py --check
if ($LASTEXITCODE -eq 0) { python tools/verify_offline.py }
```

The complete gate uses bounded child-process cleanup and records source-input maps
and executable hashes. A serial diagnostic retry does not erase a normal failure.
Results/build outputs are ignored files. Loopback HTTP fixtures are local tests,
not probes of public services. Failed required checks retain their evidence and
block the corresponding qualification until the failure is resolved.

<details>
<summary><strong>🧪 What the local journeys exercise</strong></summary>

Form/query/destination/Back/second-input behavior, provider persistence/None, CSS
resources, native interfaces/events, real video frames, audio and A/V metadata,
source replacement/reload, and animation wrappers/cancellation/microtasks/navigation.
These are App/input/offscreen checks. Physical audibility, desktop input, and
native presentation while locked remain separate, unclaimed outcomes.

</details>

## License receipts and command-line use

The default receipt is outside the checkout/profile in the user's local application
data under `ChezCat/License/receipt.json`. `--receipt-dir <absolute-external-directory>`
or `CHEZCAT_LICENSE_RECEIPT_DIR` selects another external location. **Choosing a
folder is not consent.** Keep receipts out of source, runtime ZIPs, logs, and uploaded
artifacts. Identical exact terms may reuse a receipt; changed terms need renewed
acceptance. See [license acceptance](LICENSE_ACCEPTANCE.md).

`ChezCat.exe --license` is available without browsing/networking. The affirmative
application CLI is `ChezCat.exe --accept-license <full-license-sha256>
--license-scope personal|organization`, optionally with
`--license-receipt-dir <absolute-external-directory>`. The same receipt controls
ordinary CLI, GUI, headless, and automation use before browsing/network activity.
Help and license viewing remain available without acceptance.

## Selecting local build and verification storage

| Setting | Purpose | Constraint |
| :--- | :--- | :--- |
| `CARGO_TARGET_DIR` | Reusable Cargo build cache; Build.cmd copies from this selected target | Keep one writer per output tree |
| `CHEZCAT_VERIFICATION_ROOT` | Optional absolute, separate verification directory | Defaults to `results/verification`; confined external references use `verification:` names |
| `TEMP` / `TMP` | Child-process temporary storage | Keep outside every source checkout, including parent repositories |
| `CHEZCAT_LICENSE_RECEIPT_DIR` | Existing external receipt location | Does not accept terms or create a receipt |

Normal logs and fixed offline journeys inherit verification storage, including
nested checkers. Small `latest-*.json` pointers remain in the checkout. Raw reports
stay private; public summaries include only approved bounded fields. Frozen
compatibility scorecards retain their pinned invocation/storage contract.

Set these values only for owned child processes. Do not relocate compilers,
credentials, actual receipts, or profiles. Retain failed/ambiguous evidence.
Output selection never relaxes source/build identity, offline/locked, licensing,
or test requirements. For the complete gate, explicitly select and check the
existing external receipt.

## Generated resources

After intentionally changing `resources/chezcat.rc` or `assets/chezcat.ico`:

```powershell
python tools/build_icon_resources.py --portable
```

Commit matching `.res`, `.o`, and checksum inputs; verify version/icons again.
Normal builds link those supplied resources without an external icon generator.
Optional development references may use Node, Pillow, OpenSSL, or other tools;
they are not production dependencies and are not required by `verify_offline.py`.

## Source and runtime identity

The verified F00 release is [v0.82.0-alpha.4](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.82.0-alpha.4), source `122d0b7de5988493481dc4bed168d01969eef488`. Extract the Windows runtime ZIP with its resources together. [Asset hashes and runtime evidence](RELEASING.md#published-baseline)
The earlier alpha.3 candidate at `ff5740d` failed hosted normal qualification.
The [release record](RELEASING.md#published-baseline) identifies the verified download.
The earlier immutable `0.82.0-alpha.2` tag identifies
`14134aba8ac2ab001c8ec13db77f68fa220b1400`; the
[tagged Windows workflow](https://github.com/c-a-t-f-o-o-d/chezcat-browser/actions/runs/44971512445)
failed at the normal native-test stage before the optimized release build and
package publication. Alpha.2 has no published release assets. A local build
remains a development build unless the normal tagged
workflow binds the matching immutable tag.

Published `0.82.0-alpha.1` identifies tagged source
`f2e1823266a7c08c89bce7293572d801da62053c`. Its downloaded package, PE, HTTP User-Agent,
CLI and About identities passed the [release checks](RELEASING.md#published-baseline).

About shows the development build identity and source state. As a historical
example, the verified clean integrated build at
`4d72251915d6f2c9e74c38721ddc7fd3f2f63d5f` displays
`0.81.0-alpha.2+dev.55.g4d72251915d6`. The number comes from verified public main
history and the suffix identifies the source commit. A feature head outside that
history, a dirty checkout or unavailable history uses `+dev.g<commit>` when the
revision is known and `+dev.unknown` otherwise. Declared tagged builds display the
base version and identify the declared tag with publication unverified. About
shows clean, dirty or unknown source state separately. That embedded label is not
rewritten by later download verification.

The `dev` ordinal is **not a pull-request number**. Compare the full source revision
with the intended merged commit, and record whether the working tree was clean.
Pulling newer source does not replace an existing executable; rebuild before
using that executable as evidence for the newer source.

<details>
<summary><strong>📦 Packaging boundaries and private-review exports</strong></summary>

A runtime bundle includes the executable, pages/assets/trust/notices, relevant
data notices, LICENSE, containing-revision LICENSE_SCOPE, acceptance/privacy/
contribution policies, change summary, and exact build metadata. Receipts and
private operational material are never runtime payloads. Do not recursively
archive a workspace. Missing, draft, unapproved, or hash-mismatched licensing
causes packaging to refuse.

Build commit, clean/dirty state, compiler/target, and file hashes are identities,
not security certification or proof of bit-for-bit reproduction across SDK/linker
installations.

`--review-source` on `build_release_bundle.py` creates a clearly named unapproved
source ZIP for private review only. Ordinary `--source` and runtime modes require
the approved tracked license/hash and exact containing-revision scope markers.
Parentless public snapshots are supported; obsolete ancestry identities cannot
authorize a package. Outside Git, an exported SOURCE_REVISION identifies a source
ZIP with dirty state unknown; it does not authorize a runtime release package.

</details>
