//! Fresh operating-system entropy. No clock/PID/hash fallback is permitted.
//! The OS supplies random bytes; our code implements the TLS cryptography.

#[cfg(target_os = "windows")]
pub fn fill(bytes: &mut [u8]) -> Result<(), String> {
    use std::ffi::c_void;
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(algorithm: *mut c_void, buffer: *mut u8, length: u32, flags: u32)
            -> i32;
    }
    let length = u32::try_from(bytes.len()).map_err(|_| "Random-byte request is too large.")?;
    if bytes.is_empty() {
        return Ok(());
    }
    // BCRYPT_USE_SYSTEM_PREFERRED_RNG with a null algorithm handle.
    // The supplied slice owns length writable bytes for the entire native call.
    let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), bytes.as_mut_ptr(), length, 2) };
    if status < 0 {
        bytes.fill(0);
        return Err(format!(
            "The Windows random source failed (NTSTATUS {status:#010x})."
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub fn fill(bytes: &mut [u8]) -> Result<(), String> {
    use std::io::Read;
    if bytes.is_empty() {
        return Ok(());
    }
    let result = std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(bytes));
    if let Err(error) = result {
        bytes.fill(0);
        return Err(format!(
            "The operating-system random source failed: {error}"
        ));
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", unix)))]
pub fn fill(_: &mut [u8]) -> Result<(), String> {
    Err("No secure random source is implemented for this platform.".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn operating_system_supplies_bytes_and_accepts_empty_requests() {
        super::fill(&mut []).unwrap();
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        super::fill(&mut first).unwrap();
        super::fill(&mut second).unwrap();
        assert_ne!(first, [0; 64]);
        assert_ne!(first, second);
    }
}
