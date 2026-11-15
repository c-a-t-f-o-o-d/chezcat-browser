"""Generate original public TEST TLS-SAN fixtures using development OpenSSL only.

Requires an existing OpenSSL command; never used by the browser or normal CI.
Random TEST keys are new each generation; this reproduces the profile, not bytes.
Never install the root or use the public leaf key outside isolated tests.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

NOT_BEFORE = '20200101000000Z'
NOT_AFTER = '20350101000000Z'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--openssl', default='openssl')
    parser.add_argument('--output', type=Path, required=True, help='New directory; existing fixtures are never overwritten.')
    args = parser.parse_args(argv)
    output = args.output.resolve()
    if output.exists():
        parser.error('Choose a new output directory; preserve existing fixture bytes.')
    commands = []
    with tempfile.TemporaryDirectory(prefix='chezcat-original-san-') as temporary:
        work = Path(temporary)
        (work / 'certs').mkdir()
        (work / 'index.txt').write_bytes(b'')
        (work / 'serial').write_bytes(b'1000\n')
        config = '''[ca]
default_ca = original_test
[original_test]
dir = "WORK_DIRECTORY"
database = $dir/index.txt
new_certs_dir = $dir/certs
serial = $dir/serial
certificate = $dir/root.pem
private_key = $dir/root-key.pem
default_md = sha256
default_days = 3650
policy = original_name
unique_subject = no
[original_name]
commonName = supplied
[test_root]
basicConstraints = critical,CA:true,pathlen:0
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always
[wrong_san_leaf]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always
subjectAltName = DNS:wrong.invalid
'''.replace('WORK_DIRECTORY', work.as_posix())
        (work / 'ca.cnf').write_bytes(config.encode())

        def run(*command, expected=0):
            commands.append(['openssl', *command])
            result = subprocess.run([args.openssl, *command], cwd=work, stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, timeout=30, check=False)
            if result.returncode != expected:
                raise RuntimeError('Development OpenSSL command failed: ' + command[0] + '\n' + result.stderr.decode('utf-8', errors='replace'))
            return result.stdout

        version = run('version').decode('utf-8').strip()
        run('genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048', '-out', 'root-key.pem')
        run('req', '-new', '-key', 'root-key.pem', '-subj', '/CN=ChezCat original SAN TEST root', '-out', 'root.csr')
        # A same-key bootstrap lets the CA command load its required issuer
        # context before emitting the final fixed-date self-signed certificate.
        run('req', '-new', '-x509', '-key', 'root-key.pem', '-subj', '/CN=ChezCat original SAN TEST root',
            '-sha256', '-days', '3650', '-out', 'root.pem')
        run('ca', '-batch', '-selfsign', '-config', 'ca.cnf', '-extensions', 'test_root',
            '-startdate', NOT_BEFORE, '-enddate', NOT_AFTER, '-in', 'root.csr', '-out', 'root-final.pem', '-notext')
        (work / 'root.pem').write_bytes((work / 'root-final.pem').read_bytes())
        run('genpkey', '-algorithm', 'EC', '-pkeyopt', 'ec_paramgen_curve:P-256', '-out', 'wrong-san-key.pem')
        run('req', '-new', '-key', 'wrong-san-key.pem', '-subj', '/CN=ChezCat original wrong SAN TEST leaf', '-out', 'leaf.csr')
        run('ca', '-batch', '-config', 'ca.cnf', '-extensions', 'wrong_san_leaf', '-startdate', NOT_BEFORE,
            '-enddate', NOT_AFTER, '-in', 'leaf.csr', '-out', 'leaf.pem', '-notext')
        run('verify', '-CAfile', 'root.pem', '-purpose', 'sslserver', 'leaf.pem')
        run('verify', '-CAfile', 'root.pem', '-purpose', 'sslserver', '-verify_ip', '127.0.0.1', 'leaf.pem', expected=2)
        run('verify', '-CAfile', 'root.pem', '-purpose', 'sslserver', '-verify_hostname', 'localhost', 'leaf.pem', expected=2)
        root_der = run('x509', '-in', 'root.pem', '-outform', 'DER')
        leaf_der = run('x509', '-in', 'leaf.pem', '-outform', 'DER')
        payload = {'root.pem': (work / 'root.pem').read_bytes(),
                   'wrong-san-chain.pem': (work / 'leaf.pem').read_bytes() + (work / 'root.pem').read_bytes(),
                   'wrong-san-key.pem': (work / 'wrong-san-key.pem').read_bytes()}
        output.mkdir(parents=True)
        for name, data in payload.items():
            (output / name).write_bytes(data)
        provenance = {'purpose': 'Original isolated loopback TLS IP/hostname SAN rejection fixture.',
                      'origin': 'Generated with development OpenSSL, without external certificate or key inputs.',
                      'tool': version, 'validity_utc': {'not_before': '2020-01-01T00:00:00Z', 'not_after': '2035-01-01T00:00:00Z'},
                      'root': {'self_signed': True, 'key': 'RSA2048', 'sha256': hashlib.sha256(root_der).hexdigest(),
                               'basic_constraints': 'critical CA=true pathlen=0', 'key_usage': 'critical keyCertSign,cRLSign'},
                      'leaf': {'key': 'P-256', 'sha256': hashlib.sha256(leaf_der).hexdigest(), 'subject_alt_name': ['DNS:wrong.invalid'],
                               'basic_constraints': 'critical CA=false', 'key_usage': 'critical digitalSignature', 'extended_key_usage': 'serverAuth'},
                      'files': {name: {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()} for name, data in payload.items()},
                      'generator': 'make_certificates.py --output NEW_DIRECTORY',
                      'commands': commands,
                      'limits': 'Public TEST leaf private key. Root signing key is discarded. No OS trust or normal browser root change. New random keys change hashes.'}
        (output / 'provenance.json').write_bytes((json.dumps(provenance, indent=2) + '\n').encode())
    print('Generated original public TEST SAN fixture; no browser, socket or OS trust operation.')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
