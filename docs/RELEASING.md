# Development and release process

The source preview stays at historical 0.78 / Cargo0.78.0. Ordinary commits no
longer bump the product version. Work on short-lived feat/fix/security/docs
branches and integrate through PRs. One maintainer may self-review with that fact
recorded; do not configure an impossible second-human approval requirement.
Use release/0.x only for an actually maintained parallel line.

The proposed first public alpha is the next unused minor: **0.79.0-alpha.1**,
subject to review. It is not created by this preview. Keep all historical numbers;
CSAPUL1.0 does not mean browser1.0. During0.x development, minor versions may
change the declared CLI/profile/config/export contract; patch releases preserve
that minor's stated contract except documented security necessities. Alpha/RC
interfaces remain explicitly unstable. Breaking persisted-data changes require
migration/recovery tests and release notes.

For future releases, Cargo/UI/help/User-Agent/product strings use the full agreed
SemVer value; Windows numeric fields map MAJOR,MINOR,PATCH,0. Prerelease labels stay
in string metadata and the release tag. Each numeric component must fit the
Windows representation. The legacy0.78 resource strings remain0.78; do not rewrite
old checkpoints to manufacture consistency with a new policy.

Build identity is separate: Git SHA, clean/dirty state, compiler/target and source/
executable hashes. tools/verify_offline.py creates a build-info JSON. A commit or
version bump does not authorize publication or certify program safety.

## Routine gates

The proposed offline PR/main workflow uses GitHub-hosted Windows runners, pinned
actions, read-only repository permissions, no contribution secrets, no
pull_request_target and no personal-desktop runner. Run normal Windows tests,
optimized build, PE/resource checks and all eleven local App/input journeys.
Public sites, human challenges, physical audio and desktop presentation are opt-in.
Use focused tests while editing; a serial retry cannot erase a failed normal run.
CodeQL's standard free CLI open-source basis is not assumed for custom CSAPUL;
verify any paid/hosted entitlement before adding it.

## Reviewed release

1. Approve exact project license, covered history/material, third-party notices,
   privacy/security disposition, contribution policy and reporting channel.
2. Finish one feature contract, choose a release version once, update resources
   and all version strings, and pass the final gates on a clean public lineage.
3. Recheck public main, create the exact reviewed tag, and build that clean source.
   Never merge unsanitized private ancestry or recovery refs into product history.
4. The tag workflow is disabled until ENABLE_CHEZCAT_RELEASES is explicitly set
   true and the release environment is configured for owner approval (self-review
   allowed). APPROVED_LICENSE_SHA256 must match the operative LICENSE. Its jobs
   create local runtime/source assets, checksums and build identity only. They
   upload no binaries, source ZIPs or attestations; a tag is not artifact approval.
5. The maintainer prepares a retained local build and reviews its exact artifacts
   before approving their upload as
   release assets. Executables never enter source history. Re-fetch before the
   first import and use only a normal fast-forward update of approved refs.

The packager refuses an absent/draft/mismatched license and dirty source. Hosted
workflows, attestation and publication are not validated merely by local tests.
Historical CI signatures do not authenticate rewritten hashes. Keep raw captures,
profiles, operational notes and sensitive audit findings outside product history.

References: [SemVer](https://semver.org/),
[GitHub workflow security](https://docs.github.com/en/actions/reference/security/secure-use),
[artifact provenance](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations).

A hosted runner's local ZIPs disappear when its job ends. Actions artifacts and
Sigstore/GitHub attestations are publication too, so this workflow deliberately
performs neither. Prepare retained review files locally; any later artifact upload
or attestation job needs approval of those exact hashes and separate validation.
