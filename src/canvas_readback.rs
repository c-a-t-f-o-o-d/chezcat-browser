//! Web IDL conversion and export of a fresh, bounded canvas pixel snapshot.
use super::{Fault, Host, Runtime, Value};
use crate::canvas_2d_bitmap::{ReadbackError, ReadbackRect};

pub(crate) const METHOD: &str = "\0CanvasRenderingContext2D.getImageData";

pub(super) fn enforce_long(number: f64) -> Result<f64, Fault> {
    let integer = number.trunc();
    if !integer.is_finite() || integer < f64::from(i32::MIN) || integer > f64::from(i32::MAX) {
        return Err(Fault::exception(
            "TypeError",
            "Canvas readback requires signed 32-bit coordinates.",
        ));
    }
    Ok(integer)
}

pub(crate) fn validate_setting(name: &str, value: &Value) -> Result<(), (&'static str, String)> {
    if matches!(value, Value::Undefined) {
        return Ok(());
    }
    let valid = match (name, value) {
        ("colorSpace", Value::String(s)) => matches!(
            s.as_str(),
            "srgb" | "srgb-linear" | "display-p3" | "display-p3-linear"
        ),
        ("pixelFormat", Value::String(s)) => matches!(s.as_str(), "rgba-unorm8" | "rgba-float16"),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(("TypeError", format!("Unknown canvas readback {name}.")))
    }
}

impl Runtime {
    pub(super) fn canvas_image_data(
        &mut self,
        object: u32,
        args: &[Value],
        settings: Option<&[(&str, Value)]>,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if !matches!(
            args.get(4),
            None | Some(
                Value::Null
                    | Value::Undefined
                    | Value::Object(_)
                    | Value::Host(_)
                    | Value::Function(_)
            )
        ) {
            return Err(Fault::exception(
                "TypeError",
                "ImageDataSettings requires a dictionary.",
            ));
        }
        // Conversion can invoke page code through valueOf or dictionary getters.
        // Recheck access before taking the receiver's current pixel backing.
        self.validate_native_access(object, host)?;
        let [Value::Number(x), Value::Number(y), Value::Number(w), Value::Number(h), ..] = args
        else {
            return Err(Fault::new("Missing converted canvas readback coordinates."));
        };
        let rect = match ReadbackRect::new(
            *x as i32,
            *y as i32,
            *w as i32,
            *h as i32,
            self.binary_storage_available(),
        ) {
            Ok(rect) => rect,
            Err(ReadbackError::ZeroSize) => {
                let ex = self.native_dom_exception(
                    "IndexSizeError",
                    "Canvas readback requires nonzero width and height.",
                )?;
                return Err(Fault::thrown(ex));
            }
            Err(_) => {
                return Err(Fault::new(
                    "Canvas readback exceeds the typed-array backing limit.",
                ))
            }
        };
        for (name, value) in settings.unwrap_or_default() {
            if let Value::String(value) = value {
                if !matches!(
                    (*name, value.as_str()),
                    ("colorSpace", "srgb") | ("pixelFormat", "rgba-unorm8")
                ) {
                    let ex = self.native_dom_exception(
                        "NotSupportedError",
                        "The canvas reader supports sRGB RGBA8 output.",
                    )?;
                    return Err(Fault::thrown(ex));
                }
            }
        }
        // The Page exposes only rectangles drawn by this checked context.
        // VM allocation supplies a new buffer and the source is only borrowed.
        let surface = host
            .canvas_readback_source(object, rect.byte_len())
            .map_err(Fault::new)?;
        self.new_rgba_image_data(rect.width, rect.height, |bytes| {
            surface
                .copy_rgba(rect, bytes)
                .map_err(|_| "Invalid canvas readback destination.".to_owned())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_2d_bitmap::CanvasBitmap;

    struct PixelHost {
        canvas: CanvasBitmap,
        reads: usize,
        expected_bytes: usize,
    }

    impl Host for PixelHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            panic!("unexpected get")
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            panic!("unexpected set")
        }
        fn call(&mut self, _: u32, method: &str, args: &[Value]) -> Result<Value, String> {
            assert_eq!(method, "retain");
            assert!(matches!(args, [Value::Object(_), Value::Function(_)]));
            Ok(Value::Undefined)
        }
        fn dictionary_argument(
            &self,
            _: u32,
            method: &str,
            _: &[Value],
        ) -> Option<(usize, &'static [&'static str])> {
            (method == METHOD).then_some((4, &["colorSpace", "pixelFormat"]))
        }
        fn dictionary_member_conversion(&self, _: u32, method: &str, _: &str) -> Option<bool> {
            (method == METHOD).then_some(true)
        }
        fn validate_dictionary_member(
            &self,
            _: u32,
            _: &str,
            name: &str,
            value: &Value,
        ) -> Result<(), (&'static str, String)> {
            validate_setting(name, value)
        }
        fn canvas_readback_source(
            &mut self,
            _: u32,
            bytes: usize,
        ) -> Result<&CanvasBitmap, String> {
            assert_eq!(bytes, self.expected_bytes);
            self.reads += 1;
            Ok(&self.canvas)
        }
    }

