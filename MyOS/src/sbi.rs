
macro_rules! sbicall {
    ($id:expr, $a0:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => ({
            let arg0 = {$a0};
            let arg1 = {$a1};
            let arg2 = {$a2};
            let arg3 = {$a3};
            let arg4 = {$a4};
            let arg5 = {$a5};
            let (extension_id, function_id) = $id;
            let error: i64;
            let value: i64;
            unsafe {
                core::arch::asm!("ecall",
                    in("a0") arg0,
                    in("a1") arg1,
                    in("a2") arg2,
                    in("a3") arg3,
                    in("a4") arg4,
                    in("a5") arg5,
                    in("a7") extension_id,
                    in("a6") function_id,
                    lateout("a0") error,
                    lateout("a1") value
                )
            }
        (error, value)
        }
    );
    ($id:expr $(, $args:expr)*) => (
        sbicall!($id $(, $args)*, 0)
    );
}
pub fn debug_write_char(c: u8) {
    let extid = 0x4442_434E;
    let funcid = 2;
    sbicall!((extid, funcid), c);
}

/// SBI error values for the error field (a0) in the SBI return structure
pub enum SbiError {
    Success = 0,
    Failed,
    NotSupported,
    InvalidParam,
    Denied,
    InvalidAddress,
    AlreadyAvailable,
    AlreadyStarted,
    AlreadyStopped,
    NoShMem,
    InvalidState,
    BadRange,
    Timeout,
    Io,
    DeniedLocked,
}

/// SbiERROR methods. handle various Sbi errors
/// this module is hooked to the From<i64> trait due to the sbicall! macro return value
impl From<i64> for SbiError {
    fn from(value: i64) -> Self {
        match value.abs() {
            0 => SbiError::Success,
            2 => SbiError::NotSupported,
            3 => SbiError::InvalidParam,
            4 => SbiError::Denied,
            5 => SbiError::InvalidAddress,
            6 => SbiError::AlreadyAvailable,
            7 => SbiError::AlreadyStarted,
            8 => SbiError::AlreadyStopped,
            9 => SbiError::NoShMem,
            10 => SbiError::InvalidState,
            11 => SbiError::BadRange,
            12 => SbiError::Timeout,
            13 => SbiError::Io,
            14 => SbiError::DeniedLocked,
            // Match statements must be exhaustive, so to exhaust i64, we match everything
            // below to the generic "Failed".
            _ => SbiError::Failed,
        }
    }
}
