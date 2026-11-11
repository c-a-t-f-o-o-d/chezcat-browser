//! Original bounded Object name, descriptor, value and entry inspection.
//! ECMA-262 Object.getOwnPropertyNames/getOwnPropertyDescriptors/values/entries
//! and EnumerableOwnProperties define the key snapshots and getter ordering:
//! https://tc39.es/ecma262/multipage/fundamental-objects.html
//! https://tc39.es/ecma262/multipage/abstract-operations.html#sec-enumerableownproperties

use super::{
    property_index, Fault, Host, Ordinary, Reference, Runtime, Value, MAX_OBJECT_PROPERTIES,
};

impl Runtime {
    pub(super) fn object_inspection(
        &mut self,
        method: &str,
        value: &Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        Self::require_property_receiver(value)?;
        if let Value::String(text) = value {
            return self.inspection_string(method, text);
        }
        let ordinary = Ordinary::from_value(value);
        let keys = if let Some(object) = ordinary {
            let id = self.ordinary_id(object)?;
            self.inspection_string_keys(id)?
        } else if let Value::Host(handle) = value {
            if let Some(&id) = self.host_objects.get(handle) {
                self.inspection_string_keys(id)?
            } else {
                // The Host contract supplies only its explicitly supported
                // own names. Unsupported native objects still fail closed.
                self.host_keys(*handle, false, host)?
            }
        } else {
            Vec::new() // Number, Boolean and Symbol primitives have no own keys.
        };
        if method == "getOwnPropertyNames" {
            let result = self.new_object(Some(keys.len()))?;
            for (index, key) in keys.into_iter().enumerate() {
                self.step()?;
                self.set_property(result, index.to_string(), Value::String(key))?;
            }
            return Ok(Value::Object(result));
        }
        if method == "getOwnPropertyDescriptors" {
            // Snapshot symbols before any descriptor retrieval, and preserve
            // their identities rather than converting them to string keys.
            let symbols = if let Some(object) = self.symbol_backing(value)? {
                let id = self.ordinary_id(object)?;
                self.own_symbol_keys(id, false)?
            } else {
                Vec::new()
            };
            if keys.len().saturating_add(symbols.len()) > MAX_OBJECT_PROPERTIES {
                return Err(Fault::new(
                    "Property enumeration limit reached (4096 keys).",
                ));
            }
            let result = self.new_object(None)?;
            for key in keys {
                self.step()?;
                let receiver = self.copy_value(value)?;
                self.work(key.len())?;
                let descriptor = self.object_static(
                    "getOwnPropertyDescriptor",
                    &[receiver, Value::String(key.clone())],
                    host,
                )?;
                if !matches!(descriptor, Value::Undefined) {
                    self.set_property(result, key, descriptor)?;
                }
            }
            for key in symbols {
                self.step()?;
                let receiver = self.copy_value(value)?;
                let descriptor = self.object_static(
                    "getOwnPropertyDescriptor",
                    &[receiver, Value::Symbol(key)],
                    host,
                )?;
                if !matches!(descriptor, Value::Undefined) {
                    self.store_symbol_property(result, key, descriptor, Some((true, true, true)))?;
                }
            }
            return Ok(Value::Object(result));
        }
        if !matches!(method, "values" | "entries") {
            return Err(Fault::new("Unsupported Object inspection method."));
        }
        // Gather directly into an unexposed array so retained-value and arena
        // limits apply incrementally, rather than collecting unbounded values.
        let result = self.new_object(Some(0))?;
        let mut index = 0;
        for key in keys {
            self.step()?;
            let enumerable = if let Value::Host(handle) = value {
                self.host_owned(*handle, &key, true, host)?
            } else if let Some(object) = ordinary {
                self.own_enumerable(self.ordinary_id(object)?, &key)?
            } else {
                false
            };
            if !enumerable {
                continue;
            }
            // Earlier getters can delete, replace or change the enumerability
            // of a later property. Added keys are absent from this snapshot.
            let receiver = self.copy_value(value)?;
            self.work(key.len())?;
            let next = self.read_reference(&Reference::Property(receiver, key.clone()), host)?;
            let next = if method == "entries" {
                self.array_from_values(&[Value::String(key), next])?
            } else {
                next
            };
            // Create own data properties; inherited setters on Array.prototype
            // or Object.prototype must not intercept any result construction.
            self.set_property(result, index.to_string(), next)?;
            index += 1;
        }
        Ok(Value::Object(result))
    }

