//! Original bounded array length descriptor handling. Indexed descriptors use
//! the ordinary stored-property validation and update length only after a
//! successful definition. Length stays virtual, nonenumerable and nonconfigurable.
//! https://tc39.es/ecma262/#sec-array-exotic-objects-defineownproperty-p-desc
//! https://tc39.es/ecma262/#sec-arraysetlength

use super::{
    int32, property_index, Fault, Host, Runtime, Value, MAX_ARRAY_LENGTH, MAX_PROPERTY_BYTES,
};

impl Runtime {
    pub(super) fn array_length_value(
        &mut self,
        value: &Value,
        host: &mut dyn Host,
    ) -> Result<usize, Fault> {
        // ToUint32 and ToNumber separately observe the original value. In
        // particular, valueOf may run twice and mutate the destination array.
        let unsigned = int32(self.binary_number(value, host)?) as u32;
        let number = self.binary_number(value, host)?;
        if unsigned as f64 != number {
            return Err(Fault::exception("RangeError", "Invalid array length."));
        }
        Ok(unsigned as usize)
    }

    /// Apply a converted length descriptor. False is a descriptor rejection;
    /// assignments choose strict/sloppy behavior, while defineProperty throws.
    pub(super) fn apply_array_length(
        &mut self,
        id: usize,
        length: Option<usize>,
        writable: Option<bool>,
        valid_attributes: bool,
    ) -> Result<bool, Fault> {
        self.step()?;
        let object = self.object(id)?;
        let old_length = object
            .length
            .ok_or_else(|| Fault::new("Expected an array."))?;
        let length = length.unwrap_or(old_length);
        if !valid_attributes
            || !object.length_writable && (writable == Some(true) || length != old_length)
        {
            return Ok(false);
        }
        if length > MAX_ARRAY_LENGTH {
            return Err(Fault::new("Array length limit reached (8192)."));
        }
        if length < old_length {
            // At most the existing per-object property cap is snapshotted.
            // Deletion invokes no user callbacks in this object model. Keep
            // the old length until completion so a work-limit interruption
            // cannot leave a stored index beyond the array's logical length.
            self.work(object.properties.len().saturating_mul(MAX_PROPERTY_BYTES))?;
            let mut keys = self.objects[id]
                .properties
                .iter()
                .filter_map(|(name, property)| {
                    property_index(name)
                        .filter(|index| *index >= length)
                        .map(|index| (index, name.clone(), property.configurable))
                })
                .collect::<Vec<_>>();
            keys.sort_unstable_by_key(|item| std::cmp::Reverse(item.0));
            for (index, name, configurable) in keys {
                self.step()?;
                if !configurable {
                    self.objects[id].length = Some(index + 1);
                    if writable == Some(false) {
                        self.objects[id].length_writable = false;
                    }
                    return Ok(false);
                }
                self.remove_property(id, &name)?;
            }
        }
        self.objects[id].length = Some(length);
        if let Some(writable) = writable {
            self.objects[id].length_writable = writable;
        }
        Ok(true)
    }

