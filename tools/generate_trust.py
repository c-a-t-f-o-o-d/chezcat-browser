"""Offline, reviewable Mozilla server-auth data selection; never fetch or install.

This original development tool produces data for the original Rust verifier.
The reviewed policy is deliberately a conservative subset: authorities with
unsupported certificate forms or external restrictions are excluded entirely.
It does not reproduce Firefox's revocation, CT, or complete trust policy.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import shlex

ROOT=Path(__file__).resolve().parents[1]
DATA=ROOT/'trust/mozilla'
MAX_INPUT=4*1024*1024
MAX_OBJECTS=4096
MAX_DER=65536
CERT_KEYS=set('CKA_CERTIFICATE_TYPE CKA_CLASS CKA_ID CKA_ISSUER CKA_LABEL CKA_MODIFIABLE CKA_NSS_EMAIL_DISTRUST_AFTER CKA_NSS_MOZILLA_CA_POLICY CKA_NSS_SERVER_DISTRUST_AFTER CKA_PRIVATE CKA_SERIAL_NUMBER CKA_SUBJECT CKA_TOKEN CKA_VALUE'.split())
TRUST_KEYS=set('CKA_CERT_MD5_HASH CKA_CERT_SHA1_HASH CKA_CLASS CKA_ISSUER CKA_LABEL CKA_MODIFIABLE CKA_PRIVATE CKA_SERIAL_NUMBER CKA_TOKEN CKA_TRUST_CODE_SIGNING CKA_TRUST_EMAIL_PROTECTION CKA_TRUST_SERVER_AUTH CKA_TRUST_STEP_UP_APPROVED'.split())
LIST_KEYS=set('CKA_CLASS CKA_LABEL CKA_MODIFIABLE CKA_PRIVATE CKA_TOKEN'.split())

def require(condition,message):
    if not condition: raise ValueError(message)

def digest(data): return hashlib.sha256(data).hexdigest()

def objects(data):
    require(len(data)<=MAX_INPUT,'certdata input exceeds its reviewed byte budget')
    result=[];record={};lines=iter(data.decode('utf-8').splitlines())
    for line in lines:
        if not line or line.startswith('#') or line=='BEGINDATA': continue
        fields=line.split(None,2)
        require(len(fields)>=2 and fields[0].startswith('CKA_'),'Unknown certdata syntax')
        key,kind=fields[:2]
        if key=='CKA_CLASS' and record:
            result.append(record);record={}
            require(len(result)<MAX_OBJECTS,'Too many certdata objects')
        require(key not in record,'Duplicate certdata attribute')
        if kind=='MULTILINE_OCTAL':
            value=bytearray()
            for encoded in lines:
                if encoded=='END': break
                require(re.fullmatch(r'(?:\\[0-3][0-7]{2})+',encoded),'Malformed octal attribute')
                value.extend(int(x,8) for x in re.findall(r'\\([0-3][0-7]{2})',encoded))
                require(len(value)<=MAX_DER,'Oversized octal attribute')
            else: raise ValueError('Unterminated octal attribute')
            value=bytes(value)
        else:
            require(len(fields)==3,'Missing certdata value')
            values=shlex.split(fields[2]);require(len(values)==1,'Malformed certdata value')
            value=values[0]
        record[key]=(kind,value)
    if record: result.append(record)
    require(0<len(result)<=MAX_OBJECTS,'Empty or excessive certdata objects')
    return result

def value(record,key,kind=None):
    require(key in record,f'Missing {key}')
    actual,data=record[key]
    require(kind is None or actual==kind,f'Unexpected {key} type')
    return data

def cutoff(record):
    field=record.get('CKA_NSS_SERVER_DISTRUST_AFTER')
    if field is None or field==('CK_BBOOL','CK_FALSE'): return None
    kind,data=field
    require(kind=='MULTILINE_OCTAL' and re.fullmatch(rb'\d{12}Z|\d{14}Z',data),'Unknown server distrust restriction')
    return data.decode('ascii')

VERIFIER_INPUTS=('crypto_hash.rs','crypto_sha512.rs','crypto_rsa.rs','crypto_ec.rs','x509.rs')

def verify_verifier_inputs(root,policy):
    reviewed=policy.get('reviewed_verifier_inputs')
    require(isinstance(reviewed,dict) and set(reviewed)==set(VERIFIER_INPUTS),
            'Capability review must pin exactly the five original verifier inputs')
    for name in VERIFIER_INPUTS:
        expected=reviewed[name]
        require(isinstance(expected,str) and re.fullmatch(r'[0-9a-f]{64}',expected),
                'Malformed reviewed verifier digest: '+name)
        try: data=(Path(root)/'src'/name).read_bytes()
        except OSError: raise ValueError('Reviewed verifier input is missing or unreadable: '+name) from None
        require(digest(data)==expected,'Reviewed verifier input changed; repeat original capability audit: '+name)

def render(certdata,policy,provenance):
    require(policy['schema_version']==1,'Unknown selection policy version')
    require(digest(certdata)==provenance['certdata_sha256'],'Pinned Mozilla certdata digest changed; explicit refresh review required')
    records=objects(certdata);certs=[];trusts={}
    for record in records:
        cls=value(record,'CKA_CLASS','CK_OBJECT_CLASS')
        keys={'CKO_CERTIFICATE':CERT_KEYS,'CKO_NSS_TRUST':TRUST_KEYS,'CKO_NSS_BUILTIN_ROOT_LIST':LIST_KEYS}.get(cls)
        require(keys is not None and set(record)<=keys,'Unreviewed object or policy attribute')
        if cls=='CKO_CERTIFICATE': certs.append(record)
        if cls=='CKO_NSS_TRUST':
            key=value(record,'CKA_CERT_SHA1_HASH','MULTILINE_OCTAL')
            require(len(key)==20 and key not in trusts,'Duplicate/malformed certificate trust identity')
            trusts[key]=record
    require(len(certs)==len(trusts),'Certificate/trust object counts differ')
    admissions=policy['native_supported_sha256']
    require(len(admissions)==len(set(admissions)) and all(re.fullmatch('[0-9a-f]{64}',x) for x in admissions),'Malformed reviewed native admission list')
    excluded=policy['native_unsupported']
    require(not set(admissions)&set(excluded),'Ambiguous native capability policy')
    reviewed=set(admissions)|set(excluded)
    restrictions=policy['imposed_subject_constraints']
    restricted={bytes.fromhex(x['subject_der_hex']) for x in restrictions}
    require(len(restricted)==len(restrictions),'Duplicate external subject restriction')
    entries=[];selected=[];seen=set();seen_trust=set();server_auth=set()
    for cert in certs:
        der=value(cert,'CKA_VALUE','MULTILINE_OCTAL');sha=digest(der)
        require(sha not in seen and 0<len(der)<=MAX_DER,'Duplicate or empty certificate')
        seen.add(sha)
        # SHA-1 is only the NSS data-link identity, never a certificate signature.
        key=hashlib.sha1(der).digest();require(key in trusts,'Missing exact certificate trust object')
        trust=trusts[key];seen_trust.add(key)
        for field in ['CKA_ISSUER','CKA_SERIAL_NUMBER']:
            require(value(cert,field,'MULTILINE_OCTAL')==value(trust,field,'MULTILINE_OCTAL'),'Certificate/trust identity mismatch')
        label=value(cert,'CKA_LABEL','UTF8')
        require(isinstance(label,str) and len(label)<=512,'Invalid certificate label')
        state=value(trust,'CKA_TRUST_SERVER_AUTH','CK_TRUST')
        require(state in ['CKT_NSS_TRUSTED_DELEGATOR','CKT_NSS_MUST_VERIFY_TRUST','CKT_NSS_NOT_TRUSTED'],'Unreviewed server-auth trust state')
        after=cutoff(cert);subject=value(cert,'CKA_SUBJECT','MULTILINE_OCTAL')
        reason=None
        if state!='CKT_NSS_TRUSTED_DELEGATOR': reason='Not a Mozilla server-auth trusted delegator'
        else:
            server_auth.add(sha)
            require(sha in reviewed,'Server-auth authority lacks explicit original-verifier capability review')
            if sha in excluded: reason='Unsupported by original verifier: '+excluded[sha]
            elif value(cert,'CKA_NSS_MOZILLA_CA_POLICY','CK_BBOOL')!='CK_TRUE': reason='Not marked Mozilla CA policy'
            elif after is not None: reason='Excluded entirely: leaf-issuance distrust cutoff is not implemented'
            elif subject in restricted: reason='Excluded entirely: externally imposed name constraint is not implemented'
        if reason is None: selected.append((sha,label,der))
        entries.append({'sha256':sha,'label':label,'server_auth_trust':state,
                        'server_distrust_after':after,'included':reason is None,'exclusion':reason})
    require(seen_trust==set(trusts),'Unused or duplicated trust objects')
    require(server_auth==reviewed,'Reviewed capability list differs from exact server-auth dataset')
    require(selected,'Selection contains no usable authorities')
    selected.sort()
    pem=bytearray(b'# Mozilla-derived conservative server-auth subset. See mozilla/PROVENANCE.json and selection.json.\n')
    for sha,label,der in selected:
        pem+=('# '+label+'\n# DER SHA-256 '+sha+'\n').encode('utf-8')
        body=base64.b64encode(der)
        pem+=b'-----BEGIN CERTIFICATE-----\n'+b'\n'.join(body[i:i+64] for i in range(0,len(body),64))+b'\n-----END CERTIFICATE-----\n\n'
    manifest={'schema_version':1,'source_commit':provenance['nss_commit'],
              'nss_builtins_version':provenance['nss_builtins_version'],
              'source_sha256':digest(certdata),'policy_sha256':digest((json.dumps(policy,sort_keys=True,separators=(',',':'))+'\n').encode('utf-8')),
              'pem_sha256':digest(pem),'included_count':len(selected),'excluded_count':len(entries)-len(selected),
              'policy':'Conservative original-verifier subset; excluded restrictions are never promoted to unrestricted trust. Not full Firefox trust/revocation/CT policy.',
              'certificates':sorted(entries,key=lambda x:x['sha256'])}
    return bytes(pem),(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n').encode('utf-8')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generate',action='store_true',help='Write reviewed generated data; otherwise check exact committed output')
    args=parser.parse_args()
    data=(DATA/'certdata.txt').read_bytes()
    policy=json.loads((DATA/'policy.json').read_bytes())
    provenance=json.loads((DATA/'PROVENANCE.json').read_bytes())
    verify_verifier_inputs(ROOT,policy)
    pem,manifest=render(data,policy,provenance)
    outputs={ROOT/'trust/roots.pem':pem,DATA/'selection.json':manifest}
    if args.generate:
        for path,contents in outputs.items(): path.write_bytes(contents)
    else:
        for path,contents in outputs.items(): require(path.read_bytes()==contents,'Generated trust data differs: '+path.name)
    print(f'Mozilla trust data {"generated" if args.generate else "verified"}: {json.loads(manifest)["included_count"]} authorities; no network or OS trust changes.')
    return 0

if __name__=='__main__': raise SystemExit(main())