    fn inspection_string_keys(&mut self, id: usize) -> Result<Vec<String>, Fault> {
        let mut keys = self.own_string_keys(id, false)?;
        if self.object(id)?.length.is_some() && !self.objects[id].properties.contains_key("length")
        {
            // Array length is an implicit own property, created before the
            // stored string properties but ordered after all integer indices.
            if keys.len() >= MAX_OBJECT_PROPERTIES {
                return Err(Fault::new(
                    "Property enumeration limit reached (4096 keys).",
                ));
            }
            self.work(6)?;
            let at = keys.partition_point(|key| property_index(key).is_some());
            keys.insert(at, "length".into());
        }
        Ok(keys)
    }

    fn inspection_string(&mut self, method: &str, text: &str) -> Result<Value, Fault> {
        self.work(text.len())?;
        let length = text.encode_utf16().count();
        let all = matches!(method, "getOwnPropertyNames" | "getOwnPropertyDescriptors");
        let count = length.saturating_add(usize::from(all));
        if count > MAX_OBJECT_PROPERTIES {
            return Err(Fault::new(
                "Property enumeration limit reached (4096 keys).",
            ));
        }
        if method == "getOwnPropertyNames" {
            let result = self.new_object(Some(count))?;
            for index in 0..length {
                self.step()?;
                self.set_property(result, index.to_string(), Value::String(index.to_string()))?;
            }
            self.set_property(result, length.to_string(), Value::String("length".into()))?;
            return Ok(Value::Object(result));
        }
        self.work(length.saturating_mul(2))?;
        let units: Vec<_> = text.encode_utf16().collect();
        let descriptors = method == "getOwnPropertyDescriptors";
        let result = self.new_object(if descriptors { None } else { Some(length) })?;
        for index in 0..length {
            self.step()?;
            // A supplementary character's individual code units cannot be
            // represented by this runtime's UTF-8 strings. Preserve that
            // existing capability error instead of replacing or merging them.
            let value = Value::String(self.units_string(&units[index..index + 1])?);
            let key = index.to_string();
            let value = if descriptors {
                self.inspection_string_descriptor(value, true)?
            } else if method == "entries" {
                self.array_from_values(&[Value::String(key.clone()), value])?
            } else {
                value
            };
            self.set_property(result, key, value)?;
        }
        if descriptors {
            let descriptor =
                self.inspection_string_descriptor(Value::Number(length as f64), false)?;
            self.set_property(result, "length".into(), descriptor)?;
        }
        Ok(Value::Object(result))
    }

