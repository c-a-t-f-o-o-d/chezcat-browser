<div align="center">

# Accepting CSAPUL 1.1

*An explicit offline choice, with a minimal local receipt.*

[Home](../README.md) · [Documentation](README.md) · [License](../LICENSE) · [Contributing](../CONTRIBUTING.md)

</div>

---

> [!NOTE]
> The implemented commands and receipt locations are documented in the
> [build guide](BUILDING.md#license-receipts-and-command-line-use). The policy text
> below is preserved; this presentation update does not change the terms.

The exact root LICENSE and LICENSE_SCOPE.md control. Acceptance is not copyright
registration, proof that all material is copyrightable, or permission to bypass
website terms, access controls or independent third-party rights.

## Ordinary use

The official first-run flow must offer the complete license offline and an explicit
“I agree to CSAPUL 1.1” action. Declining exits without starting browsing or network
requests. No checkbox is preselected and no automatic acceptance is inferred from
installation, download, a timer or a browser profile. A person exercising only
independently held rights is not required by CSAPUL to accept additional terms.

## Source, headless and organization use

An explicit acceptance command identifies the license version and full SHA-256
and records acceptance for the stated personal or authorized organization scope.
The build/headless tool must not silently select acceptance. An authorized
organization representative can accept for a covered deployment and its automated
builds/tests without collecting every employee's identity. An independent IT
contractor still accepts for its own permitted engagement under section 3.3.
Document the actual command in BUILDING.md once implemented; this policy alone
does not establish that a particular build contains the controls.

## Local receipt and changes

Store only schema version, exact license version and digest, the accepted scope,
and acceptance time locally. Do not require email, an account, telemetry, online
activation, an acceptance server or a hardware fingerprint. A copied repository
receipt is not evidence of the new recipient's assent; do not ship receipts in
source, CI artifacts, profiles or release bundles. Protected local deployment
configuration may record an authorized administrator's acceptance within its scope.

Acceptance may cover the identified release and later officially designated
Covered Releases carrying the same exact license text and digest. A product
version change alone does not require another acceptance. Changed license terms
require a new affirmative acceptance. A corrupt or missing local receipt calls for
an explicit acceptance flow, not fabrication of one; recovering or withdrawing a
receipt does not alter independently held rights or already-executed legal records.

The local receipt is deliberately minimal. It does not prove who accepted for an
organization, guarantee legal enforceability, or replace a separately signed
contributor agreement or special permission.
