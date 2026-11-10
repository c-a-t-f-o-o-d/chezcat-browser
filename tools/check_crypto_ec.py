#!/usr/bin/env python3
"""Independent offline checks for the browser's original ECDSA verifier.

Run: python tools/check_crypto_ec.py [--rustc /path/to/rustc]

Development requirements: Python cryptography and a host Rust compiler. No
network, dependency, private-key API, or oracle hook is added to the browser.
The temporary probe appends inspection functions to a temporary copy of our
own module. Python integer arithmetic and affine points independently check
the runtime's Montgomery/Jacobian operations. cryptography supplies a second
signature/point oracle. Published public vectors are RFC 6979 A.2.5/A.2.6.
"""

import argparse
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
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, utils

ROOT = Path(__file__).resolve().parents[1]

# Mathematical curve constants from NIST SP 800-186 sections 3.2.1.3/3.2.1.4.
CURVES = {
    "256": {
        "curve": ec.SECP256R1,
        "p": int("FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF", 16),
        "n": int("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16),
        "b": int("5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B", 16),
        "g": (int("6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296", 16),
              int("4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5", 16)),
    },
    "384": {
        "curve": ec.SECP384R1,
        "p": int("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFFFF0000000000000000FFFFFFFF", 16),
        "n": int("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC7634D81F4372DDF581A0DB248B0A77AECEC196ACCC52973", 16),
        "b": int("B3312FA7E23EE7E4988E056BE3F82D19181D9C6EFE8141120314088F5013875AC656398D8A2ED19D2A85C8EDD3EC2AEF", 16),
        "g": (int("AA87CA22BE8B05378EB1C71EF320AD746E1D3B628BA79B9859F741E082542A385502F25DBF55296C3A545E3872760AB7", 16),
              int("3617DE4A96262C6F5D9E98BF9292DC29F8F41DBD289A147CE9DA3113B5F0B8C00A60B1CE1D7E819D7A431D7C90EA0E5F", 16)),
    },
}

PUBLISHED = [
    ("256", "sha256",
     "0460FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB67903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299",
     "EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716",
     "F7CB1C942D657C41D436C7A1B6E29F65F3E900DBB9AFF4064DC4AB2F843ACDA8"),
    ("384", "sha384",
     "04EC3A4E415B4E19A4568618029F427FA5DA9A8BC4AE92E02E06AAE5286B300C64DEF8F0EA9055866064A254515480BC138015D9B72D7D57244EA8EF9AC0C621896708A59367F9DFB9F54CA84B3F1C9DB1288B231C3AE0D4FE7344FD2533264720",
     "94EDBB92A5ECB8AAD4736E56C691916B3F88140666CE9FA73D64C4EA95AD133C81A648152E44ACF96E36DD1E80FABE46",
     "99EF4AEB15F178CEA1FE40DB2603138F130E740A19624526203B6351D0A3A94FA329C145786E679E7B82C71A38628AC8"),
]

HOOKS = r'''
fn oracle_bytes(value:&Number, words:usize)->Vec<u8> {
    (0..words*8).rev().map(|i|(value[i/8]>>(8*(i%8)))as u8).collect()
}
pub fn oracle_mod(curve:Curve, scalar:bool, op:&str, a:&[u8], b:&[u8])->Vec<u8> {
    let p=parameters(curve);let m=if scalar{&p.order}else{&p.field};
    let a=m.encode(&from_be(a));let b=m.encode(&from_be(b));
    let result=match op{"add"=>m.add(&a,&b),"sub"=>m.subtract(&a,&b),
        "mul"=>m.multiply(&a,&b),"inv"=>m.inverse(&a),_=>panic!()};
    oracle_bytes(&m.decode(&result),m.words)
}
pub fn oracle_point(curve:Curve,u:&[u8],v:&[u8],q:&[u8])->Vec<u8> {
    let p=parameters(curve);let q=p.point(q).unwrap();
    let point=p.combination(&from_be(u),&from_be(v),&q);
    if point.z==ZERO{return vec![0]}
    let f=&p.field;let inv=f.inverse(&point.z);let inv2=f.square(&inv);
    let x=f.decode(&f.multiply(&point.x,&inv2));
    let y=f.decode(&f.multiply(&point.y,&f.multiply(&inv2,&inv)));
    let mut result=vec![4];result.extend(oracle_bytes(&x,f.words));
    result.extend(oracle_bytes(&y,f.words));result
}
'''

PROBE = r'''
#![allow(dead_code)]
#[path=EC] mod crypto_ec;
use std::io::{self,BufRead};
fn unhex(s:&str)->Vec<u8>{s.as_bytes().as_chunks::<2>().0.iter().map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect()}
fn hex(bytes:&[u8])->String{let mut out=String::new();for b in bytes{use std::fmt::Write;write!(&mut out,"{b:02x}").unwrap();}out}
fn valid(result:Result<(),String>)->String{if result.is_ok(){"OK".into()}else{"ERR".into()}}
fn main(){for line in io::stdin().lock().lines(){let line=line.unwrap();let f:Vec<_>=line.split('\t').collect();
    let c=match f[1]{"256"=>crypto_ec::Curve::P256,"384"=>crypto_ec::Curve::P384,_=>panic!()};
    let answer=match f[0]{
      "key"=>valid(crypto_ec::validate_key(c,&unhex(f[2]))),
      "sig"=>valid(crypto_ec::verify(c,&unhex(f[2]),&unhex(f[3]),&unhex(f[4]))),
      "point"=>hex(&crypto_ec::oracle_point(c,&unhex(f[2]),&unhex(f[3]),&unhex(f[4]))),
      "mod"=>hex(&crypto_ec::oracle_mod(c,f[2]=="n",f[3],&unhex(f[4]),&unhex(f[5]))),
      _=>panic!()};println!("{answer}");}}
'''


def affine_add(p, left, right):
    if left is None:
        return right
    if right is None:
        return left
    x1, y1 = left
    x2, y2 = right
    if x1 == x2:
        if (y1 + y2) % p == 0:
            return None
        slope = (3 * x1 * x1 - 3) * pow(2 * y1, -1, p) % p
    else:
        slope = (y2 - y1) * pow(x2 - x1, -1, p) % p
    x3 = (slope * slope - x1 - x2) % p
    return x3, (slope * (x1 - x3) - y1) % p


def affine_multiply(p, scalar, point):
    result = None
    while scalar:
        if scalar & 1:
            result = affine_add(p, result, point)
        point = affine_add(p, point, point)
        scalar >>= 1
    return result


def point_bytes(point, width):
    return b"\0" if point is None else b"\4" + b"".join(c.to_bytes(width, "big") for c in point)


def digest_algorithm(digest):
    return {32: hashes.SHA256, 48: hashes.SHA384, 64: hashes.SHA512}[len(digest)]()


def oracle_signature(curve, key, digest, signature):
    try:
        public = ec.EllipticCurvePublicKey.from_encoded_point(curve(), key)
        public.verify(signature, digest, ec.ECDSA(utils.Prehashed(digest_algorithm(digest))))
        return True
    except (ValueError, InvalidSignature):
        return False


def run(rustc):
    started = time.monotonic()
    rng = random.Random(0x4543445341333834)
    cases = []

    def add(label, operation, curve, fields, expected):
        fields = [value.hex() if isinstance(value, bytes) else str(value) for value in fields]
        if isinstance(expected, bool):
            expected = "OK" if expected else "ERR"
        if isinstance(expected, bytes):
            expected = expected.hex()
        cases.append((label, operation, "\t".join([operation, curve] + fields), expected))

    for curve_id, hash_name, key_hex, r, s in PUBLISHED:
        key = bytes.fromhex(key_hex)
        digest = hashlib.new(hash_name, b"sample").digest()
        signature = utils.encode_dss_signature(int(r, 16), int(s, 16))
        assert oracle_signature(CURVES[curve_id]["curve"], key, digest, signature)
        add("published RFC6979 " + curve_id, "sig", curve_id, [key, digest, signature], True)

    for curve_id, parameters in CURVES.items():
        width = int(curve_id) // 8
        p, n, g = parameters["p"], parameters["n"], parameters["g"]
        magnitude = lambda value: value.to_bytes(width, "big")
        generator = point_bytes(g, width)
        add("generator valid", "key", curve_id, [generator], True)
        for modulus_name in ["p", "n"]:
            modulus = parameters[modulus_name]
            values = [0, 1, 2, modulus - 2, modulus - 1]
            for bit in range(32, int(curve_id), 32):
                values.extend([(1 << bit) - 1, 1 << bit, (1 << bit) + 1])
            values.extend(rng.randrange(modulus) for _ in range(64))
            pairs = [(a, b) for a in values[:5] for b in values[:5]]
            pairs += [(a, modulus - 1 - a) for a in values]
            pairs += [(a, rng.randrange(modulus)) for a in values]
            for a, b in pairs:
                for operation, value in [("add", (a + b) % modulus),
                                         ("sub", (a - b) % modulus),
                                         ("mul", a * b % modulus)]:
                    add("modular " + operation, "mod", curve_id,
                        [modulus_name, operation, magnitude(a), magnitude(b)], magnitude(value))
            for a in values[1::3]:
                add("modular inverse", "mod", curve_id,
                    [modulus_name, "inv", magnitude(a), magnitude(0)], magnitude(pow(a, -1, modulus)))

        for scalar in [1, 2, 3, n - 1, rng.randrange(1, n)]:
            point = affine_multiply(p, scalar, g)
            public = ec.derive_private_key(scalar, parameters["curve"]()).public_key()
            encoded = public.public_bytes(serialization.Encoding.X962, serialization.PublicFormat.UncompressedPoint)
            assert encoded == point_bytes(point, width)
            add("independent public point", "key", curve_id, [encoded], True)
            for u, v in [(0, 0), (1, 1), (n, 0), (n - 1, 1),
                         (rng.randrange(n), rng.randrange(n))]:
                expected = affine_add(p, affine_multiply(p, u, g), affine_multiply(p, v, point))
                add("affine versus Jacobian combination", "point", curve_id,
                    [magnitude(u), magnitude(v), encoded], point_bytes(expected, width))

        invalid_keys = [b"", b"\0", b"\4", generator[:-1], generator + b"\0",
                        b"\4" + bytes(2 * width)]
        invalid_keys.extend(bytes([prefix]) + generator[1:] for prefix in [0, 2, 3, 6, 7, 255])
        invalid_keys.extend(b"\4" + magnitude(x) + magnitude(g[1]) for x in [p, p + 1, (1 << int(curve_id)) - 1])
        invalid_keys.extend(b"\4" + magnitude(g[0]) + magnitude(y) for y in [p, p + 1, (1 << int(curve_id)) - 1])
        invalid_keys.append(generator[:-1] + bytes([generator[-1] ^ 1]))
        for key in invalid_keys:
            add("invalid SEC1 public point", "key", curve_id, [key], False)

        for hash_name in ["sha256", "sha384", "sha512"]:
            for index in range(12):
                private = ec.derive_private_key(rng.randrange(1, n), parameters["curve"]())
                key = private.public_key().public_bytes(serialization.Encoding.X962, serialization.PublicFormat.UncompressedPoint)
                digest = hashlib.new(hash_name, rng.randbytes(index * 11)).digest()
                signature = private.sign(digest, ec.ECDSA(utils.Prehashed(digest_algorithm(digest))))
                r, s = utils.decode_dss_signature(signature)
                for candidate in [signature, utils.encode_dss_signature(r, n - s)]:
                    assert oracle_signature(parameters["curve"], key, digest, candidate)
                    add("generated ECDSA including high-S", "sig", curve_id, [key, digest, candidate], True)
                changed = bytes([digest[0] ^ 1]) + digest[1:]
                assert not oracle_signature(parameters["curve"], key, changed, signature)
                add("altered digest", "sig", curve_id, [key, changed, signature], False)
                altered = signature[:-1] + bytes([signature[-1] ^ 1])
                assert not oracle_signature(parameters["curve"], key, digest, altered)
                add("altered signature", "sig", curve_id, [key, digest, altered], False)
                if len(digest) > width:
                    tail = digest[:-1] + bytes([digest[-1] ^ 1])
                    assert oracle_signature(parameters["curve"], key, tail, signature)
                    add("left-bit digest truncation", "sig", curve_id, [key, tail, signature], True)

        # Construct the exceptionally rare x(R)>=n verification branch without
        # relying on randomly hitting it. These arbitrary prehash test inputs
        # do not assert knowledge of any private key for a real signed message.
        for r in range(1, 100):
            x = n + r
            rhs = (x * x * x - 3 * x + parameters["b"]) % p
            y = pow(rhs, (p + 1) // 4, p)
            if y * y % p == rhs:
                point = (x, y)
                break
        else:
            raise AssertionError("No small x>=n test point found")
        public_point = affine_multiply(p, pow(r, -1, n), point)
        key = point_bytes(public_point, width)
        digest = bytes(width)
        signature = utils.encode_dss_signature(r, 1)
        assert oracle_signature(parameters["curve"], key, digest, signature)
        add("rare x-coordinate reduction branch", "sig", curve_id, [key, digest, signature], True)
        add("zero resulting verification point", "sig", curve_id,
            [generator, magnitude(n - 1), utils.encode_dss_signature(1, 1)], False)

        malformed = [
            b"", b"\x30\x00", b"\x30\x06\x02\x01\x00\x02\x01\x01",
            b"\x30\x06\x02\x01\x01\x02\x01\x00",
            b"\x30\x06\x02\x01\x80\x02\x01\x01",
            b"\x30\x07\x02\x02\x00\x01\x02\x01\x01",
            b"\x30\x81\x06\x02\x01\x01\x02\x01\x01",
            b"\x30\x80\x02\x01\x01\x02\x01\x01\x00\x00",
            utils.encode_dss_signature(n, 1), utils.encode_dss_signature(1, n),
            utils.encode_dss_signature(n + 1, 1), utils.encode_dss_signature(1, n + 1),
            signature + b"\0", b"\0" * 65536,
        ]
        malformed.extend(signature[:length] for length in range(len(signature)))
        for candidate in malformed:
            add("strict malformed signature rejection", "sig", curve_id, [key, digest, candidate], False)
        for length in [0, 1, 31, 33, 47, 49, 63, 65, 1024]:
            add("unsupported digest length", "sig", curve_id, [key, bytes(length), signature], False)

    with tempfile.TemporaryDirectory(prefix="own-browser-ec-") as directory:
        folder = Path(directory)
        module = folder / "ec.rs"
        module.write_text((ROOT / "src/crypto_ec.rs").read_text(encoding="utf-8") + HOOKS, encoding="utf-8")
        source = folder / "probe.rs"
        source.write_text(PROBE.replace("path=EC", "path=" + json.dumps(str(module))), encoding="utf-8")
        binary = folder / ("probe.exe" if os.name == "nt" else "probe")
        subprocess.run([rustc, "--edition", "2021", "-O", str(source), "-o", str(binary)], check=True)
        process = subprocess.run([str(binary)], input="\n".join(case[2] for case in cases) + "\n",
                                 text=True, capture_output=True, check=True, timeout=60)
        answers = process.stdout.splitlines()
        if len(answers) != len(cases):
            raise AssertionError(f"Expected {len(cases)} results, got {len(answers)}: {process.stderr}")
        failures = [{"case": label, "expected": expected, "actual": answer}
                    for (label, _, _, expected), answer in zip(cases, answers) if answer != expected]
        print(json.dumps({"passed": len(cases) - len(failures), "total": len(cases),
                          "kinds": dict(Counter(case[1] for case in cases)),
                          "elapsed_seconds": round(time.monotonic() - started, 2),
                          "failures": failures[:12]}, indent=2))
        if failures:
            raise SystemExit(1)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustc", default=os.environ.get("RUSTC", "rustc"))
    run(parser.parse_args().rustc)
