"""Focused publication boundary controls; no personal deny list or network."""
import os
from pathlib import Path
import struct
import subprocess
import tempfile
from unittest.mock import patch
import unittest
import zlib
import check_publication as publication


def png_chunk(kind,body):
    return struct.pack('>I',len(body))+kind+body+struct.pack('>I',zlib.crc32(kind+body)&0xffffffff)


def original_png(extra=b''):
    header=struct.pack('>IIBBBBB',1,1,8,6,0,0,0)
    return (b'\x89PNG\r\n\x1a\n'+png_chunk(b'IHDR',header)+extra+
            png_chunk(b'IDAT',zlib.compress(b'\0\0\0\0\xff'))+png_chunk(b'IEND',b''))


def original_icon(image=None):
    image=original_png() if image is None else image
    return struct.pack('<HHHBBBBHHII',0,1,1,1,1,0,0,1,32,len(image),22)+image


class PublicationControls(unittest.TestCase):
    def test_unreviewed_record_and_output_categories_rejected(self):
        for path in ['LOCAL_NOTES.md','docs/LOCAL_NOTES.md','docs/drafts/note.md',
                     'tools/run-output.json','tools/output/run.json','outputs/run.json',
                     '.local/settings.json','licensing/internal-record.txt']:
            with self.subTest(path=path),self.assertRaises(ValueError):publication.check_tree([path])
    def test_case_variant_is_rejected(self):
        with self.assertRaises(ValueError):publication.check_tree(['TOOLS/OUTPUT/RUN.JSON'])
    def test_private_path_separator_variant_rejected(self):
        with self.assertRaises(ValueError):publication.check_tree([r'results\local.json'])
    def test_product_and_reference_inputs_retained(self):
        self.assertTrue(publication.check_tree(['tools/reference/url-041/frozen.json',
          'tools/reference/url-041/edge-review-frozen.json','tools/reference/url-041/bounds-corrected.json',
          'trust/localhost.pem','resources/chezcat.res','resources/chezcat-x64.o','Cargo.lock',
          'notices/LICENSE.txt','src/scripting.rs']))
    def test_owner_email_override_rejected(self):
        with self.assertRaises(ValueError):publication.check_identity('Adam Maddox','synthetic@example.invalid')
    def test_verified_owner_and_independent_credit_allowed(self):
        publication.check_identity('Adam Maddox',publication.OWNER_EMAIL)
        publication.check_identity('Independent Fixture Author','fixture@example.test')
    def test_traversal_rejected(self):
        for path in ['../local.md','src/../README.md','/src/main.rs','src//main.rs',
                     'src/./main.rs','src/.local/state.json','src/main.rs:stream','src/bad\nname.rs']:
            with self.subTest(path=path),self.assertRaises(ValueError):publication.check_tree([path])

    def test_public_roles_and_local_outputs_match_ignore_rules(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-public-paths-') as folder:
            root=Path(folder)
            (root/'.gitignore').write_bytes((publication.ROOT/'.gitignore').read_bytes())
            subprocess.run(['git','init','-q'],cwd=root,check=True,capture_output=True)
            admitted=['README.md','src/new_feature.rs','tools/check_new_feature.py',
                      'tools/reference/example/input.json','docs/ROADMAP.md',
                      '.github/workflows/check.yml','.github/ISSUE_TEMPLATE/bug.md',
                      'resources/chezcat-x64.o','assets/chezcat.ico']
            excluded=['LOCAL_NOTES.md','docs/LOCAL_NOTES.md','tools/run-output.json',
                      'outputs/run.json','.local/settings.json','src/__pycache__/compiled.pyc']
            for name in admitted+excluded:
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b'Owned path fixture.')
            result=subprocess.run(['git','check-ignore','--no-index','-z','--stdin'],cwd=root,
                input=('\0'.join(admitted+excluded)+'\0').encode('utf-8'),capture_output=True,check=False)
            self.assertEqual(result.returncode,0)
            self.assertEqual({name.decode('utf-8') for name in result.stdout.split(b'\0') if name},set(excluded))
            self.assertTrue(publication.check_tree(admitted))

    def test_mailmap_cannot_conceal_raw_owner_metadata(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-owner-object-') as folder:
            root=Path(folder)
            (root/'payload.txt').write_text('Original synthetic fixture.\n',encoding='utf-8')
            (root/'.mailmap').write_text('Adam Maddox <'+publication.OWNER_EMAIL+'> <synthetic@example.invalid>\n',encoding='utf-8')
            env=dict(os.environ,GIT_AUTHOR_NAME='Adam Maddox',GIT_AUTHOR_EMAIL='synthetic@example.invalid',
                     GIT_COMMITTER_NAME='Adam Maddox',GIT_COMMITTER_EMAIL=publication.OWNER_EMAIL)
            for args in [('init','-q'),('add','.'),('-c','commit.gpgsign=false','commit','-qm','Synthetic metadata fixture')]:
                subprocess.run(['git',*args],cwd=root,env=env,check=True,capture_output=True)
            with patch.object(publication,'ROOT',root),self.assertRaisesRegex(ValueError,'noreply'):
                publication.check_metadata('HEAD')


class BrandingAssetControls(unittest.TestCase):
    def test_original_png_and_icon_rendering_payloads_allowed(self):
        self.assertTrue(publication.asset_metadata.check_payload('assets/icon.png',original_png()))
        self.assertTrue(publication.asset_metadata.check_payload('assets/icon.ico',original_icon()))
        bitmap=struct.pack('<IiiHHIIiiII',40,1,2,1,32,0,4,0,0,0,0)+bytes(8)
        self.assertTrue(publication.asset_metadata.check_payload('assets/icon.ico',original_icon(bitmap)))

    def test_embedded_metadata_rejected_in_png_and_icon(self):
        for kind in [b'tEXt',b'zTXt',b'iTXt',b'eXIf',b'caBX',b'tIME',b'iCCP',b'vpAg']:
            png=original_png(png_chunk(kind,b'Owned metadata fixture.'))
            for name,payload in [('icon.png',png),('icon.ico',original_icon(png))]:
                with self.subTest(chunk=kind,name=name),self.assertRaisesRegex(ValueError,'metadata'):
                    publication.asset_metadata.check_payload(name,payload)

    def test_rendering_chunks_cannot_carry_arbitrary_metadata(self):
        valid=[(b'sRGB',b'\0'),(b'gAMA',struct.pack('>I',45455)),
               (b'cHRM',struct.pack('>8I',31270,32900,64000,33000,30000,60000,15000,6000)),
               (b'pHYs',struct.pack('>IIB',3780,3780,1)),(b'sBIT',bytes([8]*4)),
               (b'bKGD',bytes(6))]
        for kind,body in valid:
            with self.subTest(chunk=kind):
                self.assertTrue(publication.asset_metadata.check_payload('icon.png',original_png(png_chunk(kind,body))))
        for kind in [b'sRGB',b'gAMA',b'cHRM',b'pHYs',b'sBIT',b'bKGD',b'tRNS',b'hIST']:
            with self.subTest(chunk=kind),self.assertRaises(ValueError):
                publication.asset_metadata.check_payload('icon.png',original_png(png_chunk(kind,b'Owned non-rendering metadata fixture.')))
        for kind,body in [(b'sRGB',b'\x04'),(b'gAMA',bytes(4)),(b'pHYs',bytes(8)+b'\x02'),(b'PLTE',b'\0'),
                          (b'sBIT',bytes([9]*4)),(b'bKGD',b'\xff'*6)]:
            with self.subTest(chunk=kind),self.assertRaises(ValueError):
                publication.asset_metadata.check_payload('icon.png',original_png(png_chunk(kind,body)))

    def test_png_framing_and_work_allowances_refuse(self):
        png=original_png();corrupt=bytearray(png);corrupt[29]^=1
        cases=[png[:7],png[:-1],png+b'extra',bytes(corrupt),
               b'\x89PNG\r\n\x1a\n'+png_chunk(b'IDAT',b''),
               original_png(png_chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0)))]
        for payload in cases:
            with self.subTest(size=len(payload)),self.assertRaises(ValueError):
                publication.asset_metadata.check_payload('icon.png',payload)
        with patch.object(publication.asset_metadata,'MAX_PNG_CHUNKS',1),self.assertRaises(ValueError):
            publication.asset_metadata.check_payload('icon.png',png)
        with patch.object(publication.asset_metadata,'MAX_ASSET_BYTES',len(png)-1),self.assertRaises(ValueError):
            publication.asset_metadata.check_payload('icon.png',png)

    def test_icon_extents_and_formats_refuse(self):
        icon=original_icon();gap=bytearray(icon);struct.pack_into('<I',gap,18,23)
        wrong_dimensions=bytearray(icon);wrong_dimensions[6]=2
        for payload in [icon[:21],icon+b'extra',bytes(gap),bytes(wrong_dimensions),original_icon(b'Owned unsupported image')]:
            with self.subTest(size=len(payload)),self.assertRaises(ValueError):
                publication.asset_metadata.check_payload('icon.ico',payload)
        with self.assertRaises(ValueError):publication.asset_metadata.check_payload('icon.dat',original_png())
        with self.assertRaises(ValueError):publication.asset_metadata.check_payload('README.md',original_png())

    def test_selected_committed_tree_uses_exact_bytes_and_preserves_history(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-asset-objects-') as folder:
            root=Path(folder);assets=root/'assets';assets.mkdir();path=assets/'icon.png'
            dirty=original_png(png_chunk(b'tEXt',b'Owned metadata fixture.'))
            env=dict(os.environ,GIT_AUTHOR_NAME='Controlled fixture',GIT_COMMITTER_NAME='Controlled fixture',
                     GIT_AUTHOR_EMAIL='fixture@example.test',GIT_COMMITTER_EMAIL='fixture@example.test')
            subprocess.run(['git','init','-q'],cwd=root,env=env,check=True,capture_output=True)
            for payload in [dirty,original_png()]:
                path.write_bytes(payload)
                for args in [('add','assets/icon.png'),('-c','commit.gpgsign=false','commit','-qm','Owned asset fixture')]:
                    subprocess.run(['git',*args],cwd=root,env=env,check=True,capture_output=True)
            with patch.object(publication,'ROOT',root):
                self.assertTrue(publication.check_assets('HEAD'))
                with self.assertRaisesRegex(ValueError,'metadata'):publication.check_assets('HEAD^')
                path.write_bytes(dirty)
                self.assertTrue(publication.check_assets('HEAD'))
                with patch.object(publication,'MAX_ASSET_SCAN_FILES',0),self.assertRaises(ValueError):
                    publication.check_assets('HEAD')
                with patch.object(publication,'MAX_ASSET_SCAN_BYTES',1),self.assertRaises(ValueError):
                    publication.check_assets('HEAD')

    def test_page_branding_copy_checked_without_restricting_codec_fixtures(self):
        with tempfile.TemporaryDirectory(prefix='chezcat-branding-copy-') as folder:
            root=Path(folder);(root/'assets').mkdir();(root/'pages/images').mkdir(parents=True)
            canonical=root/'assets/chezcat-icon.png';copy=root/'pages/images/chezcat-icon.png'
            fixture=root/'pages/images/codec-reference.png'
            dirty=original_png(png_chunk(b'tEXt',b'Owned metadata fixture.'))
            canonical.write_bytes(original_png());fixture.write_bytes(dirty)
            env=dict(os.environ,GIT_AUTHOR_NAME='Controlled fixture',GIT_COMMITTER_NAME='Controlled fixture',
                     GIT_AUTHOR_EMAIL='fixture@example.test',GIT_COMMITTER_EMAIL='fixture@example.test')
            subprocess.run(['git','init','-q'],cwd=root,env=env,check=True,capture_output=True)
            for payload,error in [(dirty,'metadata'),(original_png(),'') ,
                                  (original_png(png_chunk(b'sRGB',b'\0')),'identical bytes')]:
                copy.write_bytes(payload)
                for args in [('add','.'),('-c','commit.gpgsign=false','commit','-qm','Owned branding-copy fixture')]:
                    subprocess.run(['git',*args],cwd=root,env=env,check=True,capture_output=True)
                with patch.object(publication,'ROOT',root):
                    if error:
                        with self.assertRaisesRegex(ValueError,error):publication.check_assets('HEAD')
                    else:self.assertTrue(publication.check_assets('HEAD'))
if __name__=='__main__':unittest.main()
