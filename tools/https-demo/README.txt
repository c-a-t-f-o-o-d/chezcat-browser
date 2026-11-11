LOCAL HTTPS TEST FIXTURES - CHEZCAT

The demo server defaults to a P-256 ECDSA identity. It also supports RSA and
P-384 identities. The browser's TLS 1.3 subset supports ChaCha20-Poly1305,
AES-128-GCM and AES-256-GCM, with X25519 key exchange.

RSA DEMO PATH
ca.pem is a locally generated RSA-2048 test root.
server-chain.pem contains a localhost leaf and an RSA-2048 intermediate.
server-key.pem is the PUBLICLY SHIPPED TEST PRIVATE KEY for that leaf.
This path uses RSA/SHA-256 signatures.

RSA test root DER SHA-256:
9bab69c2933e6d8d527d7c75b5b958e056dfce8df9159b378ce4a86cc8004ddc

ECDSA DEMO PATHS
ec-ca.pem is a separate locally generated P-384 test root.
server-p256-chain.pem contains a P-256 localhost leaf and a P-384 intermediate.
server-p384-chain.pem contains a P-384 localhost leaf and a P-384 intermediate.
server-p256-key.pem and server-p384-key.pem are the PUBLICLY SHIPPED TEST
PRIVATE KEYS for their respective leaves. These paths use ECDSA/SHA-384
certificate signatures.

P-384 test root DER SHA-256:
39e3e44094aa658724f44b654f69a2d0d52982a5822b534f8b166adcf66d3a4b

All three leaf identities cover localhost and 127.0.0.1. All demo certificates
are valid from 2025-01-01 through 2035-01-01 UTC.

RUN THE DEMO
From the project root, run tools\demos\Run-Https-Demo.cmd. It starts the server and opens the correctly trusted demo browser.
For TLS 1.2, use: tools\demos\Run-Https-Demo.cmd --tls-version 1.2
For the separate two-step workflow, add --server-only then run tools\demos\Run-Https-Browser.cmd.
The browser launcher explicitly selects demo-roots.pem, which contains only
ca.pem and ec-ca.pem, in place of the normal public trust roots.
The server binds only to 127.0.0.1. Python is needed only for this test server.

These retained demos exercise original JPEG decoding and CSS positioning.
Open either through HTTPS:

  https://localhost:8766/compressed-pages/jpeg-demo.html
  https://localhost:8766/compressed-pages/position-demo.html

Gzip and zlib-wrapped deflate still work inside HTTPS. Ctrl+I shows the content
codings and encoded/decoded byte counts. The earlier compressed flex demo remains
at https://localhost:8766/compressed-pages/flex-demo.html. Neither Pillow nor an
external JPEG decoder is used by the browser; Python serves these local fixtures.

To choose another identity, stop the demo server and run one of these commands
from the project folder; the command opens the matching demo browser:

  python tools/https_fixture_server.py --identity rsa
  python tools/https_fixture_server.py --identity p256
  python tools/https_fixture_server.py --identity p384

The default is --identity p256. Each identity uses the same local HTTPS address
and the same demo-roots.pem browser trust file.

Do not install these CAs in an operating-system trust store, add them to the
normal trust/roots.pem, or use the test keys for real servers or private data.
These files were generated for this project's independent interoperability
checks; they contain no third-party browser or TLS implementation code.
