#!/usr/bin/env python3
"""Narrow integration patch for the original URL module. Run once on a copy.
It requires every anchor exactly once and never replaces whole shared files.
"""
from pathlib import Path
import sys
root=Path(sys.argv[1]).resolve()
patches={
'src/javascript.rs':[
('#[path = "date.rs"]\nmod date;', '#[path = "date.rs"]\nmod date;\n#[path = "url_api.rs"]\nmod url_api;'),
('    Date(date::Op),','    Date(date::Op),\n    Url(url_api::Op),'),
('struct Object {\n    date: Option<f64>,','struct Object {\n    date: Option<f64>,\n    url: Option<url_api::Data>,'),
('    date_prototype: Option<usize>,','    date_prototype: Option<usize>,\n    url_roots: url_api::Roots,'),
('            date_prototype: None,','            date_prototype: None,\n            url_roots: url_api::Roots::default(),'),
('        runtime.initialize_date().unwrap();','        runtime.initialize_date().unwrap();\n        runtime.initialize_url_api().unwrap();'),
('        self.objects.push(Object {\n            date: None,','        self.objects.push(Object {\n            date: None,\n            url: None,'),
('            Some(Function::Native(Builtin::Date(date::Op::Constructor))) => {\n                self.date_construct(args, host)\n            }', '            Some(Function::Native(Builtin::Date(date::Op::Constructor))) => {\n                self.date_construct(args, host)\n            }\n            Some(Function::Native(Builtin::Url(url_api::Op::Constructor(kind)))) => {\n                self.url_construct(*kind, args, host)\n            }'),
('            Some(Function::Native(Builtin::Date(operation))) => {\n                return self.date_builtin(*operation, args, this, host);\n            }', '            Some(Function::Native(Builtin::Date(operation))) => {\n                return self.date_builtin(*operation, args, this, host);\n            }\n            Some(Function::Native(Builtin::Url(operation))) => {\n                return self.url_builtin(*operation, args, this, host);\n            }'),
('            | Builtin::Date(_)','            | Builtin::Date(_)\n            | Builtin::Url(_)'),
],
'src/prototypes.rs':[
('            Builtin::Date(operation) => operation.arity(),','            Builtin::Date(operation) => operation.arity(),\n            Builtin::Url(operation) => operation.arity(),'),
('                    } else if object.date.is_some() {','                    } else if let Some(url) = &object.url {\n                        url.tag()\n                    } else if object.date.is_some() {'),
],
'src/iterable_builtins.rs':[
('                    | Builtin::Date(super::date::Op::Constructor)','                    | Builtin::Date(super::date::Op::Constructor)\n                    | Builtin::Url(super::url_api::Op::Constructor(_))'),
],
}
updated={}
for name,changes in patches.items():
    path=root/name;text=path.read_text()
    for old,new in changes:
        if text.count(old)!=1:raise SystemExit(f'{name}: expected one anchor, found {text.count(old)}: {old[:100]}')
        text=text.replace(old,new,1)
    updated[path]=text
for path,text in updated.items():path.write_text(text)
print('Applied URL wiring to '+str(root))
