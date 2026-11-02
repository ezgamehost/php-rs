// Altered Rust adaptation of PHP 8.5.0 ext/random/random.c and csprng.c.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn random_bytes(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let length = self.native_int(args[0].as_ref().unwrap(), "random_bytes", 1, line)?;
        if length < 1 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "random_bytes(): Argument #1 ($length) must be greater than 0",
                line,
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| Error::new(ErrorKind::Error, "String size overflow", line))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        bytes.resize(length, 0);
        fill(&mut bytes)
            .map_err(|message| Error::new(ErrorKind::RandomException, message, line))?;
        Ok(Value::String(bytes.into()))
    }

    pub(super) fn random_int(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let min = self.native_int(args[0].as_ref().unwrap(), "random_int", 1, line)?;
        let max = self.native_int(args[1].as_ref().unwrap(), "random_int", 2, line)?;
        if min > max {
            return Err(Error::new(
                ErrorKind::ValueError,
                "random_int(): Argument #1 ($min) must be less than or equal to argument #2 ($max)",
                line,
            ));
        }
        if min == max {
            return Ok(Value::Int(min));
        }
        let range = (max as u64).wrapping_sub(min as u64).wrapping_add(1);
        loop {
            let mut bytes = [0; 8];
            fill(&mut bytes)
                .map_err(|message| Error::new(ErrorKind::RandomException, message, line))?;
            let trial = u64::from_ne_bytes(bytes);
            if range == 0 {
                return Ok(Value::Int(trial as i64));
            }
            // Reject the incomplete remainder at the top of the unsigned range.
            // Power-of-two ranges divide the entire 64-bit sample space exactly.
            if !range.is_power_of_two() && trial > u64::MAX - (u64::MAX % range) - 1 {
                continue;
            }
            return Ok(Value::Int((trial % range).wrapping_add(min as u64) as i64));
        }
    }
}

#[cfg(target_os = "macos")]
fn fill(bytes: &mut [u8]) -> Result<(), &'static str> {
    unsafe extern "C" {
        fn CCRandomGenerateBytes(bytes: *mut std::ffi::c_void, count: usize) -> i32;
    }
    // SAFETY: the exclusive slice remains live and writable for its full length.
    let status = unsafe { CCRandomGenerateBytes(bytes.as_mut_ptr().cast(), bytes.len()) };
    if status == 0 {
        Ok(())
    } else {
        Err("Failed to retrieve randomness from the operating system (CCRandomGenerateBytes)")
    }
}

#[cfg(target_os = "linux")]
fn fill(mut bytes: &mut [u8]) -> Result<(), &'static str> {
    unsafe extern "C" {
        fn getrandom(bytes: *mut std::ffi::c_void, count: usize, flags: u32) -> isize;
    }
    while !bytes.is_empty() {
        // SAFETY: the exclusive slice remains live and writable. Flags 0 waits
        // for kernel entropy initialization; short reads retain the remainder.
        let count = unsafe { getrandom(bytes.as_mut_ptr().cast(), bytes.len(), 0) };
        if count > 0 {
            bytes = &mut bytes[count as usize..];
        } else if count < 0
            && matches!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            )
        {
            continue;
        } else {
            return Err("Failed to retrieve randomness from the operating system (getrandom)");
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn fill(_: &mut [u8]) -> Result<(), &'static str> {
    Err("No supported operating system random source")
}