    fn inspection_string_descriptor(
        &mut self,
        value: Value,
        enumerable: bool,
    ) -> Result<Value, Fault> {
        let result = self.new_object(None)?;
        for (name, value) in [
            ("value", value),
            ("writable", Value::Bool(false)),
            ("enumerable", Value::Bool(enumerable)),
            ("configurable", Value::Bool(false)),
        ] {
            self.set_property(result, name.into(), value)?;
        }
        Ok(Value::Object(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn owns_property(&self, object: u32, property: &str) -> bool {
            object != 17 || property == "native"
        }
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

    fn result(source: &str) -> String {
        Runtime::new()
            .execute(source, &mut NoHost)
            .unwrap()
            .to_text()
    }

    #[test]
    fn inspection_orders_own_names_and_excludes_symbols_from_values_and_entries() {
        assert_eq!(result(r#"
var p={inherited:99},o=Object.create(p),s=Symbol('own');
o.beta=1;o[9]='nine';o[2]='two';o['01']='leading';
Object.defineProperty(o,'hidden',{value:7});o[s]=8;delete o.beta;o.beta='last';
var d=Object.getOwnPropertyDescriptors(o);
[Object.getOwnPropertyNames(o).join(','),Object.values(o).join(','),JSON.stringify(Object.entries(o)),
 Object.getOwnPropertySymbols(d)[0]===s,d[s].value,d.hidden.enumerable,d.hidden.writable,
 d.hidden.configurable,Object.getPrototypeOf(d)===Object.prototype].join('|');
"#), "2,9,01,hidden,beta|two,nine,leading,last|[[\"2\",\"two\"],[\"9\",\"nine\"],[\"01\",\"leading\"],[\"beta\",\"last\"]]|true|8|false|false|false|true");
        assert_eq!(
            result(
                r#"
var a=[3,,5];a.extra=8;var d=Object.getOwnPropertyDescriptors(a);
[Object.getOwnPropertyNames(a).join(','),Object.values(a).join(','),d.length.value,
d.length.enumerable,d.length.writable,d.length.configurable,
Object.getOwnPropertyNames(new Uint8Array([7,8])).join(',')].join('|');
"#
            ),
            "0,2,length,extra|3,5,8|3|false|true|false|0,1"
        );
    }

    #[test]
    fn inspection_coerces_primitives_without_stringifying_them() {
        assert_eq!(result(r#"
var methods=['getOwnPropertyNames','getOwnPropertyDescriptors','values','entries'],errors=0;
for(var i=0;i<methods.length;i++){
try{Object[methods[i]]();}catch(e){if(e.name==='TypeError')errors++;}
try{Object[methods[i]](null);}catch(e){if(e.name==='TypeError')errors++;}
}
var d=Object.getOwnPropertyDescriptors('A\u00e9');
[errors,Object.getOwnPropertyNames('A\u00e9').join(','),Object.values('A\u00e9').join(','),
 JSON.stringify(Object.entries('A\u00e9')),d[1].value,d[1].writable,d[1].enumerable,d[1].configurable,
 d.length.value,d.length.enumerable,d.length.writable,d.length.configurable,
 Object.values(4).length,Object.entries(false).length,Object.getOwnPropertyNames(Symbol('x')).length,
 Object.keys(Object.getOwnPropertyDescriptors(true)).length].join('|');
"#), "8|0,1,length|A,é|[[\"0\",\"A\"],[\"1\",\"é\"]]|é|false|true|false|2|false|false|false|0|0|0|0");
    }

    #[test]
    fn inspection_snapshots_names_then_rechecks_presence_and_enumerability() {
        assert_eq!(
            result(
                r#"
var log='';function make(){
 var p={};Object.defineProperty(p,'removed',{get:function(){throw 'inherited';}});
 var o=Object.create(p);
 Object.defineProperty(o,'first',{enumerable:true,get:function(){log+='a';
  if(this!==o)throw 'receiver';delete o.removed;
  Object.defineProperty(o,'hiddenLater',{enumerable:false});
  Object.defineProperty(o,'shownLater',{enumerable:true});o.added=5;return 1;}});
 Object.defineProperty(o,'removed',{value:2,enumerable:true,configurable:true,writable:true});
 Object.defineProperty(o,'hiddenLater',{enumerable:true,configurable:true,get:function(){throw 'hidden';}});
 Object.defineProperty(o,'shownLater',{enumerable:false,configurable:true,get:function(){log+='d';return 4;}});
 return o;
}
var a=Object.values(make()),b=Object.entries(make());
[a.join(','),JSON.stringify(b),log].join('|');
"#
            ),
            "1,4|[[\"first\",1],[\"shownLater\",4]]|adad"
        );
    }

    #[test]
    fn inspection_descriptors_do_not_call_getters_and_preserve_function_metadata() {
        assert_eq!(
            result(
                r#"
var count=0,s=Symbol('accessor'),o={},getter=function(){count++;return 6;},setter=function(v){count+=v;};
Object.defineProperty(o,'answer',{get:getter,set:setter,enumerable:true,configurable:true});
Object.defineProperty(o,s,{get:getter});
var names=Object.getOwnPropertyNames(o),d=Object.getOwnPropertyDescriptors(o);
function sample(x,y){return x+y;}sample.extra=3;
var f=Object.getOwnPropertyDescriptors(sample);
[count,names.join(','),d.answer.get===getter,d.answer.set===setter,!Object.hasOwn(d.answer,'value'),
 d[s].get===getter,d[s].enumerable,f.length.value,f.name.value,f.prototype.value===sample.prototype,
 Object.values(sample).join(','),Object.getOwnPropertyNames(sample).indexOf('prototype')>=0].join('|');
"#
            ),
            "0|answer|true|true|true|true|false|2|sample|true|3|true"
        );
    }

    #[test]
    fn inspection_propagates_getter_exceptions_and_creates_own_result_properties() {
        assert_eq!(
            result(
                r#"
var marker={},calls=0,o={};
Object.defineProperty(o,'first',{enumerable:true,get:function(){throw marker;}});
Object.defineProperty(o,'later',{enumerable:true,get:function(){calls++;return 2;}});
var caught=0;try{Object.values(o);}catch(e){if(e===marker)caught++;}
try{Object.entries(o);}catch(e){if(e===marker)caught++;}
var source={value:7};
Object.defineProperty(Object.prototype,'0',{configurable:true,set:function(){calls++;}});
Object.defineProperty(Object.prototype,'value',{configurable:true,set:function(){calls++;}});
var values=Object.values(source),entries=Object.entries(source),descriptors=Object.getOwnPropertyDescriptors(source);
var good=values[0]===7&&entries[0][0]==='value'&&entries[0][1]===7&&descriptors.value.value===7;
delete Object.prototype['0'];delete Object.prototype.value;
[caught,calls,good].join('|');
"#
            ),
            "2|0|true"
        );
    }

    #[test]
    fn inspection_host_expandos_remain_scoped_and_use_original_receivers() {
        let mut runtime = Runtime::new();
        runtime.bind_host_object(17, &[]).unwrap();
        runtime.define_global("node", Value::Host(17)).unwrap();
        let value = runtime
            .execute(
                r#"
node.own=2;Object.defineProperty(node,'hidden',{value:3});
Object.defineProperty(node,'answer',{enumerable:true,get:function(){return this===node?4:99;}});
var s=Symbol('node');node[s]=5;
var d=Object.getOwnPropertyDescriptors(node);
[Object.getOwnPropertyNames(node).join(','),Object.values(node).join(','),d.hidden.value,
d[s].value,typeof d.answer.get].join('|');
"#,
                &mut NoHost,
            )
            .unwrap();
        assert_eq!(value.to_text(), "own,hidden,answer|2,4|3|5|function");
        for method in [
            "getOwnPropertyNames",
            "getOwnPropertyDescriptors",
            "values",
            "entries",
        ] {
            runtime.begin_job();
            let error = runtime
                .object_inspection(method, &Value::Host(99), &mut NoHost)
                .unwrap_err();
            assert!(error.exception.is_none());
            assert!(error
                .message
                .contains("Enumerating this native host object"));
        }
    }

    #[test]
    fn inspection_keeps_enumeration_copy_step_and_utf16_bounds() {
        let mut runtime = Runtime::new();
        let source = Value::String("x".repeat(MAX_OBJECT_PROPERTIES));
        let fault = runtime
            .object_inspection("getOwnPropertyNames", &source, &mut NoHost)
            .unwrap_err();
        assert!(fault.exception.is_none());
        assert!(fault.message.contains("Property enumeration limit"));
        runtime.begin_job();
        let source = Value::String("\u{1f408}".into());
        let names = runtime
            .object_inspection("getOwnPropertyNames", &source, &mut NoHost)
            .unwrap();
        runtime.define_global("names", names).unwrap();
        assert_eq!(
            runtime
                .execute("names.join(',');", &mut NoHost)
                .unwrap()
                .to_text(),
            "0,1,length"
        );
        for method in ["values", "entries", "getOwnPropertyDescriptors"] {
            runtime.begin_job();
            let error = runtime
                .object_inspection(method, &source, &mut NoHost)
                .unwrap_err();
            assert!(error.exception.is_none());
            assert!(error.message.contains("lone UTF-16 surrogate"));
        }
        let object = runtime.new_object(None).unwrap();
        runtime
            .set_property(object, "key".into(), Value::Number(1.0))
            .unwrap();
        runtime.begin_job();
        runtime.copy_bytes = 0;
        let error = runtime
            .object_inspection("values", &Value::Object(object), &mut NoHost)
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("String/copy work limit"));
        runtime.begin_job();
        runtime.steps = 0;
        let error = runtime
            .object_inspection("entries", &Value::Object(object), &mut NoHost)
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("Execution step limit"));
        runtime.begin_job();
        while runtime.objects.len() < super::super::MAX_OBJECTS - 1 {
            runtime.new_object(None).unwrap();
        }
        let error = runtime
            .object_inspection(
                "getOwnPropertyDescriptors",
                &Value::Object(object),
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.exception.is_none());
        assert!(error.message.contains("Object identity limit"));
    }
}
