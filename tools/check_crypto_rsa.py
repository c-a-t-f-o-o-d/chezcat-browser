#!/usr/bin/env python3
"""Offline, independent SHA-384/SHA-512 and RSA verification checks.

Run from any directory:
    python tools/check_crypto_rsa.py [--rustc /path/to/rustc]

Python cryptography and a host Rust compiler are development-only requirements.
The browser does not import, invoke, or link this script. No network is used.
Published public vectors are from RFC 7515 A.2 and RFC 9421 B.2.1. New ephemeral
keys/signatures come from cryptography; Python pow constructs malformed encoded
messages and independently checks public arithmetic. Private operations appear
only in this development checker. Temporary files are removed on completion.
"""

import argparse
import base64
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa

ROOT = Path(__file__).resolve().parents[1]
HASHES = {"256": hashes.SHA256, "384": hashes.SHA384, "512": hashes.SHA512}

# Published public test data; no implementation source is imported.
# https://www.rfc-editor.org/rfc/rfc7515.html#appendix-A.2
JWS_N = (
    "ofgWCuLjybRlzo0tZWJjNiuSfb4p4fAkd_wWJcyQoTbji9k0l8W26mPddx"
    "HmfHQp-Vaw-4qPCJrcS2mJPMEzP1Pt0Bm4d4QlL-yRT-SFd2lZS-pCgNMs"
    "D1W_YpRPEwOWvG6b32690r2jZ47soMZo9wGzjb_7OMg0LOL-bSf63kpaSH"
    "SXndS5z5rexMdbBYUsLA9e-KXBdQOS-UTo7WTBEMa2R2CapHg665xsmtdV"
    "MTBQY4uDZlxvb3qCo5ZwKh9kG4LT6_I5IhlJH7aGhyxXFvUK-DWNmoudF8"
    "NAco9_h9iaGNj8q2ethFkMLs91kzk2PAcDTW9gb54h4FRWyuXpoQ"
)
JWS_MESSAGE = (
    b"eyJhbGciOiJSUzI1NiJ9."
    b"eyJpc3MiOiJqb2UiLA0KICJleHAiOjEzMDA4MTkzODAsDQogImh0dHA6Ly9leGFt"
    b"cGxlLmNvbS9pc19yb290Ijp0cnVlfQ"
)
JWS_SIGNATURE = (
    "cC4hiUPoj9Eetdgtv3hF80EGrhuB__dzERat0XF9g2VtQgr9PJbu3XOiZj5RZmh7"
    "AAuHIm4Bh-0Qc_lF5YKt_O8W2Fp5jujGbds9uJdbF9CUAr7t1dnZcAcQjbKBYNX4"
    "BAynRFdiuB--f_nZLgrnbyTyWzO75vRK5h6xBArLIARNPvkSjtQBMHlb1L07Qe7K"
    "0GarZRmB_eSN9383LcOLn6_dO--xi12jzDwusC-eOkHWEsqtFZESc6BfI7noOPqv"
    "hJ1phCnvWh6IeYI2w9QOYEUipUTI8np6LbgGY9Fs98rqVt5AXLIhWkWywlVmtVrB"
    "p0igcN_IoypGlUPQGe77Rw"
)
# https://www.rfc-editor.org/rfc/rfc9421.html#appendix-B.2.1
PSS_N = (
    "r4tmm3r20Wd_PbqvP1s2-QEtvpuRaV8Yq40gjUR8y2Rjxa6dpG2GXHbPfvM"
    "s8ct-Lh1GH45x28Rw3Ry53mm-oAXjyQ86OnDkZ5N8lYbggD4O3w6M6pAvLkhk95An"
    "dTrifbIFPNU8PPMO7OyrFAHqgDsznjPFmTOtCEcN2Z1FpWgchwuYLPL-Wokqltd11"
    "nqqzi-bJ9cvSKADYdUAAN5WUtzdpiy6LbTgSxP7ociU4Tn0g5I6aDZJ7A8Lzo0KSy"
    "ZYoA485mqcO0GVAdVw9lq4aOT9v6d-nb4bnNkQVklLQ3fVAvJm-xdDOp9LCNCN48V"
    "2pnDOkFV6-U9nV5oyc6XI2w"
)
PSS_MESSAGE = (
    b'"@signature-params": ();created=1618884473;keyid="test-key-rsa-pss"'
    b';nonce="b3k2pp5k7z-50gnwp.yemd"'
)
PSS_SIGNATURE = (
    "d2pmTvmbncD3xQm8E9ZV2828BjQWGgiwAaw5bAkgibUopem"
    "LJcWDy/lkbbHAve4cRAtx31Iq786U7it++wgGxbtRxf8Udx7zFZsckzXaJMkA7ChG"
    "52eSkFxykJeNqsrWH5S+oxNFlD4dzVuwe8DhTSja8xxbR/Z2cOGdCbzR72rgFWhzx"
    "2VjBqJzsPLMIQKhO4DGezXehhWwE56YCE+O6c0mKZsfxVrogUvA4HELjVKWmAvtl6"
    "UnCh8jYzuVG5WSb/QEVPnP5TmcAnLH1g+s++v6d4s8m0gCw1fV5/SITLq9mhho8K3"
    "+7EPYTU8IU1bLhdxO5Nyt8C8ssinQ98Xw9Q=="
)

