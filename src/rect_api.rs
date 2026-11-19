//! Original DOMRect numeric slots and branded Web IDL accessors. Renderer
//! snapshots use these same prototypes; no coordinate is an ordinary own field.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Data { values: [f64; 4], mutable: bool }
impl Data {
    fn get(self, index: usize) -> f64 {
        if index < 4 { return self.values[index]; }
        let (a, b, maximum) = match index {
            4 => (self.values[1], self.values[1] + self.values[3], false),
            5 => (self.values[0], self.values[0] + self.values[2], true),
            6 => (self.values[1], self.values[1] + self.values[3], true),
            _ => (self.values[0], self.values[0] + self.values[2], false),
        };
        if a.is_nan() || b.is_nan() { return f64::NAN; }
        if a == 0.0 && b == 0.0 {
            return if maximum { if a.is_sign_positive() || b.is_sign_positive() { 0.0 } else { -0.0 } }
                else if a.is_sign_negative() || b.is_sign_negative() { -0.0 } else { 0.0 };
        }
        if maximum { if a > b { a } else { b } } else if a < b { a } else { b }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("No host read".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No host write".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("No host call".into()) }
    }
    #[test]
    fn domrect_interfaces_have_real_brands_prototypes_accessors_and_snapshots() {
        let mut runtime = Runtime::for_test();
        let value = runtime.execute(r#"
var r=new DOMRect(10,20,-4,-8),ro=new DOMRectReadOnly(1,2,3,4);
if(!(r instanceof DOMRect)||!(r instanceof DOMRectReadOnly)||ro instanceof DOMRect||!(ro instanceof DOMRectReadOnly))throw 'brands';
if(Object.getPrototypeOf(r)!==DOMRect.prototype||Object.getPrototypeOf(DOMRect.prototype)!==DOMRectReadOnly.prototype||Object.getPrototypeOf(DOMRect)!==DOMRectReadOnly)throw 'prototype chain';
if(Object.prototype.toString.call(r)!=='[object DOMRect]'||Object.prototype.toString.call(ro)!=='[object DOMRectReadOnly]')throw 'native tag';
if(r.left!==6||r.right!==10||r.top!==12||r.bottom!==20)throw 'negative dimensions';
if(Object.keys(r).length!==0||Object.hasOwn(r,'x')||Object.getOwnPropertyDescriptor(r,'width')!==undefined)throw 'internal slots';
var d=Object.getOwnPropertyDescriptor(DOMRect.prototype,'x');if(d.get.length!==0||d.set.length!==1||!d.enumerable||!d.configurable||DOMRect.length!==0||DOMRectReadOnly.length!==0||r.toJSON.length!==0)throw 'IDL descriptors';
var inherited=Object.getOwnPropertyDescriptor(DOMRectReadOnly.prototype,'x').get;if(inherited===d.get||inherited.call(ro)!==1||inherited.call(r)!==10)throw 'inherited getter steps, distinct derived function';
var tag=Object.getOwnPropertyDescriptor(DOMRect.prototype,Symbol.toStringTag),readonlyTag=Object.getOwnPropertyDescriptor(DOMRectReadOnly.prototype,Symbol.toStringTag);
if(tag.value!=='DOMRect'||tag.writable||tag.enumerable||!tag.configurable||readonlyTag.value!=='DOMRectReadOnly'||Object.prototype.toString.call(DOMRect.prototype)!=='[object DOMRect]')throw 'real interface tag metadata';
Object.defineProperty(r,Symbol.toStringTag,{configurable:true,value:'CustomRect'});if(Object.prototype.toString.call(r)!=='[object CustomRect]')throw 'ordinary tag override';delete r[Symbol.toStringTag];
var json=r.toJSON();r.x=30;r.width=7;if(json.x!==10||json.width!==-4||r.right!==37)throw 'snapshot JSON';
var effects=0,v={valueOf:function(){effects++;return 9;}};ro.x=v;if(ro.x!==1||effects!==0)throw 'readonly conversion';
var caught=false;try{(function(){'use strict';ro.x=8;})();}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';
for(var target of [{},DOMRect.prototype,Object.create(DOMRect.prototype)]){caught=false;try{d.get.call(target);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'getter brand';}
caught=false;try{d.set.call(ro,v);}catch(e){caught=e.name==='TypeError';}if(!caught||effects!==0||ro.x!==1)throw 'setter brand before conversion';
caught=false;try{d.get.call(ro);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'derived getter receiver brand';d.set.call(r);if(!isNaN(r.x))throw 'missing setter value converts undefined';
var bound=DOMRect.bind(null,1,2),made=new bound(3,4);if(!(made instanceof DOMRect)||made.width!==3||made.height!==4)throw 'bound constructor metadata';
for(var ctor of [DOMRect,DOMRectReadOnly]){caught=false;try{ctor();}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'requires new';}
'domrect-interfaces-ok';
"#, &mut NoHost).unwrap();
        assert_eq!(value.to_text(), "domrect-interfaces-ok");
    }
    #[test]
    fn domrect_unrestricted_numbers_ordered_conversions_dictionary_and_abrupt_values() {
        let mut runtime = Runtime::for_test();
        let value = runtime.execute(r#"
var log='',marker={},arg=function(name,value){return {valueOf:function(){log+=name;return value;}};};
var r=new DOMRect(arg('X',1.5),arg('Y',-0),arg('W',NaN),arg('H',Infinity));
if(log!=='XYWH'||r.x!==1.5||!Object.is(r.y,-0)||!isNaN(r.left)||!isNaN(r.right)||r.bottom!==Infinity)throw 'ordered unrestricted numbers';
var empty=new DOMRect(undefined,undefined,undefined,undefined);if(empty.x!==0||empty.y!==0||empty.width!==0||empty.height!==0)throw 'undefined defaults';
var produced=new DOMRect({valueOf:function(){return undefined;}});if(!isNaN(produced.x))throw 'object-produced undefined';
r.x=-0;r.width=0;if(!Object.is(r.left,-0)||!Object.is(r.right,0))throw 'signed zero minimum/maximum';
r.x=Infinity;r.width=-Infinity;if(!isNaN(r.left)||!isNaN(r.right))throw 'NaN-safe edges';
var caught=false;try{r.y={valueOf:function(){throw marker;}};}catch(e){caught=e===marker;}if(!caught||!Object.is(r.y,-0))throw 'setter abrupt atomicity';
caught=false;try{r.y=Symbol();}catch(e){caught=e.name==='TypeError';}if(!caught||!Object.is(r.y,-0))throw 'Symbol numeric failure';
log='';var dict={get height(){log+='h';return arg('H',4);},get width(){log+='w';return arg('W',3);},get x(){log+='x';return arg('X',1);},get y(){log+='y';return arg('Y',2);}};
var copy=DOMRect.fromRect(dict);if(log!=='hHwWxXyY'||copy.x!==1||copy.y!==2||copy.width!==3||copy.height!==4||!(copy instanceof DOMRect))throw 'dictionary getter/conversion order';
var readonly=DOMRectReadOnly.fromRect(copy);if(!(readonly instanceof DOMRectReadOnly)||readonly instanceof DOMRect||readonly.width!==3||readonly===copy)throw 'fromRect brand';
for(var input of [null,undefined]){if(DOMRect.fromRect(input).width!==0)throw 'empty dictionary';}
log='';caught=false;try{DOMRect.fromRect({get height(){throw marker;},get width(){log+='bad';return 1;}});}catch(e){caught=e===marker;}if(!caught||log!=='')throw 'dictionary abrupt identity';
'domrect-conversions-ok';
"#, &mut NoHost).unwrap();
        assert_eq!(value.to_text(), "domrect-conversions-ok");
    }
    #[test]
    fn domrect_conversion_recursion_and_retained_failure_remain_fatal_and_recover() {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(|| {
            let mut runtime = Runtime::for_test();
            let error = runtime.execute("var v={valueOf:function(){return DOMRect.fromRect({x:v}).x;}};DOMRect.fromRect({x:v});", &mut NoHost).unwrap_err();
            assert!(error.contains("call limit"), "{error}");
            runtime.begin_job();
            runtime.execute("var stable=new DOMRect(1,2,3,4);", &mut NoHost).unwrap();
            runtime.value_bytes = runtime.limits.heap.retained_values;
            let error = runtime.execute("try{new DOMRect();}catch(e){stable.x=99;}", &mut NoHost).unwrap_err();
            assert!(error.contains("retained"), "{error}");
            runtime.value_bytes = 0;
            runtime.begin_job();
            assert_eq!(runtime.execute("stable.x;", &mut NoHost).unwrap().to_number(), 1.0);
        }).unwrap().join().unwrap();
    }
}
#[derive(Default)]
pub(super) struct Roots { readonly: Option<usize>, rect: Option<usize> }
impl Roots {
    pub(super) fn reachability_edges(&self, add: &mut impl FnMut(super::function_reachability::Edge)) {
        for id in [self.readonly, self.rect].into_iter().flatten() { add(super::function_reachability::Edge::Object(id)); }
    }
}
#[derive(Clone, Copy)]
pub(super) enum Op { Constructor(bool), FromRect(bool), Get(usize, bool), Set(usize), ToJson }
impl Op { pub(super) fn arity(self) -> usize { usize::from(matches!(self, Self::Set(_))) } }
const FIELDS: [&str; 8] = ["x", "y", "width", "height", "top", "right", "bottom", "left"];

impl Runtime {
    pub(super) fn initialize_rect_api(&mut self) -> Result<(), Fault> {
        let readonly = self.new_object(None)?;
        let rect = self.new_object(None)?;
        self.rect_roots = Roots { readonly: Some(readonly), rect: Some(rect) };
        self.objects[rect].prototype = Some(Ordinary::Object(readonly));
        let mut constructors = [0; 2];
        for (index, name, mutable, prototype) in [(0, "DOMRectReadOnly", false, readonly), (1, "DOMRect", true, rect)] {
            let constructor = self.native_function(Builtin::Rect(Op::Constructor(mutable)), name)?;
            let Value::Function(id) = constructor else { unreachable!() };
            constructors[index] = id;
            self.put_binding(self.global_environment, name.into(), Some(constructor), Declaration::Var, false)?;
            self.constructor_prototype(name, Value::Object(prototype))?;
            let from = self.native_function(Builtin::Rect(Op::FromRect(mutable)), "fromRect")?;
            self.store_property(self.function_properties[id], "fromRect".into(), from, Some((true, true, true)))?;
            self.store_symbol_property(prototype, self.symbol_to_string_tag.unwrap(), Value::String(name.into()), Some((false, false, true)))?;
        }
        self.objects[self.function_properties[constructors[1]]].prototype = Some(Ordinary::Function(constructors[0]));
        for (index, name) in FIELDS.into_iter().enumerate() {
            let Value::Function(get) = self.native_function(Builtin::Rect(Op::Get(index, false)), &format!("get {name}"))? else { unreachable!() };
            self.store_accessor_property(readonly, name.into(), Accessors { get: Some(get), set: None }, (true, true))?;
            if index < 4 {
                let Value::Function(get) = self.native_function(Builtin::Rect(Op::Get(index, true)), &format!("get {name}"))? else { unreachable!() };
                let Value::Function(set) = self.native_function(Builtin::Rect(Op::Set(index)), &format!("set {name}"))? else { unreachable!() };
                // Web IDL creates a getter for the derived interface, using the
                // inherited getter steps with the derived receiver brand.
                self.store_accessor_property(rect, name.into(), Accessors { get: Some(get), set: Some(set) }, (true, true))?;
            }
        }
        let json = self.native_function(Builtin::Rect(Op::ToJson), "toJSON")?;
        self.store_property(readonly, "toJSON".into(), json, Some((true, true, true)))
    }

    pub(super) fn new_rect_value(&mut self, values: [f64; 4], mutable: bool) -> Result<Value, Fault> {
        self.ensure_global_intrinsics("DOMRect")?;
        self.work(32)?;
        let retained = self.value_bytes.checked_add(32).ok_or_else(|| Fault::new("DOMRect retained accounting overflow."))?;
        if retained > self.limits.heap.retained_values { return Err(Fault::new("DOMRect retained-value limit reached.")); }
        let object = self.new_object(None)?;
        self.objects[object].rect = Some(Data { values, mutable });
        self.objects[object].prototype = if mutable { self.rect_roots.rect } else { self.rect_roots.readonly }.map(Ordinary::Object);
        self.value_bytes = retained;
        Ok(Value::Object(object))
    }
    fn rect_number(&mut self, value: &Value, host: &mut dyn Host) -> Result<f64, Fault> {
        let value = self.copy_value(value)?;
        let primitive = self.primitive_with_hint(value, false, host)?;
        self.work(primitive.bytes())?;
        primitive.script_number()
    }
    fn rect_receiver(&self, this: &Value, mutable: bool) -> Result<usize, Fault> {
        if let Value::Object(id) = this {
            if self.object(*id)?.rect.as_ref().is_some_and(|data| !mutable || data.mutable) { return Ok(*id); }
        }
        Err(Fault::exception("TypeError", "DOMRect accessor or method has an incompatible receiver."))
    }
    pub(super) fn rect_construct(&mut self, mutable: bool, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        self.step()?;
        self.check_native_call_stack("DOMRect callback call")?;
        self.call_depth += 1;
        let result = self.rect_construct_inner(mutable, args, host);
        self.call_depth -= 1;
        result
    }
    fn rect_construct_inner(&mut self, mutable: bool, args: &[Value], host: &mut dyn Host) -> Result<Value, Fault> {
        let mut values = [0.0; 4];
        for (index, value) in args.iter().take(4).enumerate() {
            if !matches!(value, Value::Undefined) { values[index] = self.rect_number(value, host)?; }
        }
        self.new_rect_value(values, mutable)
    }
    pub(super) fn rect_builtin(&mut self, op: Op, args: &[Value], this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        self.step()?;
        self.check_native_call_stack("DOMRect callback call")?;
        self.call_depth += 1;
        let result = self.rect_builtin_inner(op, args, this, host);
        self.call_depth -= 1;
        result
    }
    fn rect_builtin_inner(&mut self, op: Op, args: &[Value], this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        match op {
            Op::Constructor(_) => Err(Fault::exception("TypeError", "DOMRect constructors require new.")),
            Op::FromRect(mutable) => {
                let mut values = [0.0; 4];
                let source = args.first().unwrap_or(&Value::Undefined);
                if !matches!(source, Value::Null | Value::Undefined) {
                    if !matches!(source, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
                        return Err(Fault::exception("TypeError", "DOMRectInit requires an object dictionary."));
                    }
                    // Web IDL converts dictionary members alphabetically, one
                    // getter and conversion at a time, preserving abrupt values.
                    for (index, key) in [(3, "height"), (2, "width"), (0, "x"), (1, "y")] {
                        let value = self.read_property(source, key, host)?;
                        if !matches!(value, Value::Undefined) { values[index] = self.rect_number(&value, host)?; }
                    }
                }
                self.new_rect_value(values, mutable)
            }
            Op::Get(index, mutable) => {
                let id = self.rect_receiver(&this, mutable)?;
                Ok(Value::Number(self.objects[id].rect.unwrap().get(index)))
            }
            Op::Set(index) => {
                let id = self.rect_receiver(&this, true)?;
                let value = args.first().unwrap_or(&Value::Undefined);
                let value = self.rect_number(value, host)?;
                self.objects[id].rect.as_mut().unwrap().values[index] = value;
                Ok(Value::Undefined)
            }
            Op::ToJson => {
                let id = self.rect_receiver(&this, false)?;
                let data = self.objects[id].rect.unwrap();
                let json = self.new_object(None)?;
                for (index, key) in FIELDS.into_iter().enumerate() { self.set_property(json, key.into(), Value::Number(data.get(index)))?; }
                Ok(Value::Object(json))
            }
        }
    }
}
