"""Regenerate ChezCat's checked-in Windows icon/version resources.

Requires Python 3 and GNU windres from the Windows build toolchain. Neither is
needed to run ChezCat; ordinary Cargo builds link the already-generated files.
Run from any folder. --windres accepts an explicit resource-compiler path.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--windres', default=None)
    parser.add_argument('--preprocessor', default=None,
                        help='Optional matching gcc preprocessor executable')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    windres = args.windres or shutil.which('x86_64-w64-mingw32-windres') or shutil.which('windres')
    if not windres:
        parser.error('GNU windres was not found; pass --windres with its path.')
    compiler = shutil.which(windres) or windres
    source = root / 'resources/chezcat.rc'
    icon = root / 'assets/chezcat.ico'
    if not source.is_file() or not icon.is_file():
        parser.error('Expected resources/chezcat.rc and assets/chezcat.ico.')
    command = [compiler]
    if args.preprocessor:
        command.extend(['--preprocessor', args.preprocessor,
                        '--preprocessor-arg=-E', '--preprocessor-arg=-xc',
                        '--preprocessor-arg=-DRC_INVOKED'])
    with tempfile.TemporaryDirectory(prefix='chezcat-resources-') as temporary:
        temporary = Path(temporary)
        resource = temporary / 'chezcat.res'
        coff = temporary / 'chezcat-x64.o'
        subprocess.run(command + ['-J', 'rc', '-O', 'res', '-i', str(source),
                                 '-o', str(resource)], cwd=root, check=True)
        subprocess.run([compiler, '-J', 'res', '-O', 'coff', '-F', 'pe-x86-64',
                        '-i', str(resource), '-o', str(coff)], cwd=root, check=True)
        for output in [resource, coff]:
            shutil.copyfile(output, root / 'resources' / output.name)
    files = [source, icon, root / 'resources/chezcat.res', root / 'resources/chezcat-x64.o']
    report = {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in files}
    (root / 'resources/SHA256SUMS.txt').write_text(
        ''.join(f'{digest}  {name}\n' for name, digest in report.items()), encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