PROBE = r'''
#![allow(dead_code)]
#[path=HASH] mod crypto_hash;
#[path=SHA512] mod crypto_sha512;
#[path=RSA] mod crypto_rsa;
use std::io::{self,BufRead,Write};
fn unhex(s:&str)->Vec<u8>{s.as_bytes().as_chunks::<2>().0.iter().map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()}
fn hex(v:&[u8])->String{let mut s=String::new();for b in v{use std::fmt::Write;write!(&mut s,"{b:02x}").unwrap();}s}
fn hash(s:&str)->crypto_rsa::Hash{match s{"256"=>crypto_rsa::Hash::Sha256,"384"=>crypto_rsa::Hash::Sha384,"512"=>crypto_rsa::Hash::Sha512,_=>panic!()}}
fn ok(result:Result<(),String>)->String{if result.is_ok(){"OK".into()}else{"ERR".into()}}
fn main(){let mut out=io::BufWriter::new(io::stdout().lock());for line in io::stdin().lock().lines(){let line=line.unwrap();let f:Vec<_>=line.split('\t').collect();let answer=match f[0]{
"hash384"=>hex(&crypto_sha512::sha384(&unhex(f[1]))),
"hash512"=>hex(&crypto_sha512::sha512(&unhex(f[1]))),
"key"=>ok(crypto_rsa::validate_key(&unhex(f[1]),&unhex(f[2]))),
"raw"=>crypto_rsa::oracle_public(&unhex(f[1]),&unhex(f[2]),&unhex(f[3])).map(|v|hex(&v)).unwrap_or_else(|_|"ERR".into()),
"v15"=>ok(crypto_rsa::verify_pkcs1v15(&unhex(f[1]),&unhex(f[2]),hash(f[3]),&unhex(f[4]),&unhex(f[5]))),
"pss"=>ok(crypto_rsa::verify_pss(&unhex(f[1]),&unhex(f[2]),hash(f[3]),if f[4]=="max"{usize::MAX}else{f[4].parse().unwrap()},&unhex(f[5]),&unhex(f[6]))),
_=>panic!("unknown operation")};writeln!(out,"{answer}").unwrap();}}
'''


def b64(text):
    return base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))


