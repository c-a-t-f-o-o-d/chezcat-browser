//! RGBA8 ImageData uses the same binary storage and realm prototypes as script-created views.
use super::binary_data::{Data as Binary, Kind};
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Data {
    width: u32,
    height: u32,
    pub(super) view: usize,
}

#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    Width,
    Height,
    Data,
    PixelFormat,
    ColorSpace,
}

#[derive(Default)]
struct Settings {
    non_srgb: bool,
    float16: bool,
}

impl Runtime {
    pub(super) fn initialize_image_data(&mut self) -> Result<(), Fault> {
        let prototype = self.new_object(None)?;
        self.image_data_prototype = Some(prototype);
        let constructor = self.native_function(Builtin::ImageData(Op::Constructor), "ImageData")?;
        self.put_binding(
            self.global_environment,
            "ImageData".into(),
            Some(constructor),
            Declaration::Var,
            false,
        )?;
        self.constructor_prototype("ImageData", Value::Object(prototype))?;
        for (name, op) in [
            ("width", Op::Width),
            ("height", Op::Height),
            ("data", Op::Data),
            ("pixelFormat", Op::PixelFormat),
            ("colorSpace", Op::ColorSpace),
        ] {
            let Value::Function(get) =
                self.native_function(Builtin::ImageData(op), &format!("get {name}"))?
            else {
                unreachable!()
            };
            self.store_accessor_property(
                prototype,
                name.into(),
                Accessors {
                    get: Some(get),
                    set: None,
                },
                (true, true),
            )?;
        }
        self.store_symbol_property(
            prototype,
            self.symbol_to_string_tag.unwrap(),
            Value::String("ImageData".into()),
            Some((false, false, true)),
        )
    }

    fn image_data_record(&mut self, width: u32, height: u32, view: usize) -> Result<Value, Fault> {
        let object = self.new_object(None)?;
        self.objects[object].prototype = self.image_data_prototype.map(Ordinary::Object);
        self.objects[object].binary = Some(Binary::ImageData(Data {
            width,
            height,
            view,
        }));
        Ok(Value::Object(object))
    }

    fn image_data_exception(&mut self, name: &str, message: &str) -> Fault {
        match self.native_dom_exception(name, message) {
            Ok(reason) => Fault::thrown(reason),
            Err(fault) => fault,
        }
    }

