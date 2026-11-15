"""Original offline trust-data selection and publication-boundary tests.

These are data-tool tests, not TLS handshakes or third-party verifier tests.
Small cases reuse two pinned public certificate records, mutate their data
selection metadata, and deliberately repin only that test input to reach each
guard. The actual complete source pin and 98-authority output are checked too.
"""
import base64
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import generate_trust as generator

PINNED_CERTDATA = '6d177e33ab54b338edfee24dc3868585833636716a320a725137ab4a15927292'
PINNED_PEM = 'c6a0199fe8d806aa5482fb19910b3cf7c36867739ee5e6d0225e67f86d31ae8f'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encode(records):
    """Original fixture writer; this is not an alternate NSS policy parser."""
    lines = ['# Original isolated mutation fixture', 'BEGINDATA']
    for record in records:
        for name, (kind, value) in record.items():
            if kind == 'MULTILINE_OCTAL':
                lines.append(name + ' ' + kind)
                for start in range(0, len(value), 20):
                    lines.append(''.join(f'\\{byte:03o}' for byte in value[start:start+20]))
                lines.append('END')
            else:
                lines.append(name + ' ' + kind + ' ' + json.dumps(value, ensure_ascii=False))
        lines.append('')
    return ('\n'.join(lines) + '\n').encode('utf-8')


