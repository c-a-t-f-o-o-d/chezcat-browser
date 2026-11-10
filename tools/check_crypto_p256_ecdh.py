#!/usr/bin/env python3
"""Independent offline checks for the browser's original P-256 ECDH arithmetic.

Run: python tools/check_crypto_p256_ecdh.py [--rustc /path/to/rustc]
                                          [--output report.json]

Development requirements: Python cryptography and a host Rust compiler.
cryptography independently checks public points, shared secrets and invalid
points; Python integer arithmetic checks the field operations. The fixed
6,157-case matrix includes scalar/coordinate boundaries, bit patterns, random
cases, projective point exceptions and malformed inputs. All randomness here
is deterministic test data, never production key material.

The probe temporarily copies our original module and adds inspection hooks;
its source and executable are cleaned up after the run. No network access,
third-party runtime dependency or test-only API is added to the browser.
These are correctness checks, not a constant-time proof or cryptographic audit.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.backends.openssl.backend import backend

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'src/crypto_p256_ecdh.rs'
# NIST SP 800-186 section 3.2.1.3 mathematical domain parameters.
P = 0xffffffff00000001000000000000000000000000ffffffffffffffffffffffff
N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551

# The QA-only copy adds access to private field operations inside the module.
# The original source is inserted verbatim and its SHA-256 is recorded. No
# QA functions or third-party implementation code go into the production file.
EXTRA = r'''
pub fn qa(operation: &str, a: &[u8], b: &[u8], z: &[u8]) -> Result<Vec<u8>, String> {
    if operation == "public" { return public_key(a.try_into().map_err(|_| "length")?).map(|v| v.to_vec()); }
    if operation == "shared" { return shared_secret(a.try_into().map_err(|_| "length")?, b).map(|v| v.to_vec()); }
    if operation == "point-add" {
        let pa=peer_point(a)?; let pb=peer_point(b)?;
        let z=Field::encode(from_be(z.try_into().map_err(|_| "length")?));
        let z2=z.square();
        let pa=Jacobian { x: pa.x.mul(z2), y:pa.y.mul(z2.mul(z)), z };
        let sum=pa.add_affine(pb);
        if sum.z.zero_mask()!=0 { return Ok(Vec::new()); }
        let (x,y)=sum.affine()?;
        let mut result=vec![4];result.extend_from_slice(&to_be(x.decode()));result.extend_from_slice(&to_be(y.decode()));
        return Ok(result);
    }
    let fa=Field::encode(from_be(a.try_into().map_err(|_| "length")?));
    let fb=Field::encode(from_be(b.try_into().map_err(|_| "length")?));
    let value=match operation {
        "add"=>fa.add(fb), "sub"=>fa.sub(fb), "mul"=>fa.mul(fb),
        "square"=>fa.square(), "inverse"=>fa.inverse(), _=>return Err("operation".into()),
    };
    Ok(to_be(value.decode()).to_vec())
}
'''
MAIN = r'''
use std::io::{self, BufRead, Write};
fn hex(value:&str)->Vec<u8> {
    if value=="-" { return Vec::new(); }
    (0..value.len()).step_by(2).map(|i| u8::from_str_radix(&value[i..i+2],16).unwrap()).collect()
}
fn main() {
    let mut output=io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line=line.unwrap();let values:Vec<_>=line.split_whitespace().collect();
        let a=hex(values[1]);let b=hex(values.get(2).copied().unwrap_or("-"));
        let z=hex(values.get(3).copied().unwrap_or("-"));
        match p256::qa(values[0],&a,&b,&z) {
            Ok(value)=> {write!(output,"ok ").unwrap();for byte in value { write!(output,"{byte:02x}").unwrap();}writeln!(output).unwrap();},
            Err(_)=>writeln!(output,"error").unwrap(),
        }
    }
}
'''

def encoded(value):
    if isinstance(value, bytes):
        return value.hex() or '-'
    return value.to_bytes(32, 'big').hex()

def key(value):
    return ec.derive_private_key(value, ec.SECP256R1())

def public(value):
    return key(value).public_key().public_bytes(serialization.Encoding.X962, serialization.PublicFormat.UncompressedPoint)

def run(rustc):
    started = time.monotonic()
    rng = random.Random(0x012256)
    cases = []
    source_bytes = SOURCE.read_bytes()
    source = source_bytes.decode('utf-8')

    def case(category, operation, a, b=None, z=None, expected=None):
        command = ' '.join([operation, encoded(a)] + ([] if b is None else [encoded(b)]) + ([] if z is None else [encoded(z)]))
        wanted = 'error' if expected is None else 'ok' + ('' if expected == b'' else ' ' + expected.hex())
        cases.append((category, command, wanted))

    values = set(range(1, 33)) | {N-i for i in range(1, 33)}
    values |= {1 << bit for bit in range(256)}
    values |= {N-1-(1 << bit) for bit in range(256)}
    values |= {rng.randrange(1, N) for _ in range(128)}
    for value in sorted(values):
        case('public-key-reference', 'public', value, expected=public(value))

    boundary = [1, 2, 3, 1 << 63, 1 << 127, 1 << 255, N-2, N-1]
    pairs = [(a, b) for a in boundary for b in boundary]
    random_pairs = [(rng.randrange(1, N), rng.randrange(1, N)) for _ in range(128)]
    pairs += random_pairs + [(b, a) for a, b in random_pairs]
    for a, b in pairs:
        expected = key(a).exchange(ec.ECDH(), key(b).public_key())
        case('ecdh-reference', 'shared', a, public(b), expected=expected)

    edges = [0, 1, 2, 3, P-1, P-2, (1 << 32)-1, (1 << 64)-1, 1 << 64,
             (1 << 128)-1, 1 << 128, (1 << 192)-1, 1 << 192, (P-1)//2, (P+1)//2]
    pairs = [(a, b) for a in edges for b in edges]
    pairs += [(rng.randrange(P), rng.randrange(P)) for _ in range(1024)]
    for a, b in pairs:
        for operation, result in [('add', (a+b)%P), ('sub', (a-b)%P), ('mul', (a*b)%P)]:
            case('field-' + operation, operation, a, b, expected=result.to_bytes(32, 'big'))
    for value in edges + [rng.randrange(P) for _ in range(128)]:
        case('field-square', 'square', value, 0, expected=(value*value%P).to_bytes(32, 'big'))
        case('field-inverse', 'inverse', value, 0, expected=pow(value, P-2, P).to_bytes(32, 'big'))

    # Non-unit Jacobian denominators exercise equal/opposite-point selection.
    point_pairs = [(rng.randrange(1, N), rng.randrange(1, N)) for _ in range(64)]
    point_pairs += [(v, v) for v in boundary + [rng.randrange(1, N) for _ in range(24)]]
    point_pairs += [(v, N-v) for v in boundary + [rng.randrange(1, N) for _ in range(24)]]
    for a, b in point_pairs:
        expected = public((a+b)%N) if (a+b)%N else b''
        case('projective-add-reference', 'point-add', public(a), public(b), rng.randrange(1, P), expected=expected)

    bad_scalars = {0, N, N+1, P, (1 << 256)-1} | {rng.randrange(N, 1 << 256) for _ in range(32)}
    valid = public(17)
    for value in sorted(bad_scalars):
        case('invalid-private-scalar', 'public', value)
        case('invalid-private-scalar', 'shared', value, valid)
    for length in list(range(65)) + list(range(66, 132)):
        data = (valid + b'\x00'*132)[:length]
        case('malformed-public-point', 'shared', 19, data)
    for prefix in range(256):
        if prefix != 4:
            case('malformed-public-point', 'shared', 19, bytes([prefix])+valid[1:])
    for offset in [1, 33]:
        for value in [P, P+1, (1 << 256)-1]:
            malformed = bytearray(valid)
            malformed[offset:offset+32] = value.to_bytes(32, 'big')
            case('malformed-public-point', 'shared', 19, bytes(malformed))
    for bit in range(512):
        malformed = bytearray(valid)
        malformed[1+bit//8] ^= 1 << (bit % 8)
        try:
            ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), bytes(malformed))
        except ValueError:
            case('off-curve-point-reference', 'shared', 19, bytes(malformed))
        else:
            raise AssertionError('Unexpected valid single-bit point mutation')
    for x, y in [(0, 0), (1, 0), (0, 1), (1, 1), (P-1, P-1)]:
        malformed = b'\x04' + x.to_bytes(32, 'big') + y.to_bytes(32, 'big')
        try:
            ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), malformed)
        except ValueError:
            case('off-curve-point-reference', 'shared', 19, malformed)
        else:
            raise AssertionError('Unexpected valid special malformed point')

    commands = '\n'.join(command for _, command, _ in cases) + '\n'
    with tempfile.TemporaryDirectory(prefix='own-browser-p256-') as directory:
        folder = Path(directory)
        harness = folder / 'reference_harness.rs'
        harness.write_text('mod p256 {\n' + source + '\n' + EXTRA + '\n}\n' + MAIN, encoding='utf-8')
        executable = folder / ('reference-harness.exe' if os.name == 'nt' else 'reference-harness')
        subprocess.run([rustc, '--edition=2021', '-C', 'opt-level=3', '-C',
                        'overflow-checks=yes', str(harness), '-o', str(executable)],
                       check=True, timeout=60)
        run_start = time.monotonic()
        result = subprocess.run([str(executable)], input=commands, text=True,
                                capture_output=True, check=True, timeout=90)
        run_seconds = time.monotonic() - run_start
        answers = result.stdout.splitlines()
        if len(answers) != len(cases):
            raise AssertionError(f'Expected {len(cases)} results, got {len(answers)}: {result.stderr}')
        totals = {}
        for (category, command, expected), actual in zip(cases, answers):
            if actual.strip() != expected:
                raise AssertionError({'category': category, 'command': command,
                                      'expected': expected, 'actual': actual})
            totals[category] = totals.get(category, 0) + 1

    return {
        'source': str(SOURCE),
        'source_sha256': hashlib.sha256(source_bytes).hexdigest(),
        'independent_reference': backend.openssl_version_text(),
        'rng_seed': '0x012256 (test-only deterministic inputs)',
        'checks_by_category': totals,
        'total_reference_checks': len(cases),
        'all_passed': True,
        'native_reference_harness_seconds': round(run_seconds, 4),
        'total_seconds': round(time.monotonic() - started, 4),
        'scope': 'Correctness checks only; not a proof of constant-time execution or a cryptographic audit.',
        'primary_sources': [
            'https://doi.org/10.6028/NIST.SP.800-186',
            'https://www.secg.org/sec1-v2.pdf',
            'https://doi.org/10.6028/NIST.SP.800-56Ar3',
        ],
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rustc', default=os.environ.get('RUSTC', 'rustc'),
                        help='host Rust compiler (defaults to RUSTC or rustc)')
    parser.add_argument('--output', type=Path,
                        help='optional path for the successful JSON report')
    arguments = parser.parse_args()
    report = run(arguments.rustc)
    content = json.dumps(report, indent=2) + '\n'
    if arguments.output:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(content, encoding='utf-8')
    print(content, end='')
