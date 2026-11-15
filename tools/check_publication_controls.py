"""Focused publication boundary controls; no personal deny list or network."""
import os
from pathlib import Path
import subprocess
import tempfile
from unittest.mock import patch
import unittest
import check_publication as publication

class PublicationControls(unittest.TestCase):
    def test_raw_report_categories_rejected(self):
        for path in publication.PRIVATE_EXACT|{'tools/release-checks-sample.json','results/run.json','docs/prompts/private.md'}:
            with self.subTest(path=path),self.assertRaises(ValueError):publication.check_tree([path])
    def test_case_variant_is_rejected(self):
        with self.assertRaises(ValueError):publication.check_tree(['TOOLS/URL-041/BASELINE.JSON'])
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
        with self.assertRaises(ValueError):publication.check_tree(['../private.md'])

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
if __name__=='__main__':unittest.main()
