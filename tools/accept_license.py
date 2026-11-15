"""Explicit offline source/headless acceptance; never called implicitly to consent."""
import argparse, hashlib, json, os
from pathlib import Path
import stat, tempfile, time

ROOT=Path(__file__).resolve().parents[1]
VERSION='1.1'
DIGEST='34003a38eeb1db77472c307e23b5a5540c7625c50d9f0ea0dc7bacf6e5b9b749'
FIELDS={'schema_version','license_version','license_sha256','accepted_scope','accepted_at_utc'}
MAX_BYTES=2048

def terms():
    text=(ROOT/'LICENSE').read_bytes()
    if hashlib.sha256(text).hexdigest()!=DIGEST:raise ValueError('Shipped license and approved digest differ.')
    return text

def directory(explicit=None):
    selected=explicit or os.environ.get('CHEZCAT_LICENSE_RECEIPT_DIR')
    if selected is None:
        if os.name=='nt':
            if not os.environ.get('LOCALAPPDATA'):raise ValueError('Local license storage unavailable; choose an external --receipt-dir.')
            selected=Path(os.environ['LOCALAPPDATA'])/'ChezCat/License'
        else:selected=Path(os.environ['XDG_STATE_HOME'])/'chezcat/license' if os.environ.get('XDG_STATE_HOME') else Path.home()/'.local/state/chezcat/license'
    path=Path(selected)
    if not path.is_absolute() or '..' in path.parts or path.drive.startswith('\\\\'):raise ValueError('License receipt directory must be normalized, absolute and local.')
    for part in [path,*path.parents]:
        try:info=part.lstat()
        except FileNotFoundError:continue
        if stat.S_ISLNK(info.st_mode) or getattr(info,'st_file_attributes',0)&0x400:raise ValueError('License storage cannot use links/reparse points.')
    path=path.resolve()
    roots=[ROOT.resolve()]
    for ancestor in (Path.cwd(),*Path.cwd().parents):
        if (ancestor/'.git').exists() or ((ancestor/'Cargo.toml').is_file() and (ancestor/'src/main.rs').is_file()):roots.append(ancestor.resolve())
    if os.name=='nt' and os.environ.get('LOCALAPPDATA'):roots.append((Path(os.environ['LOCALAPPDATA'])/'OwnBrowser').resolve())
    if any(path.is_relative_to(root) for root in roots):raise ValueError('License receipts must be outside source checkouts and application profiles.')
    return path

def unique(pairs):
    data={}
    for key,value in pairs:
        if key in data:raise ValueError('Duplicate receipt field.')
        data[key]=value
    return data

def valid(data):
    return (isinstance(data,dict) and set(data)==FIELDS and type(data['schema_version']) is int
        and data['schema_version']==1 and data['license_version']==VERSION and data['license_sha256']==DIGEST
        and data['accepted_scope'] in ('personal','organization') and type(data['accepted_at_utc']) is int
        and 0<=data['accepted_at_utc']<=253402300799)

def check(folder):
    path=folder/'receipt.json'
    try:
        info=path.lstat()
        if not stat.S_ISREG(info.st_mode) or getattr(info,'st_file_attributes',0)&0x400:raise ValueError('License receipt must be a regular local file.')
        if info.st_size>MAX_BYTES:return False
        with path.open('rb') as stream:raw=stream.read(MAX_BYTES+1)
        if len(raw)>MAX_BYTES:return False
        # Closed receipt encoding: all fields are plain ASCII, without escapes.
        # Match the bounded native parser rather than accept a form it rejects.
        if any(b>=128 or b==92 or (b<32 and b not in (9,10,13)) for b in raw):return False
        return valid(json.loads(raw,object_pairs_hook=unique))
    except FileNotFoundError:return False
    except (UnicodeError,json.JSONDecodeError,TypeError,KeyError):return False

def accept(folder,digest,scope):
    if digest!=DIGEST:raise ValueError('Acceptance requires the exact full shipped LICENSE SHA-256.')
    if scope not in ('personal','organization'):raise ValueError('Choose personal or authorized-organization acceptance.')
    now=int(time.time())
    if not 0<=now<=253402300799:raise ValueError('UTC clock is outside the receipt format.')
    folder.mkdir(parents=True,exist_ok=True,mode=0o700)
    directory(folder)
    target=folder/'receipt.json'
    try:
        info=target.lstat()
        if not stat.S_ISREG(info.st_mode) or getattr(info,'st_file_attributes',0)&0x400 or not info.st_mode&stat.S_IWUSR:raise ValueError('Receipt cannot be replaced safely.')
    except FileNotFoundError:pass
    data={'schema_version':1,'license_version':VERSION,'license_sha256':DIGEST,'accepted_scope':scope,'accepted_at_utc':now}
    descriptor,temporary=tempfile.mkstemp(prefix='.receipt-',suffix='.tmp',dir=folder)
    try:
        with os.fdopen(descriptor,'wb') as stream:
            stream.write((json.dumps(data,separators=(',',':'))+'\n').encode());stream.flush();os.fsync(stream.fileno())
        os.replace(temporary,target)
    finally:
        if os.path.exists(temporary):os.unlink(temporary)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    modes=parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--show',action='store_true');modes.add_argument('--check',action='store_true');modes.add_argument('--accept-license')
    parser.add_argument('--license-scope',choices=['personal','organization']);parser.add_argument('--receipt-dir',type=Path)
    args=parser.parse_args()
    try:
        text=terms()
        if args.show:
            if args.license_scope or args.receipt_dir:parser.error('--show only displays terms.')
            print(text.decode('utf-8'),end='');return 0
        folder=directory(args.receipt_dir)
        if args.check:
            if args.license_scope:parser.error('--check does not choose or accept a scope.')
            if not check(folder):raise ValueError('Explicit acceptance is missing, invalid or for different terms; view LICENSE, then use --accept-license and --license-scope.')
            print('Current exact license acceptance: PASS.');return 0
        if not args.license_scope:parser.error('--accept-license requires --license-scope.')
        accept(folder,args.accept_license,args.license_scope)
        print('Explicit local acceptance recorded: CSAPUL '+VERSION+'; SHA-256 '+DIGEST+'; scope '+args.license_scope+'.')
        return 0
    except (OSError,ValueError):
        # Avoid home/profile paths and local identities in CI console summaries.
        print('License acceptance refused: terms, explicit scope or trusted external receipt could not be verified. View LICENSE and choose a writable external receipt directory.',file=__import__('sys').stderr)
        return 1
if __name__=='__main__':raise SystemExit(main())
