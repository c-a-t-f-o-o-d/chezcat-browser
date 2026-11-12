//! Original, bounded HTML atob/btoa implementations.
//! References: https://html.spec.whatwg.org/multipage/webappapis.html#atob
//! and https://infra.spec.whatwg.org/#forgiving-base64-decode.

use super::{Fault, Host, Runtime, Value, MAX_STRING_BYTES};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn invalid_character() -> Fault {
    Fault::exception(
        "InvalidCharacterError",
        "Invalid character in base64 conversion.",
    )
}

impl Runtime {
    pub(super) fn base64_builtin(
        &mut self,
        encode: bool,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        let value = args.first().ok_or_else(|| {
            Fault::exception("TypeError", "Base64 conversion requires one argument.")
        })?;
        let value = self.copy_value(value)?;
        let value = self.primitive_with_hint(value, true, host)?;
        if value.bytes() > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.work(value.bytes())?;
        let text = value.script_text()?;
        let output = if encode {
            self.base64_encode(&text)?
        } else {
            self.base64_decode(&text)?
        };
        self.checked_value(Value::String(output))
    }

    fn base64_encode(&mut self, text: &str) -> Result<String, Fault> {
        self.work(text.len())?;
        let mut bytes = Vec::with_capacity(text.len());
        for character in text.chars() {
            self.step()?;
            if u32::from(character) > 255 {
                return Err(invalid_character());
            }
            bytes.push(character as u8);
        }
        let length = bytes.len().div_ceil(3) * 4;
        if length > MAX_STRING_BYTES {
            return Err(Fault::new("String limit reached (128 KiB)."));
        }
        self.work(length)?;
        let mut output = String::with_capacity(length);
        for chunk in bytes.chunks(3) {
            self.step()?;
            let bits = (usize::from(chunk[0]) << 16)
                | (usize::from(*chunk.get(1).unwrap_or(&0)) << 8)
                | usize::from(*chunk.get(2).unwrap_or(&0));
            output.push(char::from(ALPHABET[(bits >> 18) & 63]));
            output.push(char::from(ALPHABET[(bits >> 12) & 63]));
            output.push(if chunk.len() > 1 {
                char::from(ALPHABET[(bits >> 6) & 63])
            } else {
                '='
            });
            output.push(if chunk.len() > 2 {
                char::from(ALPHABET[bits & 63])
            } else {
                '='
            });
        }
        Ok(output)
    }

    fn base64_decode(&mut self, text: &str) -> Result<String, Fault> {
        self.work(text.len())?;
        let mut data = Vec::with_capacity(text.len());
        for byte in text.bytes() {
            self.step()?;
            if !matches!(byte, b'\t' | b'\n' | b'\x0c' | b'\r' | b' ') {
                data.push(byte);
            }
        }
        if data.len().is_multiple_of(4) {
            for _ in 0..2 {
                if data.last() == Some(&b'=') {
                    data.pop();
                }
            }
        }
        if data.len() % 4 == 1 {
            return Err(invalid_character());
        }
        let (mut buffer, mut bits) = (0u16, 0u32);
        let mut output = String::new();
        for chunk in data.chunks(4) {
            self.step()?;
            for byte in chunk {
                let value = match byte {
                    b'A'..=b'Z' => byte - b'A',
                    b'a'..=b'z' => byte - b'a' + 26,
                    b'0'..=b'9' => byte - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    _ => return Err(invalid_character()),
                };
                buffer = (buffer << 6) | u16::from(value);
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    let character = char::from((buffer >> bits) as u8);
                    self.append_string(&mut output, character.encode_utf8(&mut [0; 4]))?;
                    buffer &= (1 << bits) - 1;
                }
            }
        }
        // Forgiving decode discards the unused low bits, even if nonzero.
        Ok(output)
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

    fn convert(encode: bool, text: &str) -> Result<Value, Fault> {
        Runtime::for_test().base64_builtin(encode, &[Value::String(text.into())], &mut NoHost)
    }

