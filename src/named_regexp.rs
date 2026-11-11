//! Native named capture results use ordinary null-prototype dictionaries.
//! Names and captured values keep the existing property/string/allocation caps.

use super::{regexp, Fault, Runtime, Value};

impl Runtime {
    pub(super) fn regexp_named_groups(
        &mut self,
        found: &regexp::Match,
        input: &[u16],
    ) -> Result<Value, Fault> {
        if found.named_captures.is_empty() {
            return Ok(Value::Undefined);
        }
        let id = self.new_object(None)?;
        self.objects[id].prototype = None;
        for (name, capture) in &found.named_captures {
            let value = if let Some((start, end)) = capture {
                Value::String(self.units_string(&input[*start..*end])?)
            } else {
                Value::Undefined
            };
            self.set_property_throw(id, name.clone(), value)?;
        }
        Ok(Value::Object(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::javascript::Host;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("Unexpected native read".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("Unexpected native write".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("Unexpected native call".into())
        }
    }

    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::new()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn named_results_have_null_prototype_own_descriptors_and_stable_order() {
        succeeds(
            r#"
var m=/(?<word>a)(?<optional>b)?(c)/.exec('ac');
if(m[0]!=='ac'||m[1]!=='a'||m[2]!==undefined||m[3]!=='c'||m.index!==0||m.input!=='ac')throw 'capture values';
if(Object.getPrototypeOf(m.groups)!==null||Object.keys(m.groups).join()!=='word,optional')throw 'groups shape';
var d=Object.getOwnPropertyDescriptor(m.groups,'word');
if(!d.writable||!d.enumerable||!d.configurable||d.value!=='a')throw 'descriptor';
var n=/(?<constructor>a)(?<toString>b)/.exec('ab');
if(n.groups.constructor!=='a'||n.groups.toString!=='b')throw 'prototype names';
if(/a/.exec('a').groups!==undefined)throw 'unnamed shape';
m.groups.word='changed';if(!delete m.groups.optional||m.groups.word!=='changed')throw 'ordinary properties';'ok';
"#,
        );
    }

    #[test]
    fn named_results_follow_regex_string_methods_and_last_index() {
        succeeds(
            r#"
var r=/(?<x>a)/gy;r.lastIndex=1;var m=r.exec('ba');
if(m.groups.x!=='a'||r.lastIndex!==2||r.exec('ba')!==null||r.lastIndex!==0)throw 'state';
if('ba'.match(/(?<x>a)/).groups.x!=='a'||'baa'.match(/(?<x>a)/g).join()!=='a,a')throw 'match';
if('ba'.search(/(?<x>a)/)!==1||'bac'.split(/(?<x>a)/).join()!=='b,a,c')throw 'search split';
var d=/^(?:(?<x>a)|(?<x>b))*$/.exec('ab');
if(d.groups.x!=='b'||d[1]!==undefined||d[2]!=='b')throw 'reset duplicate';'ok';
"#,
        );
    }

    #[test]
    fn named_replacements_support_templates_and_callback_dictionaries() {
        succeeds(
            r#"
if('ab a'.replace(/(?<x>a)(?<y>b)?/g,'[$<x>:$<y>:$<missing>:$1:$2:$$]')!=='[a:b::a:b:$] [a:::a::$]')throw 'template';
if('a'.replace(/a/,'$<x>')!=='$<x>'||'a'.replace(/(?<x>a)/,'$<x')!=='$<x')throw 'literal fallback';
var r=/(?<x>a)|(?<y>b)/g,seen=[],last;
var result='ab'.replaceAll(r,function(m,a,b,index,input,groups){'use strict';
 if(this!==undefined||arguments.length!==6||input!=='ab'||Object.getPrototypeOf(groups)!==null||groups===last)throw 'callback shape';
 seen.push(index+':'+r.lastIndex);last=groups;r.lastIndex=7;return groups.x||groups.y;
});
if(result!=='ab'||seen.join()!=='0:0,1:7')throw 'snapshot';'ok';
"#,
        );
    }

    #[test]
    fn named_groups_do_not_read_inherited_setters() {
        succeeds(
            r#"
var called=false;Object.defineProperty(Object.prototype,'entry',{set:function(){called=true;},configurable:true});
var a=/(?<entry>a)/.exec('a');
var b='b'.replace(/(?<entry>b)/,function(match,entry,index,text,groups){return groups.entry;});
delete Object.prototype.entry;
if(called||a.groups.entry!=='a'||b!=='b')throw 'inherited setter';'ok';
"#,
        );
    }

    #[test]
    fn named_syntax_and_capability_failures_preserve_atomicity() {
        for pattern in [
            "(?<x>a)(?<x>b)",
            "(?<1x>a)",
            "(?<x>a)\\k<missing>",
            "(?<猫>a)",
            "(?<__proto__>a)",
        ] {
            let mut runtime = Runtime::new();
            runtime.execute("var effect=false;", &mut NoHost).unwrap();
            runtime
                .execute(&format!("effect=true;/{pattern}/;"), &mut NoHost)
                .unwrap_err();
            runtime.begin_job();
            assert_eq!(
                runtime.execute("effect;", &mut NoHost).unwrap().to_text(),
                "false"
            );
        }
    }

    #[test]
    fn named_reference_limits_remain_fatal_and_reset_for_next_job() {
        let mut runtime = Runtime::new();
        runtime
            .execute(
                "var caught=false,finalized=false,r=/(?<x>a+)\\k<x>/;",
                &mut NoHost,
            )
            .unwrap();
        runtime.regexp_work = 100;
        let error = runtime
            .execute(
                "try{while(true)r.test('aaaaaaaa');}catch(e){caught=true;}finally{finalized=true;}",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("work limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime
                .execute(
                    "!caught&&!finalized&&r.exec('aaaa').groups.x==='aa';",
                    &mut NoHost
                )
                .unwrap()
                .to_text(),
            "true"
        );
    }
}