    #[test]
    fn canvas_readback_settings_and_ignored_arguments_remain_transient() {
        let mut runtime = Runtime::for_test();
        let mut host = PixelHost {
            canvas: CanvasBitmap::new(1, 1),
            reads: 0,
            expected_bytes: 4,
        };
        runtime
            .define_host_function("readPixels", 1, METHOD)
            .unwrap();
        runtime.define_host_function("retain", 1, "retain").unwrap();
        let roots = |runtime: &Runtime| {
            (
                runtime.exported_object_roots.len(),
                runtime.exported_function_roots.len(),
            )
        };
        let before = roots(&runtime);
        runtime.execute(r#"
var visits=0;
function options(space){return {
    get colorSpace(){visits++;return {toString:function(){return space;}};},
    get pixelFormat(){visits++;return 'rgba-unorm8';}
};}
if(readPixels(0,0,1,1,options('srgb'),function(){}).data[3]!==0)throw 'valid readback';
var caught=false;try{readPixels(0,0,1,1,options('invalid'),function(){});}catch(e){caught=e instanceof TypeError;}
if(!caught)throw 'invalid enum';
caught=false;try{readPixels(0,0,0,1,options('srgb'),function(){});}catch(e){caught=e instanceof DOMException&&e.name==='IndexSizeError';}
if(!caught||visits!==5)throw 'ordered transient getters';
0;
"#, &mut host).unwrap();
        assert_eq!(host.reads, 1);
        assert_eq!(roots(&runtime), before);
        assert!(runtime.transient_registration_is_empty());
        runtime
            .execute("retain({},function(){});0;", &mut host)
            .unwrap();
        assert_eq!(roots(&runtime), (before.0 + 1, before.1 + 1));
    }

    #[test]
    fn canvas_readback_uses_shared_storage_beyond_four_mib() {
        let mut runtime = Runtime::new();
        let mut host = PixelHost {
            canvas: CanvasBitmap::new(1, 1), reads: 0, expected_bytes: 1025 * 1024 * 4,
        };
        runtime.define_host_function("readPixels", 1, METHOD).unwrap();
        runtime.execute("var image=readPixels(0,0,1025,1024);if(image.width!==1025||image.height!==1024||image.data.length!==4198400||image.data[4198399]!==0)throw 'large readback';", &mut host).unwrap();
        assert_eq!(host.reads, 1);
        runtime.limits.heap.retained_values = runtime.value_bytes + 64;
        let error = runtime.execute("readPixels(0,0,1025,1024);", &mut host).unwrap_err();
        assert!(error.contains("backing limit"), "{error}");
        assert_eq!(host.reads, 1);
        assert!(runtime.transient_registration_is_empty());
    }
}
