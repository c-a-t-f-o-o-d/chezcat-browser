# Development and releases

Use tested main and short-lived feat/, fix/, docs/ and security branches, with
PR integration and recorded checks. Solo maintainer/agent review is identified
honestly; no nonexistent second reviewer is required. Keep one primary finite
feature contract active. release/ branches serve genuinely maintained parallel
lines only. Never import raw pre-transition private ancestry or recovery refs.

Ordinary commits keep the planned product version. Publish releases only for
completed milestones; there is no daily or per-commit release cadence. Choose a
release version once when a finite feature contract or explicitly authorized
replacement milestone is complete. The product version and CSAPUL license version
are independent. Verify the actual tag/release before claiming it published.
During0.x, minor releases can change a stated interface;
patch releases preserve its contract except documented security necessities.
Alpha/RC contracts remain unstable. Persisted-data changes need migration and
recovery tests. See FEATURE_CONTRACTS.md and CAPABILITIES.md.

Completion of a named finite milestone starts release closeout before the next
independent implementation item. Reconcile its integrated source and required
local, hosted and scoped acceptance evidence, then complete the exact final
versioned-source, package and publication checks below. Source integration,
green CI and development-runtime delivery are distinct from published delivery.
Record the actual immutable tag, downloadable assets and downloaded-byte checks
before reporting a release complete.

The planned [v0.81.0-alpha.1](https://github.com/c-a-t-f-o-o-d/chezcat-browser/releases/tag/v0.81.0-alpha.1) is one consolidated catch-up prerelease for the
completed B2 milestone and integrated platform/compatibility prerequisites since
v0.80.0-alpha.1. Its final versioned-source gates, packages and publication remain
pending. The target designation does not establish published assets. Preserve
v0.80.0-alpha.1 and other existing releases as immutable historical delivery.

Keep closeout focused on concrete blockers: a failed exact-source check, missing
required acceptance, an unresolved exposed high/critical runtime defect, unavailable
publication access or a failed package/download check. Record the affected step,
its evidence and the next corrective action. Use the recorded licensing and
publication disposition with the existing exact-term checks; revisit it only
when material facts change. A concrete blocker leaves closeout pending and must
be reported with the next corrective action. Resume the next independent
implementation item after closeout completes.

The existing tag/manual workflow handles completed milestones. It has no daily
schedule or daily-enable option. One publication concurrency group serializes
manual and tag runs without cancelling an in-progress release. Trusted branch
push CI remains verification, with no automatic release promotion.

Cargo, UI/help/User-Agent and Windows string metadata use the full release
SemVer. Windows numeric fields map MAJOR,MINOR,PATCH,0 (each within16bits), with
prerelease labels in strings and tag. Historical0.78 strings are not rewritten.
Build SHA, clean state, compiler/target and payload hashes identify a build; they
do not assert reproducibility across SDK/linker installations or certify safety.

## Contribution and routine verification

Routine hosted Windows CI runs on this maintainer's branch/main pushes. External
intake is currently reports-only: the external-evaluation workflow does not fetch,
execute or evaluate substantive submissions. Ordinary issue reports and minimal
original bug reproducers follow CONTRIBUTING.md and the published agreement.

Before enabling substantive evaluation, operationally verify exact-version C2
assent, private storage and authority records bound to the immutable submission
and agreement digest. A dispatch input or green run does not prove assent.
Maintained inclusion separately requires the executed assignment and countersigned
Schedule A for the exact accepted scope. Keep signatures and identity records
private under docs/CONTRIBUTOR_PRIVACY.md. Owner-controlled work requires no
fictional outside signature. Independent platform and limited original
bug-reproducer rights remain separate.

All build/test jobs have read-only permissions, pinned actions and hosted runners;
no secrets, privileged pull_request_target checkout or personal-desktop runner
evaluates public submissions. No source shared cache or downstream release promotion
is used for external evaluation. CodeQL eligibility for CSAPUL is not assumed.

Maintainer-owned CI/release execution uses a named affirmative acceptance step
under the repository/owner gate: `python tools/accept_license.py --accept-license
<approved-full-license-sha256> --license-scope personal`, with
`CHEZCAT_LICENSE_RECEIPT_DIR` pointing to a runner-temporary external directory.
This is the owner's scoped authorization for that maintainer run, not assent by
outside users or contributors. A directory variable alone grants no acceptance.
Keep the receipt private and out of logs, caches and uploaded artifacts. Extracted
runtime checks inherit and validate the same-text receipt; they have no bypass.
Changed license text needs fresh affirmative acceptance. See BUILDING.md and
LICENSE_ACCEPTANCE.md for local commands and organizational scope.

Use focused tests while editing. Before integration, pass tools/verify_offline.py:
normal Windows tests, optimized build, PE/resources/version/import inspection,
identity/packaging controls and required local App/input journeys. Preserve normal
failures and diagnostic serial retries separately. Routine CI stays offline;
public sites, physical desktop presentation and audio-device checks are separate.

## Exact tagged delivery

1. Complete the finite feature and review actual trust-boundary changes. Update
   version/resources, release notes and accompanying license/scope deliberately.
   CSAPUL1.1 LICENSE bytes for this containing revision are pinned at SHA256
   34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749.
   Existing immutable releases retain their supplied licenses and valid earlier
   grants; never rewrite their assets or tags to adopt later terms.
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
LICENSE and its containing-revision scope, exact acceptance/privacy/contribution
policies and change summary, product docs and exact build identity. Private
acceptance receipts are excluded. Source ZIP is separate and contains buildable
source/resources/fixtures, never app binaries or
private operations. Verify LICENSE working/blob/ZIP digests, source/tag agreement,
complete payload and an extracted offline journey. The committed scope has one
exact `Scope: containing-source-revision` line and one `License-SHA256: <digest>`
line matching the approved license. A parentless snapshot is valid; old ancestry
lists and withdrawn source identities are not distribution prerequisites.
Do not recursively archive a
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
