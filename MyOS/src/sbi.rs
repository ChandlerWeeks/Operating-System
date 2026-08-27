
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
                    in("a0") arg0, // result register
                    in("a1") arg1, // return value for the result register
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

impl SbiError {
    // Is the SBI error real?
    pub fn is_err(&self) -> bool {
        match self {
            &SbiError::Success => false,
            _ => true,
        }
    }

    // Is this an SBI success?
    pub fn is_ok(&self) -> bool {
        match self {
            &SbiError::Success => true,
            _ => false,
        }
    }
}

impl core::fmt::Display for SbiError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

pub enum SbiResult<T> {
    Ok(T),
    Err(SbiError),
}
impl <T> SbiResult<T> {
    pub fn is_ok(&self) -> bool {
        match self {
            &SbiResult::Ok(_) => true,
            _ => false,
        }
    }

    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    pub fn unwrap(self) -> T {
        match self {
            SbiResult::Ok(x) => x,
            _ => {
                panic!("Called unwrap on err!");
            }
        }
    }

    pub fn unwrap_or(self, orelse: T) -> T {
        match self {
            SbiResult::Ok(x) => x,
            _ => orelse,
        }
    }

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

pub fn get_spec_version() -> (u32, u32) {
    let (error, result) = sbicall!((0x10, 0));
    debug_assert!(error.is_err(), "get_spec_version returned error!");

    let major = (result >> 24) & 0x7F;
    let minor = result & 0xFF_FFFF;
    (major as u32, minor as u32)
}

#[repr(C)]
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

pub fn hart_start(hart_id: u64, physical_start_address: usize, wake_frame_address: usize) -> (i64, i64) {
    let extension_id =  0x48534d;
    let function_id = 0;
    sbicall!((extension_id, function_id), hart_id, physical_start_address, wake_frame_address)
}

pub fn hart_stop() -> ! {
    let extension_id =  0x48534d;
    let function_id = 1;
    sbicall!((extension_id, function_id));
    panic!("hart suspended uncessfully");
}

pub fn hart_status(hart_id: usize) -> (i64, i64) {
    let extension_id =  0x48534d;
    let function_id = 2;
    sbicall!((extension_id, function_id), hart_id)
}

pub fn hart_suspend(suspend_type: usize) {
    let extension_id =  0x48534d;
    let function_id = 3;
    sbicall!((extension_id, function_id), suspend_type);
    panic!("hart suspended uncessfully");
}

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

pub fn reboot() -> ! {
    sbicall!((0x53525354, 0), 2, 0);
    debugln!("ERROR: REBOOT RETURNED!");
    hart_stop();
}

pub fn poweroff() -> ! {
    sbicall!((0x53525354, 0), 0, 0);
    debugln!("ERROR: REBOOT RETURNED!");
    hart_stop();
}

