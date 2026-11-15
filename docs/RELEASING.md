# Development and releases

Use tested main and short-lived feat/, fix/, docs/ and security branches, with
PR integration and recorded checks. Solo maintainer/agent review is identified
honestly; no nonexistent second reviewer is required. Keep one primary finite
feature contract active. release/ branches serve genuinely maintained parallel
lines only. Never import raw pre-transition private ancestry or recovery refs.

Ordinary commits keep the planned product version. Choose a release version once
when a feature contract is complete. Preserve historical numbers: imported0.78
is not browser1.0 because CSAPUL is1.0. The intended first animation milestone is
0.79.0-alpha.1 if unused. During0.x, minor releases can change a stated interface;
patch releases preserve its contract except documented security necessities.
Alpha/RC contracts remain unstable. Persisted-data changes need migration and
recovery tests. See FEATURE_CONTRACTS.md and CAPABILITIES.md.

Cargo, UI/help/User-Agent and Windows string metadata use the full release
SemVer. Windows numeric fields map MAJOR,MINOR,PATCH,0 (each within16bits), with
prerelease labels in strings and tag. Historical0.78 strings are not rewritten.
Build SHA, clean state, compiler/target and payload hashes identify a build; they
do not assert reproducibility across SDK/linker installations or certify safety.

## Contribution and routine verification

Routine hosted Windows CI runs on this maintainer's branch/main pushes. It does
not automatically execute arbitrary external PR code. Before a substantive
external evaluation dispatch, privately retain the exact C2 assent, agreement
digest, immutable submission SHA, rights holder/authority and date. The controlled
dispatch requires that SHA and an opaque record reference; neither the input nor
a green run proves assent. Updated content needs an appropriate updated record.
Before maintained product/test inclusion, obtain the executed assignment and
countersigned Schedule A for the exact accepted scope. Never publish signatures
or identity records. Owner-controlled work requires no fictional outside signature.
Independent platform and limited original bug-reproducer rights remain separate.

All build/test jobs have read-only permissions, pinned actions and hosted runners;
no secrets, privileged pull_request_target checkout or personal-desktop runner
evaluates public submissions. No source shared cache or downstream release promotion
is used for external evaluation. CodeQL eligibility for CSAPUL is not assumed.

Use focused tests while editing. Before integration, pass tools/verify_offline.py:
normal Windows tests, optimized build, PE/resources/version/import inspection,
identity/packaging controls and required local App/input journeys. Preserve normal
failures and diagnostic serial retries separately. Routine CI stays offline;
public sites, physical desktop presentation and audio-device checks are separate.

## Exact tagged delivery

1. Complete the finite feature and review actual trust-boundary changes. Update
   version/resources, release notes and accompanying license/scope deliberately.
   CSAPUL1.0 LICENSE bytes remain pinned at SHA256
   632232a42c046628de0fd8c497e89f02055e1e919860bfa9fcd8fe3b5eb06d65.
2. Validate the exact clean final commit, including the actual merge identity.
   Re-fetch main; use only explicit non-force branch updates preserving others' work.
3. Create a new unused immutable v<fullCargoVersion> tag on that public source.
   Never move/reuse it for corrected source. A source change after failure requires
   a new prerelease version/tag; a no-source-change service rerun is recorded.
4. release.yml checks out that exact tag, validates it and the adopted license,
   runs offline gates, packages runtime/source ZIPs and checksums, checks payloads
   and exercises the extracted runtime. Only its explicit release payloads and
   public-safe provenance record are retained as Actions artifacts.
5. The separate publication job has contents:write, executes no project code,
   rechecks transferred hashes and remote tag, creates the actual release with
   verified existing tag, uploads explicit assets, then downloads/checks their
   bytes. Ordinary jobs retain contents:read; repository-wide defaults stay read.

Runtime ZIP includes executable, pages/assets/trust, applicable notices/data,
LICENSE and scope/history, product docs and exact build identity. Source ZIP is
separate and contains buildable source/resources/fixtures, never app binaries or
private operations. Verify LICENSE working/blob/ZIP digests, source/tag agreement,
complete payload and an extracted offline journey. Do not recursively archive a
development workspace. Missing/draft/mismatched license or dirty/stale proof fails
closed. The packager does not publish; release workflow performs actual delivery.

Release notes state supported behavior and deferred features, exact source/tag,
downloadable assets and SHA256SUMS. publication.json describes actual checks and
compiler/payload identities; it is not a signed attestation. Claim hosted CI,
attestations, device tests or reproduction only when they actually ran. An actual
unresolved exposed high/critical runtime defect blocks the affected binary release;
absent exhaustive certification is not a substitute for concrete risk disposition.

Later license changes need an explicit revision and release/scope record. Never
silently replace the exact license attached to an existing immutable release.
