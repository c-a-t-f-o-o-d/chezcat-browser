//! Isolated original-codec tests, without the ChezCat Runtime or application.
//!
//! rustc --edition=2021 --test tools/multibyte_decoder_probe.rs -o <probe-path>
//! This checks the allocation-free decoder only. It neither runs nor replaces
//! the application, Cargo acceptance check, or public TextDecoder API tests.

#[path = "../src/text_encoding_multibyte.rs"]
mod multibyte;

#[path = "../src/text_encoding_iso2022jp.rs"]
mod iso2022jp;
