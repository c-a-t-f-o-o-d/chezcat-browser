# Original TLS SAN rejection fixture

These are public TEST materials for an isolated loopback server. The RSA2048
root signs a P-256 server leaf whose only SAN is `DNS:wrong.invalid`; it deliberately
does not identify `127.0.0.1` or `localhost`. Both certificates are valid from
2020-01-01 through2035-01-01, with explicit CA/key-usage/server-auth constraints.

The normal harness supplies `root.pem` only to that process using `--trust-file`.
Never add it to normal browser or OS trust, and never use the public TEST leaf
private key outside this fixture. The root signing key is not retained.

`provenance.json` records DER and file hashes, original generation profile and
the development OpenSSL version. Optional `make_certificates.py --output NEW_DIR`
regenerates the profile with new random keys; it does not reproduce identical
bytes. Development OpenSSL is not a browser or normal-verification dependency.
No external certificate or key input was used.
