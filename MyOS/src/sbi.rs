//! SBI Call Implementation and Use 

/// SBI call: runs the RISC-V SBI, which is handled by SBI firmware. 
/// Inputs:
///
/// id: a tuple containing the function id (a6) and extension id (a7):
/// a{2..=5} optional arguement registers for different SBI calls. 
/// a0 represents the SBI error result
/// a1 represents the SBI value result
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
            // ecall is unsafe
            unsafe {
                // executes the ecall instruction, which requests elevated permissions 
                core::arch::asm!("ecall",
                    in("a0") arg0, // SBI Error Register
                    in("a1") arg1, // SBI Result Register
                    in("a2") arg2,
                    in("a3") arg3,
                    in("a4") arg4,
                    in("a5") arg5,
                    in("a7") extension_id, // extention
                    in("a6") function_id, // function_id
                    lateout("a0") error,
                    lateout("a1") value
                )
            }
        (error, value)
        }
    );
    // Allows none or some arguements, allowing the SBIcall to work with 0 to 6 arguements. 
    ($id:expr $(, $args:expr)*) => (
        sbicall!($id $(, $args)*, 0)
    );
}

/// Uses an SBI call to write a byte to the kernel output. 
///
/// Inputs:
///
/// - u8: a byte to go to the kernel debug console
pub fn debug_write_char(c: u8) {
    let extid = 0x4442_434E;
    let funcid = 2;
    sbicall!((extid, funcid), c);
}

/// Enumeration to abstract the SBI error code.
#[derive(Debug, Clone)]
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

/// SbiError methods. Abstract various SBI errors
impl From<i64> for SbiError {
    /// Convert the i64 representation of a SbiError abstraction. 
    ///
    /// Inputs:
    ///
    ///     - i64: an integer representation of the SbiError
    ///
    /// Returns:
    ///
    ///     - SbiError: an abstraction value for the SbiError field. 
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

/// Convert an SbiError back into an integer as an i64
impl From<SbiError> for i64 {
    fn from(value:SbiError) -> Self {
        -(value as i64)
    }
}

impl SbiError {
    /// Returns wheter there is an Sbi Error or not
    pub fn is_err(&self) -> bool {
        match self {
            &SbiError::Success => false,
            _ => true,
        }
    }

    /// Returns whether the SbiCall was successful
    pub fn is_ok(&self) -> bool {
        match self {
            &SbiError::Success => true,
            _ => false,
        }
    }
}

impl core::fmt::Display for SbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Formats an SbiError abstraction into a string representation 
        //
        // Inputs:
        //
        //     f - The output destination of the string
        //
        // Return:
        //
        //     Result - Representation of whether the write was successful, or returned an error. 
        let s = match self {
            &SbiError::Success => "success",
            &SbiError::Failed => "failed",
            &SbiError::NotSupported => "not supported",
            &SbiError::InvalidParam => "invalid parameter",
            &SbiError::Denied => "denied",
            &SbiError::InvalidAddress => "invalid address",
            &SbiError::AlreadyAvailable => "already available",
            &SbiError::AlreadyStarted => "already started",
            &SbiError::AlreadyStopped => "already stopped",
            &SbiError::NoShMem => "no shared memory",
            &SbiError::InvalidState => "invalid state",
            &SbiError::BadRange => "bad range",
            &SbiError::Timeout => "timeout",
            &SbiError::Io => "IO error",
            &SbiError::DeniedLocked => "denied locked",
        };
        write!(f, "{}", s)
    }
}

/// SbiResult result. Either returns the error, or signals no error.
///
/// Inputs:
///
///     - T: A generic representing what succeeded
#[derive(Clone, Debug)]
pub enum SbiResult<T> {
    Ok(T),
    Err(SbiError),
}
/// implementation of the SbiResult enum
impl <T> SbiResult<T> {
    /// returns weather the SbiResult is Ok()
    pub fn is_ok(&self) -> bool {
        match self {
            &SbiResult::Ok(_) => true,
            _ => false,
        }
    }
    /// returns whether the SbiResult has any errors.
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// Extract the result from an SBI success using the enum. Panic on failure.
    pub fn unwrap(self) -> T {
        match self {
            SbiResult::Ok(x) => x,
            _ => {
                panic!("Called unwrap on err!");
            }
        }
    }

    /// Extract the result from an Sbi success using the enum. Also handles extracting errors
    pub fn unwrap_or(self, orelse: T) -> T {
        match self {
            SbiResult::Ok(x) => x,
            _ => orelse,
        }
    }

    /// returns a SBI success value, or calls a function to generate a replacement. 
    pub fn unwrap_or_else<F>(self, orelse: F) -> T
        where
            F: FnOnce(SbiError) -> T,
    {
        match self {
            SbiResult::Ok(x) => x,
            SbiResult::Err(e) => orelse(e),
        }
    }
}