def magnitude(value, length=None):
    return value.to_bytes(length if length is not None else max(1, (value.bit_length() + 7) // 8), "big")


def mgf1(seed, length, name):
    return b"".join(hashlib.new("sha" + name, seed + i.to_bytes(4, "big")).digest()
                    for i in range((length + int(name) // 8 - 1) // (int(name) // 8)))[:length]


def oracle_valid(public, scheme, name, salt, message, signature):
    algorithm = HASHES[name]()
    pad = padding.PKCS1v15() if scheme == "v15" else padding.PSS(padding.MGF1(algorithm), salt)
    try:
        public.verify(signature, message, pad, algorithm)
        return True
    except (InvalidSignature, ValueError):
        return False


def ephemeral_key(bits, exponent):
    if bits != 2049:
        return rsa.generate_private_key(public_exponent=exponent, key_size=bits)
    # Some OpenSSL versions round an odd key-size request down to an even
    # number. Compose independently generated 1025- and 1024-bit primes, then
    # have cryptography validate the private key used ONLY by this checker.
    for _ in range(8):
        p = rsa.generate_private_key(public_exponent=exponent, key_size=2050).private_numbers().p
        q = rsa.generate_private_key(public_exponent=exponent, key_size=2048).private_numbers().q
        if (p * q).bit_length() == bits:
            d = pow(exponent, -1, (p - 1) * (q - 1))
            numbers = rsa.RSAPrivateNumbers(p, q, d, d % (p - 1), d % (q - 1),
                                           pow(q, -1, p), rsa.RSAPublicNumbers(exponent, p * q))
            return numbers.private_key()
    raise RuntimeError("Could not construct the 2049-bit development fixture.")


def run(rustc):
    started = time.monotonic()
    rng = random.Random(0x525341353132)
    cases = []

    def add(label, operation, fields, expected):
        encoded = [field.hex() if isinstance(field, bytes) else str(field) for field in fields]
        answer = expected.hex() if isinstance(expected, bytes) else expected
        cases.append((label, "\t".join([operation] + encoded), answer))

    def signature_case(label, scheme, n, e, name, salt, message, signature, valid):
        fields = [n, e, name] + ([salt] if scheme == "pss" else []) + [message, signature]
        add(label, scheme, fields, "OK" if valid else "ERR")

    for scheme, n, name, salt, message, sig in [
        ("v15", b64(JWS_N), "256", 0, JWS_MESSAGE, b64(JWS_SIGNATURE)),
        ("pss", b64(PSS_N), "512", 64, PSS_MESSAGE, b64(PSS_SIGNATURE)),
    ]:
        public = rsa.RSAPublicNumbers(65537, int.from_bytes(n, "big")).public_key()
        assert oracle_valid(public, scheme, name, salt, message, sig)
        signature_case("published " + scheme, scheme, n, b"\x01\x00\x01", name, salt, message, sig, True)
        signature_case("published wrong message", scheme, n, b"\x01\x00\x01", name, salt, message + b"!", sig, False)

    lengths = list(range(0, 260)) + [511, 512, 513, 1023, 1024, 1025, 4097, 65536, 1_048_576]
    for length in lengths:
        data = rng.randbytes(length)
        for name in ["384", "512"]:
            add("SHA padding/length boundary", "hash" + name, [data], hashlib.new("sha" + name, data).digest())

    # Public arithmetic is meaningful for every odd modulus, even composite
    # values that are deliberately not generated as RSA keys. Include partial
    # words, the emLen=k-1 boundary, full carry chains, and dense exponents.
    for bits in [2048, 2049, 2050, 2055, 2056, 2079, 2080, 2081, 3072, 4095, 4096]:
        for n in [(1 << (bits - 1)) | 1, (1 << bits) - 1, rng.getrandbits(bits) | (1 << (bits - 1)) | 1]:
            size = (bits + 7) // 8
            for e in [3, 17, 65537, 0xffff_ffff]:
                for s in [0, 1, n - 1, rng.randrange(n)]:
                    add("Montgomery vs Python pow", "raw", [magnitude(n), magnitude(e), magnitude(s, size)], magnitude(pow(s, e, n), size))

    base_n = b"\xff" * 256
    for bits in [0, 1, 2047, 2048, 2049, 4096, 4097]:
        n = magnitude(((1 << (bits - 1)) | 1) if bits else 0)
        add("modulus bit policy", "key", [n, b"\x03"], "OK" if 2048 <= bits <= 4096 else "ERR")
    for n, e in [(b"", b"\x03"), (b"\0" + base_n, b"\x03"), (base_n[:-1] + b"\xfe", b"\x03")]:
        add("invalid modulus encoding", "key", [n, e], "ERR")
    for e in [b"", b"\0", b"\x01", b"\x02", b"\0\x03", b"\x01\0\0\0\x01", b"\xff\xff\xff\xfe"]:
        add("invalid public exponent", "key", [base_n, e], "ERR")
        for scheme in ["v15", "pss"]:
            signature_case("invalid key on verify", scheme, base_n, e, "256", 32, b"", bytes(256), False)
    for e in [b"\x03", b"\x11", b"\x01\0\x01", b"\xff" * 4]:
        add("valid public exponent policy", "key", [base_n, e], "OK")
    signature_case("salt usize maximum", "pss", base_n, b"\x03", "256", "max", b"", bytes(256), False)

    for bits, exponent in [(2048, 3), (2048, 65537), (2049, 65537), (3072, 65537), (4096, 65537)]:
        key = ephemeral_key(bits, exponent)
        private = key.private_numbers()
        public = key.public_key()
        n, e = private.public_numbers.n, private.public_numbers.e
        assert n.bit_length() == bits
        nb, eb = magnitude(n), magnitude(e)
        size, emlen = len(nb), (bits - 1 + 7) // 8
        add("generated valid RSA key", "key", [nb, eb], "OK")
        for name, factory in HASHES.items():
            algorithm = factory()
            hlen = algorithm.digest_size
            for message in [b"", b"abc", rng.randbytes(4097)]:
                for scheme, salt in [("v15", 0), ("pss", 0), ("pss", hlen), ("pss", emlen - hlen - 2)]:
                    pad = padding.PKCS1v15() if scheme == "v15" else padding.PSS(padding.MGF1(algorithm), salt)
                    sig = key.sign(message, pad, algorithm)
                    assert oracle_valid(public, scheme, name, salt, message, sig)
                    signature_case("cryptography valid", scheme, nb, eb, name, salt, message, sig, True)
                    changed = sig[:-1] + bytes([sig[-1] ^ 1])
                    assert not oracle_valid(public, scheme, name, salt, message, changed)
                    signature_case("signature bit changed", scheme, nb, eb, name, salt, message, changed, False)
                    signature_case("message changed", scheme, nb, eb, name, salt, message + b"!", sig, False)
                    if scheme == "pss":
                        signature_case("salt length mismatch", scheme, nb, eb, name, salt + 1, message, sig, False)
            for scheme in ["v15", "pss"]:
                for sig in [b"", bytes(size - 1), bytes(size + 1), bytes(size), magnitude(1, size), nb, magnitude(n + 1, size)]:
                    signature_case("signature length/range/zero", scheme, nb, eb, name, hlen, b"test", sig, False)
            mixed = padding.PSS(padding.MGF1(hashes.SHA384() if name != "384" else hashes.SHA256()), hlen)
            sig = key.sign(b"mixed MGF", mixed, algorithm)
            signature_case("MGF1 hash mismatch", "pss", nb, eb, name, hlen, b"mixed MGF", sig, False)

            # Build invalid EMSA messages that nevertheless recover EXACTLY
            # under RSA. Random signature flips alone cannot exercise these.
            if bits not in [2048, 2049] or exponent == 3:
                continue
            message = b"independent strict encoding checks"

            def check_encoded(label, scheme, encoded, salt=hlen):
                integer = int.from_bytes(encoded, "big")
                assert integer < n, label
                sig = magnitude(pow(integer, private.d, n), size)
                assert pow(int.from_bytes(sig, "big"), e, n) == integer
                signature_case(label, scheme, nb, eb, name, salt, message, sig, False)

            sig = key.sign(message, padding.PKCS1v15(), algorithm)
            original = magnitude(pow(int.from_bytes(sig, "big"), e, n), size)
            separator = original.index(b"\0", 2)
            digest_info = original[separator + 1:]
            for index in [1, 2, separator - 1, separator, separator + 2, size - 1]:
                malformed = bytearray(original)
                malformed[index] ^= 1
                check_encoded("PKCS1 invalid prefix/padding/digest", "v15", malformed)
            # A BER long-form length and missing NULL each preserve the hash
            # value but violate this verifier's exact DER DigestInfo contract.
            for tail in [digest_info + b"\0", b"\x30\x81" + digest_info[1:],
                         bytes([0x30, digest_info[1] - 2, 0x30, 0x0b]) + digest_info[4:15] + digest_info[17:],
                         digest_info[-hlen:]]:
                encoded = b"\0\x01" + b"\xff" * (size - len(tail) - 3) + b"\0" + tail
                check_encoded("PKCS1 noncanonical/trailing/raw digest", "v15", encoded)

            sig = key.sign(message, padding.PSS(padding.MGF1(algorithm), hlen), algorithm)
            original = magnitude(pow(int.from_bytes(sig, "big"), e, n), size)[-emlen:]
            data_len = emlen - hlen - 1
            expected_hash = original[data_len:-1]
            mask = mgf1(expected_hash, data_len, name)
            high_mask = 0xff >> (8 * emlen - (bits - 1))
            db = bytearray(a ^ b for a, b in zip(original[:data_len], mask))
            db[0] &= high_mask
            separator = data_len - hlen - 1
            assert db[:separator] == bytes(separator) and db[separator] == 1
            for index, value in [(1, 1), (separator - 1, 0xff), (separator, 0), (separator, 2), (data_len - 1, db[-1] ^ 1)]:
                changed = db.copy()
                changed[index] = value
                masked = bytearray(a ^ b for a, b in zip(changed, mask))
                masked[0] &= high_mask
                check_encoded("PSS invalid PS/marker/salt", "pss", masked + expected_hash + b"\xbc")
            for index in [-1, data_len]:
                changed = bytearray(original)
                changed[index] ^= 1
                check_encoded("PSS invalid trailer/hash", "pss", changed)
            # A value of 2^(modBits-1) is below n but outside emBits. It tests
            # the high-bit rule (2048) or the required extra zero byte (2049).
            check_encoded("PSS encoded message exceeds emBits", "pss", magnitude(1 << (bits - 1), size))

    with tempfile.TemporaryDirectory(prefix="rsa-check-") as temporary:
        temp = Path(temporary)
        # Append a public-arithmetic hook to a TEMPORARY, otherwise byte-exact
        # module copy. The browser's real module has no raw-RSA public API.
        rsa_source = (ROOT / "src/crypto_rsa.rs").read_text(encoding="utf-8")
        rsa_source += "\npub fn oracle_public(n:&[u8],e:&[u8],s:&[u8])->Result<Vec<u8>,String>{public_operation(n,e,s).map(|v|v.bytes[..v.length].to_vec())}\n"
        (temp / "rsa_probe.rs").write_text(rsa_source, encoding="utf-8")
        probe = PROBE.replace("HASH", json.dumps(str(ROOT / "src/crypto_hash.rs"), ensure_ascii=False))
        probe = probe.replace("SHA512", json.dumps(str(ROOT / "src/crypto_sha512.rs"), ensure_ascii=False))
        probe = probe.replace("RSA", json.dumps(str(temp / "rsa_probe.rs"), ensure_ascii=False))
        source = temp / "probe.rs"
        executable = temp / ("probe.exe" if os.name == "nt" else "probe")
        source.write_text(probe, encoding="utf-8")
        subprocess.run([rustc, "--edition=2021", "-O", "-C", "overflow-checks=yes", str(source), "-o", str(executable)], check=True, timeout=60)
        result = subprocess.run([str(executable)], input="\n".join(case[1] for case in cases) + "\n", text=True, capture_output=True, check=True, timeout=120)
        actual = result.stdout.splitlines()
        assert len(actual) == len(cases), (len(actual), len(cases), result.stderr)
        failures = [{"case": label, "expected": expected[:160], "actual": answer[:160]}
                    for (label, _, expected), answer in zip(cases, actual) if answer != expected]
        report = {"cases": len(cases), "operations": dict(Counter(case[1].split("\t", 1)[0] for case in cases)),
                  "failures": failures, "seconds": round(time.monotonic() - started, 2)}
        print(json.dumps(report, indent=2))
        assert not failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"), help="Host compiler (RUSTC or rustc from PATH by default)")
    args = parser.parse_args()
    run(args.rustc)
    print("Independent SHA-384/SHA-512 and RSA checks passed; no browser runtime dependency added.")


if __name__ == "__main__":
    main()
