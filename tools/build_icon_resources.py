"""Regenerate ChezCat's checked-in Windows icon/version resources.

The default path uses GNU windres from the Windows build toolchain. --portable
uses the original Python standard-library writer for this project's bounded RC
grammar. Neither tool is needed to run ChezCat or build the checked-in resources.
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
    parser.add_argument('--portable', action='store_true',
                        help='Use the bounded original standard-library resource writer')
    parser.add_argument('--preprocessor', default=None,
                        help='Optional matching gcc preprocessor executable')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    windres = args.windres or shutil.which('x86_64-w64-mingw32-windres') or shutil.which('windres')
    if not windres and not args.portable:
        parser.error('GNU windres was not found; pass --windres with its path.')
    if args.portable and (args.windres or args.preprocessor):
        parser.error('--portable cannot be combined with external compiler options.')
    compiler = (shutil.which(windres) or windres) if windres else None
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
        if args.portable:
            from portable_resources import generate
            resource_bytes, coff_bytes = generate(source.read_text(encoding='utf-8'), icon.read_bytes())
            resource.write_bytes(resource_bytes)
            coff.write_bytes(coff_bytes)
        else:
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
