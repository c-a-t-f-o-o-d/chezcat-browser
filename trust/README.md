# Explicit HTTPS trust

ChezCat reads `trust/roots.pem` beside the executable once at startup. The file
contains certificate data; the browser's original Rust TLS, X.509, RSA and ECDSA
code validates each chosen path. It does not use the operating-system root
store, discover authorities, download updates or bypass certificate errors.

## Default data and selection

The default contains 98 self-signed authorities selected from Mozilla NSS
built-ins 2.94, revision `8c60e757c30b1b239bae46532b4f6d4ebb1e1cad`
(2026-11-13 UTC). [mozilla/PROVENANCE.json](mozilla/PROVENANCE.json) pins the
upstream revision, source hashes, external-constraint references and license.
The unchanged [certdata.txt](mozilla/certdata.txt) snapshot retains upstream
notices; its MPL-2.0 terms and [license](mozilla/LICENSE-MPL-2.0.txt) remain
separate from the original browser's license.

The original offline [generator](https://github.com/c-a-t-f-o-o-d/chezcat-browser/blob/9ad97abd2d2f971d976d8260c4087b7319d9f64f/tools/generate_trust.py) joins certificates
to their exact NSS trust objects. It selects server-auth trusted delegators only
after an explicit original-verifier capability review and the recorded
[policy](mozilla/policy.json). Of 113 server-auth authorities, 103 have supported
certificate forms. Four with unimplemented leaf-issuance distrust cutoffs and
one with an externally imposed `.tr` name restriction are excluded entirely.
Ten unsupported authorities and 54 other certificates are also excluded. No
restricted authority is promoted to unrestricted trust. The exact included and
excluded DER SHA-256 identities and reasons are in
[selection.json](mozilla/selection.json); names alone never establish trust.

This conservative subset does not reproduce the complete Firefox trust policy.
It does not implement OneCRL, OCSP/CRL fetching, revocation or certificate
transparency. A successful certificate result implies none of those checks and
does not establish page usability or broad public-web compatibility.

The native regression checks pin the exact generated PEM and validate every
selected root's supported format, CA constraints, validity and self-signature.
They also preserve all eight previously bundled DER identities and their key
forms independently of bundle order. The earlier exact G3 provenance remains in
[digicert-global-root-g3-provenance.json](digicert-global-root-g3-provenance.json).
These data checks do not supply an absent peer chain or establish service access.

## Explicit custom stores and verifier bounds

To replace the process's default authorities, edit the local file and restart,
or launch:

```powershell
.\ChezCat.exe --trust-file C:\path\my-roots.pem https://example.org/
```

`--trust-file` replaces the default; it does not merge stores or change Windows.
The file must be a regular file of at most 2 MiB, containing 1–1024 PEM
certificate blocks before exact-DER deduplication. The reusable PEM decoder
retains its separate 12 MiB input bound, each DER certificate is at most 64 KiB,
the presented server chain has at most eight certificates, and at most eight
matching root candidates undergo signature validation. Storage grows with input;
the root limit does not preallocate a maximum-sized store.

The verifier supports v3 certificates with RSA 2048–4096-bit `rsaEncryption`
keys or named P-256/P-384 EC keys in uncompressed form. Certificate signatures
may use RSA PKCS1 v1.5, RSA-PSS or ECDSA with SHA-256/384/512. Every custom root
is parsed before path selection; unsupported entries are rejected when the
HTTPS path is validated, never silently discarded by default-data filtering.
An unreadable or malformed explicit PEM file stops startup. An unavailable
default leaves local pages and HTTP usable while HTTPS reports the trust error.

The validated ordered prefix must reach an exact stored authority or a last
certificate signed by an eligible stored root. Extra supported cross-signs do
not force a valid prefix toward another root. All supplied peer certificates
are parsed and duplicate peer certificates rejected. Selected roots retain
their CA, time, usage and self-signature checks; SAN identity, signature and
constraint checks are unchanged. TLS 1.2/1.3 support remains a bounded subset.

Ctrl+I reports the selected file and PEM SHA-256, successful leaf/root DER
fingerprints, selected prefix and unused suffix lengths. Failed navigation is
reported separately from the retained page. Store changes take effect after
restart.

## Reviewed offline refresh

Root data is refreshed explicitly, not by the running browser. Pin a reviewed
Mozilla source revision and hashes, retain its notices, and review the associated
external constraints. Re-audit all server-auth authorities against the exact
original verifier inputs; new unsupported fields, unknown trust states or
unreviewed authorities must fail generation. Update the reviewed policy instead
of dropping restrictions or silently accepting unsupported certificate forms.

After review, `python tools/generate_trust.py --generate` writes the deterministic
bundle and selection manifest; `python tools/generate_trust.py` checks their
exact committed bytes without downloading or installing anything. Refresh the
native bundle-identity expectation and run all-root CA/signature/time checks,
the retained identity/isolation negatives, signed X.509/TLS fixtures and the
normal offline verification gate. Live website behavior requires its separately
coordinated local check; a data refresh alone proves no live outcome.

`tools/https-demo/demo-roots.pem` remains a separate original test-authority
bundle selected only by local demo/test processes. Its RSA and P-384 roots are
not part of the default store and must not be added to ordinary trust.
