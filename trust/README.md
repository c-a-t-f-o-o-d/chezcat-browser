# Explicit HTTPS trust

The browser reads `trust/roots.pem` beside the executable once at startup.
This file contains public certificate **data**, not a TLS or certificate-library
implementation. The browser's original X.509, RSA and ECDSA code validates the chain.
There is no operating-system trust-store lookup, authority discovery, download,
automatic root update, or certificate-error bypass.

The default file contains these eight self-signed authorities:

| Certificate | Public key | DER SHA-256 | Expires (UTC) |
| --- | ---: | --- | --- |
| ISRG Root X1 | RSA 4096 | `96bcec06264976f37460779acf28c5a7cfe8a3c0aae11a8ffcee05c0bddf08c6` | 2035-06-04 11:04:38 |
| DigiCert Global Root G2 | RSA 2048 | `cb3ccbb76031e5e0138f8dd39a23f9de47ffc35e43c1144cea27d46a5ab1cb5f` | 2038-01-15 12:00:00 |
| Amazon Root CA 1 | RSA 2048 | `8ecde6884f3d87b1125ba31ac3fcb13d7016de7f57cc904fe1cb97c6ae98196e` | 2038-01-17 00:00:00 |
| ISRG Root X2 | ECDSA P-384 | `69729b8e15a86efc177a57afb7171dfc64add28c2fca8cf1507e34453ccb1470` | 2040-09-17 16:00:00 |
| GTS Root R1 | RSA 4096 | `d947432abde7b7fa90fc2e6b59101b1280e0e1c7e4e40fa3c6887fff57a7f4cf` | 2036-06-22 00:00:00 |
| GTS Root R3 | ECDSA P-384 | `34d8a73ee208d9bcdb0d956520934b4e40e69482596e8b6f73c8426b010a6f48` | 2036-06-22 00:00:00 |
| GTS Root R4 | ECDSA P-384 | `349dfa4058c5e263123b398ae795573c4e1313c83fe68f93556cd5e8031b3c7d` | 2036-06-22 00:00:00 |
| DigiCert Global Root G3 | ECDSA P-384 | `31ad6648f8104138c738f39ea4320133393e3a18cc02296ef97c2ac9ef6731d0` | 2038-01-15 12:00:00 |

Expiry means the certificate's `notAfter`, not a browser root program's trust-retirement date.
The GTS entries are the self-signed **with digitalSignature** variants; other
certificates with the same CA names can have different DER fingerprints.

The first three self-signatures use RSA PKCS1 v1.5/SHA-256. GTS Root R1 uses
RSA PKCS1 v1.5/SHA-384; the P-384 roots use ECDSA/SHA-384. A supported path may
mix RSA and EC keys, because each certificate is verified with its issuer's key.

The certificate data was selected from the development operating system's
public certificate bundle and matched by exact DER SHA-256 against each issuer's
published fingerprint. Issuer references:

- [ISRG certificate policy, root fingerprints](https://letsencrypt.org/documents/isrg-cp-v3.3/)
- [Let's Encrypt certificates and chains](https://letsencrypt.org/certificates/)
- [DigiCert trusted root certificates](https://knowledge.digicert.com/general-information/digicert-trusted-root-authority-certificates)
- [Amazon Trust Services repository](https://www.amazontrust.com/repository/)
- [Google Trust Services repository](https://pki.goog/repository/)
- [GTS CPS 5.22, exact certificate variants and fingerprints](https://pki.goog/repo/cps/5.22/GTS-CPS.html)

To choose your own authorities, edit this PEM file and restart, or launch:

```powershell
.\ChezCat.exe --trust-file C:\path\my-roots.pem https://example.org/
```

`--trust-file` **replaces** the default authorities for that process. It does not
merge them or install anything into Windows. Trust files must be regular files,
at most 2 MiB, and contain 1–128 PEM certificates. Exact duplicate certificates
are deduplicated. The current verifier supports v3 certificates with RSA
2048–4096-bit `rsaEncryption` keys or named P-256/P-384 EC keys in uncompressed
form. Certificate signatures can use RSA PKCS1 v1.5, RSA-PSS or ECDSA with
SHA-256/384/512. Unsupported certificates in a custom file cause validation to
fail rather than being silently discarded.

Authority names alone do not confer trust. A validated ordered prefix must end
at the exact trusted certificate, or its last certificate must be signed by an
eligible stored root. The verifier tries stored roots at each validated prefix,
so an unnecessary supported cross-sign after that point does not force an
otherwise valid path toward a different authority. It still parses every
supplied certificate and rejects duplicates before selecting the path.
The root's constraints, validity and self-signature are also checked. These are
an initial set chosen for this build; they are not a complete public-web root
program. A trusted issuer does not make every possible server configuration
compatible with this browser's limited TLS and certificate algorithms.

Ctrl+I shows the selected file and its SHA-256, and the leaf/root DER fingerprints
for successful HTTPS requests, plus the selected path and unused suffix lengths.
The last failed navigation is listed separately from the retained page. File
changes take effect only after restart.
An unavailable default file leaves local pages and HTTP working; HTTPS reports
the trust configuration error. An unreadable or malformed explicit PEM file
stops startup. Certificate-policy errors are reported when validating an HTTPS
connection.

`tools/https-demo/demo-roots.pem` is a separate bundle of two publicly shipped
**test** authorities: the original RSA root from `ca.pem` and the P-384 root
from `ec-ca.pem`. `Run-Https-Browser.cmd` selects this combined file only for the
local demo process. The default demo server has a P-256 identity; its optional
RSA and P-384 identities use the same bundle. The original `ca.pem` remains
valid for the older RSA fixtures. None of these test roots is included in
`trust/roots.pem` or installed into Windows. Do not add them to ordinary trust.

Revocation, OCSP/CRL fetching and certificate transparency are not implemented.
No revocation or CT status is implied by the status bar's certificate result.

## 0.51 explicit addition

The G3 DER fingerprint was checked against DigiCert's published root list on
2026-11-12. Its subject-name DER SHA-256 exactly matches the last issuer-name
hash in the user's failing www.bing.com path. The first seven certificates are
unchanged. See `digicert-global-root-g3-provenance.json` for fingerprints and the
source of these certificate data. OpenSSL independently verified its self-signature.

This is an explicit addition to the bundled trust policy, not automatic trust
of certificates sent by a server. It can validate only a correctly signed path
satisfying the existing hostname, time, constraints and algorithm checks. The
reported peer DER files were not supplied, so the full Bing path still needs a
fresh capture. Certificate validation, revocation/CT limitations and custom
`--trust-file` replacement semantics remain unchanged. Restart the browser to
load the updated bundle; the activity report should list eight roots.