/// Get Specification versions
///
/// Returns the major and minor version numbers.
pub fn get_spec_version() -> (u32, u32) {
    let (raw_error, result) = sbicall!((0x10, 0));
    let error = SbiError::from(raw_error);
    debug_assert!(error.is_ok(), "get_spec_version returned error!");

    let major = (result >> 24) & 0x7F;
    let minor = result & 0xFF_FFFF;
    (major as u32, minor as u32)
}

/// Stores planned startup values for a secondary hart.
///
/// A caller can pass the address of this frame as the opaque value to `hart_start`. SBI passes the opaque value to the target hart in register `a1`.
///
/// Future secondary-hart startup code can read this frame and configure the hart before it enters normal kernel code.
///
/// This type uses C field layout so startup code can access its fields at fixed offsets.
///
/// The current source tree does not yet read this frame.
#[repr(C)] // forces the compiler to layour struct fields using C ABI ordering
#[derive(Debug, Copy, Clone, Default)]
pub struct WakeFrame {
    pub sepc: usize,     // 0
    pub sstatus: usize,  // 8
    pub sie: usize,      // 16
    pub satp: usize,     // 24
    pub sscratch: usize, // 32
    pub stvec: usize,    // 40
    pub stack: usize,    // 48
    pub page: usize,     // 56 (the WakeFrame's page address)
}

/// Start a hart using an SBI call.
///
/// Inputs:
///
///     - hart_id: The HART ID to start
///     - physical_start_address: the phsyical memory address where the hart will begin execution in S-mode
///     - wake_frame_address: A starting address for the wake_frame
pub fn hart_start(hart_id: u64, physical_start_address: usize, wake_frame_address: usize) -> SbiResult<()> {
    let extension_id =  0x48534d;
    let function_id = 0;
    let (err, _) = sbicall!((extension_id, function_id), hart_id, physical_start_address, wake_frame_address);
    let error = SbiError::from(err);

    if error.is_err() {
        SbiResult::Err(error)
    } else {
        SbiResult::Ok(())
    }
}

/// Stop a hart using an SBI call. 
pub fn hart_stop() -> ! {
    let extension_id =  0x48534d;
    let function_id = 1;
    sbicall!((extension_id, function_id));
    panic!("Continued running after stopping a hart");
}

/// Check the specification of a hart. 
///
/// Inputs:
///
///     - hart_id: The hart ID to check the status of
///
/// Returns:
///
///     - HartState -> Current State of the Hart
pub fn hart_status(hart_id: usize) -> SbiResult<HartState> {
    let extension_id =  0x48534d;
    let function_id = 2;
    let (err, state) = sbicall!((extension_id, function_id), hart_id);
    let error = SbiError::from(err);

    if error.is_err() {
        return SbiResult::Err(error);
    }
    HartState::from_u64(state as u64)
}

/// Makes an SBI call to stop a hart.
/// Generally used to enter a low power state while remaining responsive. 
///
/// Inputs:
///
///     - suspend_type: The type of suspension that will occur within the hart
pub fn hart_suspend(suspend_type: u32, resume_addr: usize, opaque: usize) -> SbiResult<()> {
    let extension_id =  0x48534d;
    let function_id = 3;
    let (err, _) = sbicall!((extension_id, function_id), suspend_type as usize, resume_addr, opaque);
    let error = SbiError::from(err);

    if error.is_err() {
        SbiResult::Err(error)
    } else {
        SbiResult::Ok(())
    }
}

/// Enumerator to represent the state of a hart
#[repr(i64)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum HartState {
    Started = 0,
    Stopped = 1,
    StartPending = 2,
    StopPending = 3,
    Suspended = 4,
    SuspendPending = 5,
    ResumePending = 6,
}


impl HartState {
    /// Converts a u64 into a HartState, which is turned into a SbiResult. 
    #[allow(dead_code)]
    fn from_u64(value: u64) -> SbiResult<HartState> {
        match value {
            0 => SbiResult::Ok(HartState::Started),
            1 => SbiResult::Ok(HartState::Stopped),
            2 => SbiResult::Ok(HartState::StartPending),
            3 => SbiResult::Ok(HartState::StopPending),
            4 => SbiResult::Ok(HartState::Suspended),
            5 => SbiResult::Ok(HartState::SuspendPending),
            6 => SbiResult::Ok(HartState::ResumePending),
            _ => SbiResult::Err(SbiError::InvalidState),
        }
    }
}

/// reboot the system using an SBI call (function id 2)
pub fn reboot() -> ! {
    sbicall!((0x53525354, 0), 2, 0);
    crate::debugln!("ERROR: REBOOT RETURNED!");
    hart_stop();
}

/// power the system off using an SBI call (function id 0)
pub fn shutdown() -> ! {
    sbicall!((0x53525354, 0), 0, 0);
    crate::debugln!("ERROR: REBOOT RETURNED!");
    hart_stop();
}