    // Internal numeric-length writes share the same deletion/accounting path.
    // They are Set operations, so a read-only length rejects even SameValue.
    pub(super) fn set_array_length(&mut self, id: usize, length: usize) -> Result<(), Fault> {
        let succeeded = self.object(id)?.length_writable
            && self.apply_array_length(id, Some(length), None, true)?;
        if !succeeded && self.strict {
            return Err(Fault::exception("TypeError", "Cannot change array length."));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No native objects.".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No native objects.".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No native objects.".into())
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
    fn array_descriptors_define_data_accessors_and_ordered_names() {
        succeeds(
            r#"
var a=[], hits=0, key=Symbol('side');
Object.defineProperty(a,'3',{value:'third',writable:true,enumerable:true,configurable:true});
if(a.length!==4||0 in a||a[3]!=='third')throw 'index growth';
Object.defineProperty(a,'1',{get:function(){hits++;return this[3];},configurable:true});
var d=Object.getOwnPropertyDescriptors(a);
if(hits||d[1].get.call(a)!=='third'||hits!==1||d[1].enumerable||'value' in d[1])throw 'accessor descriptor';
Object.defineProperty(a,'1',{set:function(v){this[3]=v;}});a[1]='changed';
if(a[1]!=='changed'||hits!==2)throw 'merged accessor';
Object.defineProperty(a,'1',{value:'first',writable:true,enumerable:true});
Object.defineProperty(a,'01',{value:'ordinary'});a[key]='symbol';a.note=7;
if(a[1]!=='first'||a.length!==4||Object.keys(a).join('|')!=='1|3|note')throw 'attributes';
if(Object.getOwnPropertyNames(a).join('|')!=='1|3|length|01|note')throw 'name ordering';
Object.defineProperty(a,'length',{value:6});d=Object.getOwnPropertyDescriptor(a,'length');
if(d.value!==6||!d.writable||d.enumerable||d.configurable||Object.keys(a).indexOf('length')!==-1)throw 'length descriptor';
Object.defineProperty(a,'length',{value:1});
if(a.length!==1||1 in a||3 in a||a['01']!=='ordinary'||a[key]!=='symbol'||a.note!==7)throw 'shrink';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_shrink_descending_and_lock_after_blocked_deletion() {
        succeeds(
            r#"
var a=[0,1,2,3,4,5,6,7,8,9,10],caught=0;
Object.defineProperty(a,'2',{configurable:false});
Object.defineProperty(a,'7',{configurable:false});
try{Object.defineProperty(a,'length',{value:1,writable:false});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==1||a.length!==8||a[7]!==7||8 in a||10 in a||!(6 in a)||!(2 in a))throw 'descending partial shrink';
if(Object.getOwnPropertyDescriptor(a,'length').writable)throw 'lock after failure';
a[0]='ok';a[8]='absent';a.length=0;
if(a[0]!=='ok'||8 in a||a.length!==8)throw 'locked writes';
var b=[0,1,2,3];Object.defineProperty(b,'2',{configurable:false});b.length=1;
if(b.length!==3||3 in b||!(1 in b))throw 'sloppy shrink';
try{(function(){'use strict';b.length=0;})();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2||b.length!==3||Object.getOwnPropertyDescriptor(b,'length').writable!==true)throw 'strict shrink';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_readonly_length_allows_existing_indices_and_same_definitions() {
        succeeds(
            r#"
var a=[1,,3],caught=0, calls=0;
Object.defineProperty(a,'length',{writable:false});
Object.defineProperty(a,'0',{value:9});Object.defineProperty(a,'1',{value:2,configurable:true});
a[0]=8;a[3]=4;a.length={valueOf:function(){calls++;return 0;}};
if(a.length!==3||a[0]!==8||a[1]!==2||3 in a||calls)throw 'readonly writes';
Object.defineProperty(a,'length',{value:'3',writable:false});Object.defineProperty(a,'length',{});
try{Object.defineProperty(a,'3',{value:4});}catch(e){if(e.name==='TypeError')caught++;}
try{Object.defineProperty(a,'length',{writable:true});}catch(e){if(e.name==='TypeError')caught++;}
try{(function(){'use strict';a.length=3;})();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3)throw 'readonly definitions';
var b=Object.preventExtensions([4,5]);Object.defineProperty(b,'length',{value:4});
try{Object.defineProperty(b,'3',{value:6});}catch(e){if(e.name==='TypeError')caught++;}
Object.defineProperty(b,'0',{value:7});Object.defineProperty(b,'length',{value:1});
if(caught!==4||b.length!==1||b[0]!==7||1 in b)throw 'nonextensible';
var z=[];Object.freeze(z);Object.defineProperty(z,'length',{value:-0});
if(1/z.length!==Infinity||!Object.isFrozen(z))throw 'zero same value';
var child=Object.create(a);child.length=10;
if(child.hasOwnProperty('length')||child.length!==3)throw 'inherited readonly length';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_length_conversion_runs_twice_and_rechecks_live_attributes() {
        succeeds(
            r#"
var a=[0,1,2], calls=0;
Object.defineProperty(a,'length',{value:{valueOf:function(){calls++;if(calls===1){a.length=6;return 4294967297;}return 1;}}});
if(calls!==2||a.length!==1||1 in a)throw 'two conversions';
calls=0;a.length={valueOf:function(){calls++;return calls===1?2.5:2;}};
if(calls!==2||a.length!==2||1 in a)throw 'assignment conversion';
calls=0;Object.defineProperty(a,'length',{value:{valueOf:function(){calls++;if(calls===2)Object.freeze(a);return 2;}}});
if(calls!==2||!Object.isFrozen(a)||a.length!==2)throw 'live descriptor';
var b=[0], n=0;
b.length={valueOf:function(){n++;if(n===2)Object.freeze(b);return 1;}};
if(n!==2||!Object.isFrozen(b)||b.length!==1)throw 'assignment live same value';
var c=[0,1], count=0, caught=0;
try{Object.defineProperty(c,'length',{value:{valueOf:function(){count++;return count;}}});}catch(e){if(e.name==='RangeError')caught++;}
if(count!==2||caught!==1||c.length!==2)throw 'mismatched conversions';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_reject_invalid_lengths_and_preserve_exception_order() {
        succeeds(
            r#"
var a=[1,2], invalid=[NaN,Infinity,-Infinity,-1,1.5,4294967296,undefined,'bad'], caught=0;
for(var i=0;i<invalid.length;i++){try{Object.defineProperty(a,'length',{value:invalid[i]});}catch(e){if(e.name==='RangeError')caught++;}}
if(caught!==invalid.length||a.length!==2||a[1]!==2)throw 'invalid lengths';
var calls=0, marker=Symbol('throw'), value={valueOf:function(){calls++;if(calls===2)throw marker;return 1;}};
try{Object.defineProperty(a,'length',{value:value,enumerable:true});}catch(e){if(e===marker)caught++;}
if(calls!==2||caught!==9||a.length!==2)throw 'coercion before attributes';
calls=0;
try{Object.defineProperty(a,'length',{value:value,get:function(){return 1;}});}catch(e){if(e.name==='TypeError')caught++;}
if(calls||caught!==10)throw 'descriptor validation before value conversion';
try{Object.defineProperty(a,'length',{get:function(){return 1;}});}catch(e){if(e.name==='TypeError')caught++;}
try{Object.defineProperty(a,'length',{enumerable:true});}catch(e){if(e.name==='TypeError')caught++;}
try{Object.defineProperty(a,'length',{configurable:true});}catch(e){if(e.name==='TypeError')caught++;}
try{Object.defineProperty(a,'length',{value:Symbol('length')});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==14||a.length!==2||a[1]!==2)throw 'attributes';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_mutators_throw_and_preserve_partial_changes() {
        succeeds(
            r#"
var a=[1,2], caught=0;Object.defineProperty(a,'length',{writable:false});
try{a.pop();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==1||a.length!==2||1 in a||a[0]!==1)throw 'pop deletion before length';
try{a.push(3);}catch(e){if(e.name==='TypeError')caught++;}
try{a.push();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3||a.length!==2||2 in a)throw 'push failure';
var b=[1,2,3];Object.defineProperty(b,'1',{writable:false});
try{b.splice(0,1);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==4||b.length!==3||b[0]!==2||b[1]!==2||b[2]!==3)throw 'splice partial';
var c=[1,2];Object.defineProperty(c,'1',{configurable:false});
try{c.pop();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==5||c.length!==2||c[1]!==2)throw 'pop nonconfigurable';
var hits=0,d=Object.create({});Object.setPrototypeOf(d,Array.prototype);
Object.defineProperty(d,'0',{set:function(v){hits+=v;},configurable:true});d.length=0;
if(Array.prototype.push.call(d,5)!==1||hits!==5||d.length!==1)throw 'generic inherited accessor';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_define_properties_snapshots_before_partial_application() {
        succeeds(
            r#"
var a=[0], fields={}, order='';
Object.defineProperty(fields,'2',{enumerable:true,get:function(){order+='i';return {value:2,enumerable:true,configurable:true};}});
Object.defineProperty(fields,'length',{enumerable:true,get:function(){order+='l';if(a.length!==1)throw 'early definition';return {value:2,writable:false};}});
Object.defineProperties(a,fields);
if(order!=='il'||a.length!==2||2 in a||Object.getOwnPropertyDescriptor(a,'length').writable)throw 'snapshot and shrink';
var b=[];Object.defineProperty(b,'1',{value:NaN});
Object.defineProperty(b,'1',{value:NaN});
var caught=0;try{Object.defineProperty(b,'1',{value:0});}catch(e){if(e.name==='TypeError')caught++;}
Object.defineProperty(b,'0',{value:-0});
try{Object.defineProperty(b,'0',{value:0});}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==2||b.length!==2||1/b[0]!==-Infinity)throw 'nonconfigurable SameValue';
Object.defineProperty(b,'4294967295',{value:4});Object.defineProperty(b,'-0',{value:5});
if(b.length!==2||b['4294967295']!==4||b['-0']!==5)throw 'non-index names';
'ok';
"#,
        );
    }

    #[test]
    fn array_descriptors_enforce_existing_limits_without_length_or_property_growth() {
        let mut runtime = Runtime::new();
        runtime.execute("var capped=[1,2];", &mut NoHost).unwrap();
        for source in [
            "Object.defineProperty(capped,'length',{value:8193,writable:false});",
            "Object.defineProperty(capped,'8192',{value:3});",
        ] {
            runtime.begin_job();
            let error = runtime.execute(source, &mut NoHost).unwrap_err();
            assert!(error.to_string().contains("Array length limit"), "{error}");
            runtime.begin_job();
            assert_eq!(runtime.execute("capped.length===2 && capped[1]===2 && !capped.hasOwnProperty(8192) && Object.getOwnPropertyDescriptor(capped,'length').writable;", &mut NoHost).unwrap().to_text(), "true");
        }
        runtime.begin_job();
        runtime
            .execute(
                "Object.defineProperty(capped,'length',{value:8192});var shrink={value:0};",
                &mut NoHost,
            )
            .unwrap();
        let before = runtime.properties;
        runtime.begin_job();
        runtime
            .execute(
                "Object.defineProperty(capped,'length',shrink);",
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(runtime.properties, before - 2);
    }
}