    #[test]
    fn base64_original_vectors_binary_roundtrip_and_forgiving_padding() {
        for (binary, encoded) in [
            ("", ""),
            ("C", "Qw=="),
            ("Ca", "Q2E="),
            ("Cat", "Q2F0"),
            ("ChezCat", "Q2hlekNhdA=="),
            ("\0\u{80}\u{ff}", "AID/"),
        ] {
            assert_eq!(convert(true, binary).unwrap().to_text(), encoded);
            assert_eq!(convert(false, encoded).unwrap().to_text(), binary);
        }
        for (encoded, expected) in [
            (" \t\n\r\u{c}", ""),
            (" Q 2\tF\n0\r\u{c}", "Cat"),
            ("Qw", "C"),
            ("Qx", "C"),
            ("Qx==", "C"),
            ("Q2F", "Ca"),
            ("Q2F=", "Ca"),
        ] {
            assert_eq!(convert(false, encoded).unwrap().to_text(), expected);
        }
        let binary: String = (0..=255u8).map(char::from).collect();
        let encoded = convert(true, &binary).unwrap().to_text();
        assert_eq!(convert(false, &encoded).unwrap().to_text(), binary);
    }

    #[test]
    fn base64_rejects_invalid_alphabet_padding_and_non_latin1() {
        for text in [
            "A",
            "A=",
            "A==",
            "A===",
            "====",
            "Qw=",
            "Qw===",
            "Q=w=",
            "Qw==Qw==",
            "Qw==!",
            "Qw-",
            "Qw_",
            "Qw\u{b}",
            "Qw\u{a0}",
            "Qw\u{100}",
        ] {
            let fault = convert(false, text).unwrap_err();
            assert!(
                matches!(
                    fault.exception.as_deref(),
                    Some(super::super::Exception::Error("InvalidCharacterError"))
                ),
                "{text:?}"
            );
        }
        for text in ["\u{100}", "emoji: \u{1f431}"] {
            let fault = convert(true, text).unwrap_err();
            assert!(matches!(
                fault.exception.as_deref(),
                Some(super::super::Exception::Error("InvalidCharacterError"))
            ));
        }
    }

    #[test]
    fn base64_required_argument_and_string_coercion_order() {
        let answer = Runtime::for_test().execute(r#"
var seen='', errors=[];
var value={toString:function(){seen+='s';return '\xff';},valueOf:function(){seen+='v';return 'wrong';}};
var encoded=btoa(value,{toString:function(){seen+='x';return 'ignored';}});
var marker={};
try{atob({toString:function(){seen+='t';throw marker;}});}catch(e){errors.push(e===marker);}
try{btoa({toString:function(){seen+='y';return Symbol('x');},valueOf:function(){seen+='z';return 'wrong';}});}catch(e){errors.push(e.name);}
try{atob();}catch(e){errors.push(e.name);}
try{btoa();}catch(e){errors.push(e.name);}
try{atob(Symbol('x'));}catch(e){errors.push(e.name);}
try{atob(undefined);}catch(e){errors.push(e.name);}
[encoded,seen,errors.join(','),btoa(undefined),atob('Qw')].join('|');
"#, &mut NoHost).unwrap();
        assert_eq!(answer.to_text(), "/w==|sty|true,TypeError,TypeError,TypeError,TypeError,InvalidCharacterError|dW5kZWZpbmVk|C");
    }

    #[test]
    fn base64_preserves_utf8_string_and_execution_work_bounds() {
        let max_ascii = "x".repeat(MAX_STRING_BYTES / 4 * 3);
        assert_eq!(
            convert(true, &max_ascii).unwrap().to_text().len(),
            MAX_STRING_BYTES
        );
        let too_large = max_ascii + "x";
        let fault = convert(true, &too_large).unwrap_err();
        assert!(fault.exception.is_none());
        assert!(fault.message.contains("String limit"));
        let max_latin1 = "\u{ff}".repeat(MAX_STRING_BYTES / 2);
        let encoded = convert(true, &max_latin1).unwrap().to_text();
        assert_eq!(convert(false, &encoded).unwrap().to_text(), max_latin1);
        let fault = convert(false, &"/".repeat(MAX_STRING_BYTES)).unwrap_err();
        assert!(fault.exception.is_none());
        assert!(fault.message.contains("String limit"));
        let mut runtime = Runtime::for_test();
        runtime.copy_bytes = 0;
        let fault = runtime
            .base64_builtin(false, &[Value::String("Qw".into())], &mut NoHost)
            .unwrap_err();
        assert!(fault.exception.is_none());
        assert!(fault.message.contains("String/copy work limit"));
        runtime.begin_job();
        runtime.steps = 0;
        let fault = runtime
            .base64_builtin(true, &[Value::String("Cat".into())], &mut NoHost)
            .unwrap_err();
        assert!(fault.exception.is_none());
        assert!(fault.message.contains("Execution step limit"));
    }
}