    fn allocate_rgba_image_data(
        &mut self,
        width: u32,
        height: u32,
        initialize: impl FnOnce(&mut Self, usize, usize) -> Result<(), Fault>,
    ) -> Result<Value, Fault> {
        if width == 0 || height == 0 {
            return Err(self
                .image_data_exception("IndexSizeError", "ImageData dimensions must be positive."));
        }
        let length = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| Fault::new("ImageData exceeds the bounded ArrayBuffer byte limit."))?;
        self.ensure_global_intrinsics("ImageData")?;
        self.ensure_global_intrinsics("Uint8ClampedArray")?;
        // Only fresh objects are created below. All lazy intrinsic setup, conversions and
        // author callbacks precede this transaction. Used work is never refunded.
        let first = self.objects.len();
        let retained = self.retained_binary;
        let result = (|| {
            let view = self.new_typed(Kind::U8Clamped, length)?;
            let image = self.image_data_record(width, height, view)?;
            let buffer = self.typed_view(view).expect("fresh clamped view").buffer;
            initialize(self, buffer, length)?;
            Ok(image)
        })();
        if result.is_err() {
            self.objects.truncate(first);
            self.value_bytes -= self.retained_binary - retained;
            self.retained_binary = retained;
        }
        result
    }

    /// The caller has entered the canvas method's realm. The producer receives only the
    /// destination bytes and cannot publish a partially filled wrapper or call author code.
    pub(super) fn new_rgba_image_data(
        &mut self,
        width: u32,
        height: u32,
        fill: impl FnOnce(&mut [u8]) -> Result<(), String>,
    ) -> Result<Value, Fault> {
        self.allocate_rgba_image_data(width, height, |runtime, buffer, length| {
            runtime.work(length)?;
            fill(runtime.buffer_bytes_mut(buffer)?).map_err(Fault::new)
        })
    }

    pub(super) fn image_data_builtin(&mut self, op: Op, this: Value) -> Result<Value, Fault> {
        self.step()?;
        if matches!(op, Op::Constructor) {
            return Err(Fault::exception("TypeError", "ImageData requires new."));
        }
        let Some(Binary::ImageData(data)) = (match this {
            Value::Object(id) => self.object(id)?.binary.as_ref(),
            _ => None,
        }) else {
            return Err(Fault::exception(
                "TypeError",
                "ImageData accessor requires a genuine ImageData receiver.",
            ));
        };
        let value = match op {
            Op::Width => Value::Number(f64::from(data.width)),
            Op::Height => Value::Number(f64::from(data.height)),
            Op::Data => Value::Object(data.view),
            Op::PixelFormat => Value::String("rgba-unorm8".into()),
            Op::ColorSpace => Value::String("srgb".into()),
            Op::Constructor => unreachable!(),
        };
        self.checked_value(value)
    }

    fn image_data_settings(
        &mut self,
        options: Option<&Value>,
        host: &mut dyn Host,
    ) -> Result<Settings, Fault> {
        let mut settings = Settings::default();
        let Some(options) = options.filter(|v| !matches!(v, Value::Null | Value::Undefined)) else {
            return Ok(settings);
        };
        if !matches!(
            options,
            Value::Object(_) | Value::Function(_) | Value::Host(_)
        ) {
            return Err(Fault::exception(
                "TypeError",
                "ImageDataSettings requires an object dictionary.",
            ));
        }
        for name in ["colorSpace", "pixelFormat"] {
            let value = self.read_property(options, name, host)?;
            if matches!(value, Value::Undefined) {
                continue;
            }
            let primitive = self.primitive_with_hint(value, true, host)?;
            self.work(primitive.bytes())?;
            let value = self.checked_value(Value::String(primitive.script_text()?))?;
            canvas_readback::validate_setting(name, &value)
                .map_err(|(kind, message)| Fault::exception(kind, message))?;
            let Value::String(text) = value else {
                unreachable!()
            };
            if name == "colorSpace" {
                settings.non_srgb = text != "srgb";
            } else {
                settings.float16 = text == "rgba-float16";
            }
        }
        Ok(settings)
    }

    pub(super) fn image_data_construct(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        self.check_native_call_stack("ImageData conversion call")?;
        self.call_depth += 1;
        let result = self.image_data_construct_inner(args, host);
        self.call_depth -= 1;
        result
    }

    fn image_data_construct_inner(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        use Kind::{U8Clamped, F16};
        if args.len() < 2 {
            return Err(Fault::exception(
                "TypeError",
                "ImageData requires at least two arguments.",
            ));
        }
        let source = match args[0] {
            Value::Object(id) => self
                .typed_view(id)
                .filter(|v| matches!(v.kind, U8Clamped | F16))
                .map(|v| (id, v)),
            _ => None,
        };
        if args.len() >= 4 && source.is_none() {
            return Err(Fault::exception(
                "TypeError",
                "The four-argument ImageData constructor requires an ImageDataArray.",
            ));
        }
        let offset = usize::from(source.is_some());
        let width = int32(self.binary_number(&args[offset], host)?) as u32;
        let height_arg = args.get(offset + 1);
        let height = if source.is_some() && matches!(height_arg, None | Some(Value::Undefined)) {
            None
        } else {
            Some(int32(self.binary_number(height_arg.unwrap_or(&Value::Undefined), host)?) as u32)
        };
        let settings = self.image_data_settings(args.get(offset + 2), host)?;
        let height = if let Some((_, view)) = source {
            let bytes = view.length * if view.kind == F16 { 2 } else { 1 };
            let pixel_bytes = if settings.float16 { 8 } else { 4 };
            if bytes == 0 || !bytes.is_multiple_of(pixel_bytes) {
                return Err(self.image_data_exception(
                    "InvalidStateError",
                    "ImageDataArray needs an integral, nonzero number of pixels.",
                ));
            }
            let pixels = bytes / pixel_bytes;
            if width == 0 || !pixels.is_multiple_of(width as usize) {
                return Err(self.image_data_exception(
                    "IndexSizeError",
                    "ImageDataArray length does not fit its width.",
                ));
            }
            let actual = (pixels / width as usize) as u32;
            if height.is_some_and(|requested| requested != actual) {
                return Err(self.image_data_exception(
                    "IndexSizeError",
                    "ImageDataArray length does not fit its height.",
                ));
            }
            if settings.float16 != (view.kind == F16) {
                return Err(self.image_data_exception(
                    "InvalidStateError",
                    "ImageDataArray type does not match the pixel format.",
                ));
            }
            actual
        } else {
            let height = height.expect("dimension overload supplies height");
            if width == 0 || height == 0 {
                return Err(self.image_data_exception(
                    "IndexSizeError",
                    "ImageData dimensions must be positive.",
                ));
            }
            height
        };
        if settings.float16 || settings.non_srgb {
            return Err(self.image_data_exception(
                "NotSupportedError",
                "Only sRGB rgba-unorm8 ImageData is supported.",
            ));
        }
        if let Some((view, _)) = source {
            self.image_data_record(width, height, view)
        } else {
            self.allocate_rgba_image_data(width, height, |_, _, _| Ok(()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("unexpected host read")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("unexpected host write")
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            panic!("unexpected host call")
        }
    }

    #[test]
    fn image_data_brands_descriptors_live_offset_view_and_zeroed_independent_storage() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var a=new ImageData(2,1),b=new ImageData(2,1);
if(ImageData.length!==2||a.constructor!==ImageData||!(a instanceof ImageData)||Object.getPrototypeOf(a)!==ImageData.prototype||Object.prototype.toString.call(a)!=='[object ImageData]')throw 'identity';
if(a.width!==2||a.height!==1||a.colorSpace!=='srgb'||a.pixelFormat!=='rgba-unorm8'||Object.keys(a).length!==0)throw 'metadata';
if(a.data!==a.data||a.data===b.data||!(a.data instanceof Uint8ClampedArray)||a.data.buffer===b.data.buffer||a.data.byteOffset!==0||a.data.byteLength!==8||a.data.buffer.byteLength!==8||ArrayBuffer.isView(a))throw 'native view';
for(var i=0;i<8;i++){if(a.data[i]!==0)throw 'initialization';}
for(var key of ['width','height','data','pixelFormat','colorSpace']){
    var d=Object.getOwnPropertyDescriptor(ImageData.prototype,key),caught=false;
    if(d.get.length!==0||d.set!==undefined||!d.enumerable||!d.configurable||Object.hasOwn(a,key))throw 'accessor descriptor';
    try{d.get.call(Object.create(ImageData.prototype));}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'borrowed accessor brand';
}
var caught=false;try{(function(){'use strict';a.width=9;})();}catch(e){caught=e instanceof TypeError;}if(!caught||a.width!==2)throw 'readonly';
var buffer=new ArrayBuffer(16),off=new Uint8ClampedArray(buffer,4,8),c=new ImageData(off,1,undefined,{colorSpace:'srgb',pixelFormat:'rgba-unorm8'});
if(c.width!==1||c.height!==2||c.data!==off||c.data.byteOffset!==4)throw 'source view';
c.data[0]=300;off[7]=77;
if(new Uint8Array(buffer)[4]!==255||c.data[7]!==77||a.data[0]!==0||b.data[7]!==0)throw 'live bytes';
"#, &mut NoHost).unwrap();
    }

    #[test]
    fn image_data_overloads_order_branded_exceptions_and_abrupt_settings() {
        let mut runtime = Runtime::for_test();
        runtime.execute(r#"
var log='',marker={};
function number(label,n){return {valueOf:function(){log+=label;return n;}};}
var settings={get colorSpace(){log+='C';return {toString:function(){log+='c';return 'srgb';}};},get pixelFormat(){log+='P';return 'rgba-unorm8';}};
var a=new ImageData(number('W',4294967297),number('H',1.9),settings);
if(a.width!==1||a.height!==1||log!=='WHCcP')throw 'unsigned long and settings order';
log='';var caught=false;try{new ImageData(0,0,{get colorSpace(){throw marker;},get pixelFormat(){log+='bad';}});}catch(e){caught=e===marker;}
if(!caught||log!=='')throw 'dictionary conversion before dimension validation';
function expect(f,type,name){var e;try{f();}catch(value){e=value;}if(!(e instanceof type)||e.name!==name)throw name;}
expect(function(){ImageData(1,2);},TypeError,'TypeError');
expect(function(){new ImageData(1);},TypeError,'TypeError');
expect(function(){new ImageData(1,1,3);},TypeError,'TypeError');
expect(function(){new ImageData(1,1,{colorSpace:'SRGB'});},TypeError,'TypeError');
expect(function(){new ImageData(0,1);},DOMException,'IndexSizeError');
expect(function(){new ImageData(new Uint8ClampedArray(0),1);},DOMException,'InvalidStateError');
expect(function(){new ImageData(new Uint8ClampedArray(3),1);},DOMException,'InvalidStateError');
expect(function(){new ImageData(new Uint8ClampedArray(8),3);},DOMException,'IndexSizeError');
expect(function(){new ImageData(new Uint8ClampedArray(8),1,1);},DOMException,'IndexSizeError');
expect(function(){new ImageData(new Float16Array(4),1);},DOMException,'InvalidStateError');
expect(function(){new ImageData(1,1,{pixelFormat:'rgba-float16'});},DOMException,'NotSupportedError');
expect(function(){new ImageData(1,1,{colorSpace:'display-p3'});},DOMException,'NotSupportedError');
"#, &mut NoHost).unwrap();
    }

    #[test]
    fn image_data_snapshots_use_the_active_realm_and_survive_global_replacement() {
        let mut runtime = Runtime::for_capacity_test();
        let realm = runtime.allocate_intrinsic_realm().unwrap();
        let (image, constructor, view) = runtime
            .with_intrinsic_realm(Some(realm), |runtime| {
                let image = runtime.new_rgba_image_data(1, 1, |bytes| {
                    bytes.copy_from_slice(&[5, 7, 9, 11]);
                    Ok(())
                })?;
                let ctor = runtime.binding_value(runtime.global_environment, "ImageData")?;
                let view =
                    runtime.binding_value(runtime.global_environment, "Uint8ClampedArray")?;
                Ok((image, ctor, view))
            })
            .unwrap();
        runtime.define_global("childPixel", image).unwrap();
        runtime
            .define_global("ChildImageData", constructor)
            .unwrap();
        runtime.define_global("ChildClamped", view).unwrap();
        runtime.execute(r#"
if(!(childPixel instanceof ChildImageData)||childPixel instanceof ImageData||!(childPixel.data instanceof ChildClamped)||childPixel.data instanceof Uint8ClampedArray)throw 'snapshot realm';
var foreign=new ChildImageData(1,2);if(Object.getPrototypeOf(foreign)!==ChildImageData.prototype||foreign.data.constructor!==ChildClamped)throw 'saved constructor';
var getter=Object.getOwnPropertyDescriptor(ImageData.prototype,'data').get;
if(getter.call(childPixel)!==childPixel.data||childPixel.data[0]!==5||childPixel.data[3]!==11)throw 'cross-realm accessor';
var saved=ImageData.prototype;delete saved.constructor;ImageData=3;
"#, &mut NoHost).unwrap();
        let image = runtime.new_rgba_image_data(1, 1, |_| Ok(())).unwrap();
        runtime.define_global("snapshot", image).unwrap();
        runtime.execute("if(Object.getPrototypeOf(snapshot)!==saved||ImageData!==3||snapshot.data[0]!==0)throw 'canonical allocation';",&mut NoHost).unwrap();
    }

    #[test]
    fn image_data_failed_snapshot_refunds_unpublished_bytes_and_identities() {
        let mut runtime = Runtime::for_test();
        runtime.new_rgba_image_data(1, 1, |_| Ok(())).unwrap();
        let usage = (runtime.objects.len(), runtime.retained_binary, runtime.value_bytes);
        let err = runtime
            .new_rgba_image_data(1, 1, |bytes| {
                bytes[0] = 17;
                Err("producer failed".into())
            })
            .unwrap_err();
        assert_eq!(err.message, "producer failed");
        assert_eq!((runtime.objects.len(), runtime.retained_binary, runtime.value_bytes), usage);
        let limit = runtime.limits.heap.objects;
        runtime.limits.heap.objects = usage.0 + 2;
        let err = runtime
            .new_rgba_image_data(1, 1, |_| panic!("wrapper quota must precede fill"))
            .unwrap_err();
        assert!(err.message.contains("Object identity limit"), "{err:?}");
        assert_eq!((runtime.objects.len(), runtime.retained_binary, runtime.value_bytes), usage);
        runtime.limits.heap.objects = limit;
        let err = runtime
            .new_rgba_image_data(2048, 2048, |_| panic!("overlarge buffer"))
            .unwrap_err();
        assert!(err.message.contains("Retained value limit"), "{err:?}");
        assert_eq!((runtime.objects.len(), runtime.retained_binary, runtime.value_bytes), usage);
        let _ = runtime
            .new_rgba_image_data(1, 1, |bytes| {
                assert_eq!(bytes, &[0; 4]);
                Ok(())
            })
            .unwrap();
    }
}
