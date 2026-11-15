"""Small offline publication boundary: tracked private reports and owner metadata."""
import argparse
import fnmatch
from pathlib import Path, PurePosixPath
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]
OWNER_EMAIL='21222662+c-a-t-f-o-o-d@users.noreply.github.com'
PRIVATE_EXACT={'AGENTS.md','INSTRUCTIONS.md','STATUS.md','docs/AUTONOMY_STATE.md',
 'docs/SESSION.md','docs/STATUS.md','docs/WORKFLOW.md','licensing/COVERED_HISTORY.txt',
 'tools/url-041/baseline.json','tools/url-041/current.json','tools/url-041/first-replay.json',
 'tools/url-041/edge-review-results.json','tools/url-041/rust-tests.txt',
 'tools/url-041/URL_API_NOTES_0.41.txt'}
PRIVATE_PREFIXES=('results/','dist/','recovery/','private/','.codex/','.vscode/','docs/prompts/')

def check_tree(entries):
    for name in entries:
        normalized=name.replace('\\','/').casefold()
        if normalized in {p.casefold() for p in PRIVATE_EXACT} or normalized.startswith(tuple(p.casefold() for p in PRIVATE_PREFIXES)) or fnmatch.fnmatchcase(normalized,'tools/release-checks-*.json'):
            raise ValueError('Private working material or raw validation report in publication tree.')
        if PurePosixPath(normalized).is_absolute() or '..' in PurePosixPath(normalized).parts:
            raise ValueError('Unsafe tracked publication path.')
    return True

def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)

def check_identity(name,email):
    owner=name.casefold().replace(' ','') in ('adammaddox','adam','c-a-t-f-o-o-d')
    if owner and email!=OWNER_EMAIL:raise ValueError('Owner author/committer/tagger must use verified GitHub noreply identity.')

def check_metadata(commit):
    # Examine actual commit objects, including author fields preserved by cherry-pick.
    for line in git('log',commit,'--format=%an%x09%ae%x09%cn%x09%ce').decode('utf-8').splitlines():
        an,ae,cn,ce=line.split('\t');check_identity(an,ae);check_identity(cn,ce)
    return True

def check(ref='HEAD'):
    check_metadata(ref)
    roots=git('rev-list','--max-parents=0',ref).decode().splitlines()
    if len(roots)!=1:raise ValueError('Publication must retain a single snapshot lineage.')
    for commit in git('rev-list',ref).decode().splitlines():
        entries=[p.decode('utf-8') for p in git('ls-tree','-rz','--name-only',commit).split(b'\0') if p]
        check_tree(entries)
        if git('show',commit+':SOURCE_REVISION')!=b'$Format:%H$\n':
            raise ValueError('Committed SOURCE_REVISION must remain the literal export template.')
    scope=git('show',roots[0]+':LICENSE_SCOPE.md')
    if b'\nScope: containing-source-revision\n' not in scope:raise ValueError('Root must designate its containing snapshot.')
    # Tags can otherwise reintroduce a personal address independently of commits.
    for row in git('for-each-ref','refs/tags','--format=%(objecttype)%09%(taggername)%09%(taggeremail)').decode().splitlines():
        kind,name,email=row.split('\t')
        if kind=='tag':check_identity(name,email.strip('<>'))
    return True

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--ref',default='HEAD');args=parser.parse_args()
    try:check(args.ref)
    except (ValueError,OSError,subprocess.CalledProcessError) as error:
        print('Publication boundary failed: '+str(error),file=sys.stderr);raise SystemExit(1)
    print('Publication paths, snapshot template and owner metadata: PASS (not an exhaustive privacy audit).')