class Selection(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = Path(__file__).resolve().parents[1]
        cls.raw = (cls.root / 'trust/mozilla/certdata.txt').read_bytes()
        cls.policy = json.loads((cls.root / 'trust/mozilla/policy.json').read_bytes())
        cls.provenance = json.loads((cls.root / 'trust/mozilla/PROVENANCE.json').read_bytes())
        cls.selection = json.loads((cls.root / 'trust/mozilla/selection.json').read_bytes())
        cls.records = generator.objects(cls.raw)
        certs = {sha(generator.value(record, 'CKA_VALUE')): record
                 for record in cls.records
                 if generator.value(record, 'CKA_CLASS') == 'CKO_CERTIFICATE'}
        trusts = {generator.value(record, 'CKA_CERT_SHA1_HASH'): record
                  for record in cls.records
                  if generator.value(record, 'CKA_CLASS') == 'CKO_NSS_TRUST'}
        chosen = [entry['sha256'] for entry in cls.selection['certificates'] if entry['included']][:2]
        cls.small = []
        for identity in chosen:
            cert = certs[identity]
            cls.small.extend([cert, trusts[hashlib.sha1(generator.value(cert, 'CKA_VALUE')).digest()]])
        cls.small_policy = copy.deepcopy(cls.policy)
        cls.small_policy.update(native_supported_sha256=sorted(chosen),
                                native_unsupported={}, imposed_subject_constraints=[])

    def fixture(self):
        return copy.deepcopy(self.small), copy.deepcopy(self.small_policy)

    def render(self, records, policy):
        data = encode(records)
        provenance = dict(self.provenance, certdata_sha256=sha(data))
        pem, manifest = generator.render(data, policy, provenance)
        return pem, json.loads(manifest)

    def test_actual_pinned_data_reproduces_all98_and_every_exclusion_class(self):
        self.assertEqual(sha(self.raw), PINNED_CERTDATA)
        pem, manifest_bytes = generator.render(self.raw, self.policy, self.provenance)
        self.assertEqual(pem, (self.root / 'trust/roots.pem').read_bytes())
        self.assertEqual(manifest_bytes, (self.root / 'trust/mozilla/selection.json').read_bytes())
        self.assertEqual(sha(pem), PINNED_PEM)
        result = json.loads(manifest_bytes)
        self.assertEqual((result['included_count'], result['excluded_count']), (98, 69))
        entries = result['certificates']
        self.assertEqual(len(entries), 167)
        self.assertEqual(sum(item['server_auth_trust'] != 'CKT_NSS_TRUSTED_DELEGATOR' for item in entries), 54)
        self.assertEqual(sum((item['exclusion'] or '').startswith('Unsupported by original verifier:') for item in entries), 10)
        self.assertEqual(sum((item['exclusion'] or '').startswith('Excluded entirely: leaf-issuance') for item in entries), 4)
        self.assertEqual(sum((item['exclusion'] or '').startswith('Excluded entirely: externally imposed') for item in entries), 1)
        blocks = pem.split(b'-----BEGIN CERTIFICATE-----\n')[1:]
        hashes = []
        for block in blocks:
            body = block.split(b'-----END CERTIFICATE-----', 1)[0]
            hashes.append(sha(base64.b64decode(b''.join(body.splitlines()), validate=True)))
        included = [item['sha256'] for item in entries if item['included']]
        self.assertEqual(hashes, sorted(included))
        self.assertEqual(len(hashes), len(set(hashes)))
        restricted = {bytes.fromhex(item['subject_der_hex']) for item in self.policy['imposed_subject_constraints']}
        for record in self.records:
            if generator.value(record, 'CKA_CLASS') != 'CKO_CERTIFICATE':
                continue
            identity = sha(generator.value(record, 'CKA_VALUE'))
            if identity in included:
                self.assertIn(identity, self.policy['native_supported_sha256'])
                self.assertNotIn(identity, self.policy['native_unsupported'])
                self.assertIsNone(generator.cutoff(record))
                self.assertNotIn(generator.value(record, 'CKA_SUBJECT'), restricted)
                self.assertEqual(generator.value(record, 'CKA_NSS_MOZILLA_CA_POLICY'), 'CK_TRUE')

    def test_changed_pin_stops_before_selection(self):
        with self.assertRaisesRegex(ValueError, 'Pinned Mozilla certdata digest changed'):
            generator.render(self.raw + b'# Original changed source\n', self.policy, self.provenance)

    def test_unknown_trust_attribute_class_and_purpose_state_refuse(self):
        for change in ('attribute', 'class', 'state'):
            records, policy = self.fixture()
            if change == 'attribute':
                records[1]['CKA_TRUST_ORIGINAL_FUTURE_TLS_RESTRICTION'] = ('CK_BBOOL', 'CK_TRUE')
            elif change == 'class':
                records[1]['CKA_CLASS'] = ('CK_OBJECT_CLASS', 'CKO_ORIGINAL_UNKNOWN_POLICY')
            else:
                records[1]['CKA_TRUST_SERVER_AUTH'] = ('CK_TRUST', 'CKT_ORIGINAL_CONDITIONAL_TLS_TRUST')
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.render(records, policy)

    def test_email_or_signing_trust_never_substitutes_for_server_auth(self):
        records, policy = self.fixture()
        excluded = sha(generator.value(records[0], 'CKA_VALUE'))
        records[1]['CKA_TRUST_SERVER_AUTH'] = ('CK_TRUST', 'CKT_NSS_MUST_VERIFY_TRUST')
        records[1]['CKA_TRUST_EMAIL_PROTECTION'] = ('CK_TRUST', 'CKT_NSS_TRUSTED_DELEGATOR')
        records[1]['CKA_TRUST_CODE_SIGNING'] = ('CK_TRUST', 'CKT_NSS_TRUSTED_DELEGATOR')
        policy['native_supported_sha256'].remove(excluded)
        _, manifest = self.render(records, policy)
        entry = next(item for item in manifest['certificates'] if item['sha256'] == excluded)
        self.assertFalse(entry['included'])
        self.assertEqual(entry['exclusion'], 'Not a Mozilla server-auth trusted delegator')
        self.assertEqual(manifest['included_count'], 1)

    def test_hash_link_identity_mismatch_duplicate_and_missing_trust_refuse(self):
        for change in ('hash', 'issuer', 'serial', 'duplicate', 'missing', 'length', 'type'):
            records, policy = self.fixture()
            if change == 'hash':
                records[1]['CKA_CERT_SHA1_HASH'] = ('MULTILINE_OCTAL', b'\xff' * 20)
            elif change in ('issuer', 'serial'):
                key = 'CKA_ISSUER' if change == 'issuer' else 'CKA_SERIAL_NUMBER'
                records[1][key] = ('MULTILINE_OCTAL', generator.value(records[1], key) + b'\x00')
            elif change == 'duplicate':
                records.append(copy.deepcopy(records[1]))
            elif change == 'missing':
                records.pop(1)
            elif change == 'length':
                records[1]['CKA_CERT_SHA1_HASH'] = ('MULTILINE_OCTAL', b'\x01' * 19)
            else:
                records[1]['CKA_CERT_SHA1_HASH'] = ('CK_BBOOL', 'CK_TRUE')
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.render(records, policy)

    def test_duplicate_attribute_and_malformed_octal_refuse(self):
        records, _ = self.fixture()
        data = encode(records)
        for malformed in [data.replace(b'CKA_LABEL UTF8', b'CKA_TOKEN CK_BBOOL CK_TRUE\nCKA_LABEL UTF8', 1),
                          b'BEGINDATA\nCKA_CLASS CK_OBJECT_CLASS CKO_CERTIFICATE\nCKA_VALUE MULTILINE_OCTAL\n\\777\nEND\n',
                          b'BEGINDATA\nCKA_CLASS CK_OBJECT_CLASS CKO_CERTIFICATE\nCKA_VALUE MULTILINE_OCTAL\n\\001\n']:
            with self.subTest(data=malformed[:100]), self.assertRaises(ValueError):
                generator.objects(malformed)

    def test_all_dates_and_external_constraints_exclude_entire_authority(self):
        for cutoff in (b'000101000000Z', b'491231235959Z', b'20991231235959Z'):
            records, policy = self.fixture()
            records[0]['CKA_NSS_SERVER_DISTRUST_AFTER'] = ('MULTILINE_OCTAL', cutoff)
            _, manifest = self.render(records, policy)
            self.assertEqual(manifest['included_count'], 1)
            excluded = next(item for item in manifest['certificates'] if not item['included'])
            self.assertEqual(excluded['server_distrust_after'], cutoff.decode('ascii'))
            self.assertIn('leaf-issuance distrust cutoff', excluded['exclusion'])
        records, policy = self.fixture()
        policy['imposed_subject_constraints'] = [{'name': 'Original restricted data control',
            'subject_der_hex': generator.value(records[0], 'CKA_SUBJECT').hex(),
            'dns_subtrees': ['.example']}]
        _, manifest = self.render(records, policy)
        self.assertEqual(manifest['included_count'], 1)
        self.assertTrue(any('externally imposed' in (item['exclusion'] or '') for item in manifest['certificates']))

    def test_unknown_date_restriction_refuses_instead_of_ignoring(self):
        for field in [('UTF8', 'tomorrow'), ('MULTILINE_OCTAL', b'not-a-time'), ('CK_BBOOL', 'CK_TRUE')]:
            records, policy = self.fixture()
            records[0]['CKA_NSS_SERVER_DISTRUST_AFTER'] = field
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'Unknown server distrust'):
                self.render(records, policy)

    def test_native_capability_review_cannot_be_unknown_duplicate_or_ambiguous(self):
        for change in ('missing', 'additional', 'duplicate', 'ambiguous'):
            records, policy = self.fixture()
            first = policy['native_supported_sha256'][0]
            if change == 'missing':
                policy['native_supported_sha256'].remove(first)
            elif change == 'additional':
                policy['native_supported_sha256'].append('0' * 64)
            elif change == 'duplicate':
                policy['native_supported_sha256'].append(first)
            else:
                policy['native_unsupported'][first] = 'Original explicitly unsupported control'
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.render(records, policy)

    def prepare_main(self, root):
        records, policy = self.fixture()
        sources = root / 'src'
        sources.mkdir()
        policy['reviewed_verifier_inputs'] = {}
        for number, name in enumerate(generator.VERIFIER_INPUTS):
            data = f'// Original isolated source-identity control {number}\n'.encode()
            (sources / name).write_bytes(data)
            policy['reviewed_verifier_inputs'][name] = sha(data)
        data = encode(records)
        provenance = dict(self.provenance, certdata_sha256=sha(data))
        folder = root / 'trust/mozilla'
        folder.mkdir(parents=True)
        for name, value in [('policy.json', policy), ('PROVENANCE.json', provenance)]:
            (folder / name).write_text(json.dumps(value), encoding='utf-8')
        (folder / 'certdata.txt').write_bytes(data)
        pem, manifest = generator.render(data, policy, provenance)
        (root / 'trust/roots.pem').write_bytes(pem)
        (folder / 'selection.json').write_bytes(manifest)
        return folder, policy

    def invoke_main(self, root, generate=False):
        with mock.patch.object(generator, 'ROOT', root), mock.patch.object(generator, 'DATA', root / 'trust/mozilla'), \
             mock.patch('sys.argv', ['generate_trust.py'] + (['--generate'] if generate else [])), mock.patch('builtins.print'):
            return generator.main()

    def test_changed_or_missing_verifier_input_prevents_fresh_generation_before_any_write(self):
        for missing in (False, True):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory(prefix='chezcat-trust-gate-') as temporary:
                root = Path(temporary)
                folder, _ = self.prepare_main(root)
                self.assertEqual(self.invoke_main(root, generate=True), 0)
                outputs = [root / 'trust/roots.pem', folder / 'selection.json']
                previous = [path.read_bytes() for path in outputs]
                source = root / 'src/x509.rs'
                source.unlink() if missing else source.write_bytes(b'// Original changed verifier input\n')
                with self.assertRaisesRegex(ValueError, 'verifier input'):
                    self.invoke_main(root, generate=True)
                self.assertEqual([path.read_bytes() for path in outputs], previous)

    def test_exact_verifier_names_and_digest_shapes_are_required(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-trust-names-') as temporary:
            root = Path(temporary)
            _, policy = self.prepare_main(root)
            generator.verify_verifier_inputs(root, policy)
            for change in ('missing', 'path', 'digest'):
                invalid = copy.deepcopy(policy)
                if change == 'missing':
                    invalid['reviewed_verifier_inputs'].pop('x509.rs')
                elif change == 'path':
                    invalid['reviewed_verifier_inputs']['../x509.rs'] = invalid['reviewed_verifier_inputs'].pop('x509.rs')
                else:
                    invalid['reviewed_verifier_inputs']['x509.rs'] = 'not-a-digest'
                with self.subTest(change=change), self.assertRaises(ValueError):
                    generator.verify_verifier_inputs(root, invalid)

    def test_check_mode_refuses_either_output_difference_without_rewriting_it(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-trust-output-') as temporary:
            root = Path(temporary)
            folder, _ = self.prepare_main(root)
            self.assertEqual(self.invoke_main(root), 0)
            for path in (root / 'trust/roots.pem', folder / 'selection.json'):
                original = path.read_bytes()
                changed = original + b'Original unexpected output\n'
                path.write_bytes(changed)
                with self.subTest(path=path.name), self.assertRaisesRegex(ValueError, 'Generated trust data differs'):
                    self.invoke_main(root)
                self.assertEqual(path.read_bytes(), changed)
                path.write_bytes(original)


if __name__ == '__main__':
    unittest.main(verbosity=2)
