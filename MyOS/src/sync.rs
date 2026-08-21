//! # Mutex locking and unlocking functions.
//!
//! © Stephen Marz
//! 1 July 2026
use core::{
    arch::asm,
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::read_volatile,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

// ==============================================================================
// IRQ Routines for RISC-V 64-bit
// ==============================================================================

/// Saves the current state of the SIE bit in sstatus and disables interrupts by clearing the SIE bit.
/// Returns the previous state of sstatus, which can be used to restore the interrupt state later.
///
/// # Safety
///
/// This should NOT be called directly. Instead, it should be used with the CriticalSection structure.
///
/// This function should only be called with a valid `saved_state` obtained from a previous call to `irq_save_and_disable()`.
/// It should be called in the same context (S-mode).
///
/// # Examples
///
/// ```rust
/// let saved_state = irq_save_and_disable();
/// // critical section
/// irq_restore(saved_state);
/// ```
#[inline(always)]
pub fn irq_save_and_disable() -> usize {
    let sstatus: usize;
    // SAFETY:
    //   csrrci atomically reads sstatus and clears SIE (bit index 1)
    //   in a single instruction — no race between read and clear.
    //   Valid in S-mode (supervisor) kernel context only.
    unsafe {
        asm!(
            // csrrci rd, csr, imm:
            //   rd = sstatus           (save full register)
            //   sstatus.SIE = 0        (clear bit 1 atomically)
            "csrrci {saved}, sstatus, 0b10",
            saved = out(reg) sstatus,
            options(nomem, nostack),
        );
    }
    sstatus
}

/// Restores the SIE bit in sstatus to its previous state, as saved in `saved_state`. This should NOT be called
/// directly. Instead, it should be used with the CriticalSection structure.
///
/// # Safety
///
/// This function should only be called with a valid `saved_state` obtained from a previous call to `irq_save_and_disable()`.
/// It should be called in the same context (S-mode).
///
/// # Examples
///
/// ```rust
/// let saved_state = irq_save_and_disable();
/// // critical section
/// irq_restore(saved_state);
/// ```
#[inline(always)]
pub fn irq_restore(saved_state: usize) {
    // SAFETY:
    //   Writes the full saved sstatus value back. This restores SIE
    //   to its pre-lock state. Writing sstatus is valid in S-mode.
    unsafe {
        asm!(
            "csrw sstatus, {saved}",
            saved = in(reg) saved_state,
            options(nomem, nostack),
        );
    }
}

/// # Mutex with IRQ-Save Critical Section
///
/// Implements the `spin_lock_irqsave` / `spin_lock_irqrestore` pattern from
/// Linux, expressed through Rust's `Drop` trait rather than explicit function
/// pairs.
///
/// ## Drop Ordering (Critical)
///
/// `MutexGuard` contains two resources that must be released in a specific order:
///
/// ```text
/// MutexGuard::drop() impl  ->  releases the spinlock       (step 1)
/// _cs field drop           ->  restores saved IRQ state    (step 2)
/// ```
///
/// Rust guarantees: the explicit `Drop` impl runs before field drops, and
/// fields drop in declaration order. Reversing this order (restoring IRQs
/// before releasing the spinlock) would allow an interrupt handler to race
/// to acquire the same lock — a guaranteed deadlock on a single-core system.
///
/// ## Architecture Support
///
/// | Architecture | Save mechanism           | Disable       | Restore           |
/// |--------------|--------------------------|---------------|-------------------|
/// | x86-64       | `pushfq` -> save RFLAGS  | `cli`         | `push` + `popfq`  |
/// | RISC-V 64    | `csrrci sstatus, 2`      | clears SIE    | `csrw sstatus`    |
///
/// On x86-64, the full RFLAGS register is saved and restored, so any other
/// flags that were set (DF, AC, etc.) are also preserved — not just IF.
///
/// On RISC-V, `csrrci` atomically reads `sstatus` and clears the SIE bit
/// (bit 1) in a single instruction. Restoring the full saved `sstatus` value
/// brings SIE back to exactly what it was before the lock.

// ==============================================================================
// CriticalSection — IRQ-save / IRQ-restore
// ==============================================================================

/// Saved interrupt state for the current CPU.
///
/// Construction saves and disables interrupts (`spin_lock_irqsave`).
/// Drop restores exactly the interrupt state that was saved (`spin_lock_irqrestore`).
///
/// Nesting is safe: if interrupts were already disabled when `CriticalSection::new()`
/// is called, the saved state records them as disabled and `drop()` leaves them
/// disabled — a nested critical section never accidentally re-enables interrupts
/// on exit.
#[derive(Debug)]
pub struct CriticalSection {
    /// Saved interrupt state. Interpreted as RFLAGS on x86-64 or sstatus on RISC-V.
    saved_state: usize,
    no_send_marker: core::marker::PhantomData<*const ()>, // !Send, !Sync
}

impl CriticalSection {
    /// Save the current interrupt state and disable interrupts.
    ///
    /// Equivalent to `local_irq_save(flags)` in Linux.
    pub fn new() -> Self {
        let saved_state = Self::save_and_disable();
        CriticalSection {
            saved_state,
            no_send_marker: core::marker::PhantomData,
        }
    }

    /// Architecture-specific: atomically read and disable interrupts,
    /// returning the saved state.
    fn save_and_disable() -> usize {
        irq_save_and_disable()
    }

    /// Architecture-specific: restore the saved interrupt state.
    fn restore(saved_state: usize) {
        irq_restore(saved_state);
    }
}

impl Drop for CriticalSection {
    /// Restore the interrupt state that was saved when this `CriticalSection`
    /// was constructed.
    ///
    /// Equivalent to `local_irq_restore(flags)` in Linux.
    fn drop(&mut self) {
        Self::restore(self.saved_state);
    }
}

// CriticalSection is inherently per-core — it must not be sent across cores
// while it is live, since the saved IRQ state belongs to the core that saved it.
// However !Send is experimental and cannot be used in stable Rust.
// impl !Send for CriticalSection {}

// ==============================================================================
// Mutex
// ==============================================================================

/// A mutual exclusion primitive that saves and restores interrupt state.
///
/// Equivalent to a Linux spinlock used with `spin_lock_irqsave` /
/// `spin_lock_irqrestore`. Acquiring the lock disables interrupts on the
/// current core and saves their prior state; releasing restores exactly that
/// state.
///
/// # Nesting
///
/// ```rust (notest)
/// let outer = MY_MUTEX.lock();   // IRQs disabled; saved state = "enabled"
/// let inner = OTHER.lock();      // IRQs already disabled; saved state = "disabled"
/// drop(inner);                   // restores "disabled" -> IRQs stay off
/// drop(outer);                   // restores "enabled"  -> IRQs come back on
/// ```
///
/// Each guard independently captures and restores the state at the moment it
/// was acquired, so nesting is always correct.
#[derive(Debug)]
pub struct Mutex<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

/// RAII guard returned by [`Mutex::lock`] and [`Mutex::try_lock`].
///
/// Releasing the guard (via `drop` or end of scope) performs two operations
/// in this exact order:
///
/// 1. Releases the spinlock (`locked = false`).
/// 2. Restores the interrupt state saved when the lock was acquired.
///
/// This ordering is enforced by Rust's drop rules: the explicit `Drop` impl
/// runs before field drops, and `_cs` is the only field with a meaningful drop.
#[derive(Debug)]
pub struct MutexGuard<'a, T: ?Sized> {
    mutex: &'a Mutex<T>,
    /// Holds the saved IRQ state. Dropped AFTER the spinlock is released.
    /// The field name prefix `_` documents intent: kept alive for its Drop,
    /// not for direct use. Use an Option here so we can toggle whether we want
    /// to save/restore IRQs (e.g., for lock vs. lock_irqsave).
    _cs: Option<CriticalSection>,
}

// SAFETY: Mutex<T> is Send + Sync if T: Send — the spinlock + IRQ-save
// mechanism provides the necessary mutual exclusion across cores and ISRs.
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

// SAFETY: MutexGuard gives exclusive access — safe to send if T: Send.
// Sync requires T: Sync since multiple threads could observe the guard's
// Deref target simultaneously (in theory; in practice the lock prevents this).
unsafe impl<T: ?Sized + Send> Send for MutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

impl<T> Mutex<T> {
    /// Creates a new, unlocked mutex wrapping `value`.
    ///
    /// `const fn` — suitable for use in `static` initializers.
    pub const fn new(value: T) -> Self {
        Mutex {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    /// Creates a new mutex that is **pre-locked**.
    ///
    /// Useful for sequencing: one core creates the mutex locked, performs
    /// setup work, then calls [`force_unlock`](Mutex::force_unlock) to
    /// release a waiting core.
    pub const fn new_locked(value: T) -> Self {
        Mutex {
            locked: AtomicBool::new(true),
            data: UnsafeCell::new(value),
        }
    }

    /// Force-unlock the mutex without going through a guard.
    ///
    /// # Safety
    ///
    /// The caller must guarantee no other core holds the lock and that the
    /// protected data is in a consistent state. Intended for:
    /// - Deadlock recovery.
    /// - Releasing a `new_locked` mutex after setup is complete.
    pub unsafe fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquire the mutex, spinning until the lock is obtained.
    ///
    /// Interrupts are NOT disabled. The returned [`MutexGuard`] will release
    /// the lock when dropped. If you need to disable interrupts before acquiring the lock,
    /// use [`Mutex::lock_irqsave`] instead.
    ///
    /// # Deadlock
    ///
    /// If the same core attempts to acquire a mutex it already holds (directly
    /// or via an interrupt handler), it will spin forever. This is the same
    /// behavior as Linux spinlocks.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Yield the pipeline while the lock is contended.
            // On x86-64: emits `pause` (reduces speculation, saves power).
            // On RISC-V: emits `nop` or a fence hint depending on the toolchain.
            core::hint::spin_loop();
        }

        MutexGuard {
            mutex: self,
            _cs: None,
        }
    }

    /// Acquire the mutex, spinning until the lock is obtained.
    ///
    /// Interrupts are disabled and their state is saved before the spin loop
    /// begins. The returned [`MutexGuard`] will release the lock and restore
    /// interrupts when dropped. If you do not need to change the interrupt state,
    /// use [`lock`](Mutex::lock) instead.
    ///
    /// # Deadlock
    ///
    /// If the same core attempts to acquire a mutex it already holds (directly
    /// or via an interrupt handler), it will spin forever. This is the same
    /// behavior as Linux spinlocks.
    ///
    pub fn lock_irqsave(&self) -> MutexGuard<'_, T> {
        // Save interrupt state and disable interrupts BEFORE spinning.
        // This prevents an ISR from attempting to acquire the same lock
        // while we hold it, which would deadlock.
        let cs = CriticalSection::new();

        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Yield the pipeline while the lock is contended.
            // On x86-64: emits `pause` (reduces speculation, saves power).
            // On RISC-V: emits `nop` or a fence hint depending on the toolchain.
            core::hint::spin_loop();
        }

        MutexGuard {
            mutex: self,
            _cs: Some(cs),
        }
    }

    /// Attempt to acquire the mutex without spinning.
    ///
    /// Returns `Some(guard)` if the lock was free and is now held, or `None`
    /// if the lock was already taken.
    ///
    /// This does not change the state of interrupts.
    ///
    /// Use [`try_lock_irqsave`](Mutex::try_lock_irqsave) if you need to disable interrupts.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                mutex: self,
                _cs: None,
            })
        } else {
            // cs drops here and interrupts restored before we return None.
            None
        }
    }

    /// Attempt to acquire the mutex without spinning.
    ///
    /// Returns `Some(guard)` if the lock was free and is now held, or `None`
    /// if the lock was already taken. In the `None` case, interrupts are
    /// restored before returning.
    ///
    /// Equivalent to `spin_trylock_irqsave(&lock, flags)`.
    pub fn try_lock_irqsave(&self) -> Option<MutexGuard<'_, T>> {
        // Disable interrupts first — same reason as lock().
        // If the CAS fails, cs is dropped at the end of this function,
        // restoring interrupts automatically.
        let cs = CriticalSection::new();

        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                mutex: self,
                _cs: Some(cs),
            })
        } else {
            // cs drops here and interrupts restored before we return None.
            None
        }
    }

    /// Returns `true` if the mutex is currently locked.
    ///
    /// This is inherently racy — the state may change between the load and
    /// any subsequent operation. Suitable only for diagnostics / assertions.
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }
}

// ==============================================================================
// MutexGuard — Drop, Deref, DerefMut
// ==============================================================================

impl<T: ?Sized> MutexGuard<'_, T> {
    /// Returns a reference to the underlying data.
    ///
    /// The returned reference is valid for the lifetime of the guard.
    pub fn as_ref(&self) -> &T {
        // SAFETY: We hold the spinlock, so we have exclusive access.
        unsafe { &*self.mutex.data.get() }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// The returned reference is valid for the lifetime of the guard.
    pub fn as_mut(&mut self) -> &mut T {
        // SAFETY: We hold the spinlock, so we have exclusive mutable access.
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    /// Release the spinlock.
    ///
    /// This runs BEFORE `_cs` is dropped, guaranteeing:
    ///   1. Spinlock released  (this impl)
    ///   2. Interrupts restored (`_cs` field drop)
    ///
    /// Equivalent to `spin_unlock_irqrestore(&lock, flags)`.
    fn drop(&mut self) {
        // Release ordering ensures all writes inside the critical section
        // are visible to the next core that acquires the lock.
        self.mutex.locked.store(false, Ordering::Release);
        // _cs is dropped after this function returns, restoring IRQ state.
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: We hold the spinlock, so we have exclusive access.
        self.as_ref()
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: We hold the spinlock, so we have exclusive mutable access.
        self.as_mut()
    }
}

/// ### Watch a variable until it changes
///
/// Spin loop keeps reading value until it changes to the given value.
pub fn spin_watch<'a, T>(val: &'a T, change_to: T)
where
    T: PartialEq,
{
    unsafe {
        while read_volatile(val) != change_to {
            core::hint::spin_loop();
        }
    }
}

// ==============================================================================
// OnceLock - no_std implementation for kernels.
// ==============================================================================

/// # OnceLock
///
/// A write-once, read-many static cell for `no_std` kernels.
///
/// Equivalent to `std::sync::OnceLock` but built on atomics with no
/// OS dependencies. Safe for use across multiple cores once SMP is live.
///
/// ## Usage
///
/// ```rust
/// static CONSOLE: OnceLock<Console> = OnceLock::new();
///
/// // Boot (once):
/// CONSOLE.set(Console::new(device)).ok();
///
/// // Anywhere:
/// if let Some(c) = CONSOLE.get() {
///     c.write(b"hello\n");
/// }
/// ```
///
/// ## SMP Safety
///
/// Uses a three-state atomic (UNINITIALIZED -> INITIALIZING -> INITIALIZED)
/// so that concurrent `set()` calls on different cores are safe; only one
/// caller wins, the rest get `Err(value)` back.
///
/// ## Ordering Notes
///
/// - `set()` uses `AcqRel` on the CAS and `Release` on the final store so
///   the written value is fully visible before the state flips to INITIALIZED.
/// - `get()` uses `Acquire` so it synchronizes with the `Release` store in
///   `set()`, guaranteeing the caller sees the fully initialized value.

// Three-state machine stored in the atomic:
const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const INITIALIZED: u8 = 2;

/// A write-once static cell.
///
/// `T` must be `Send` for `OnceLock<T>` to be `Send + Sync`; i.e. safe
/// to share across cores. Your `Console` already implements both.
#[derive(Debug)]
pub struct OnceLock<T> {
    state: AtomicU8,
    data: UnsafeCell<MaybeUninit<T>>,
}

// SAFETY:
//   The atomic state machine ensures at most one writer and that all readers
//   see a fully initialized value. T must be Send for cross-core sharing.
unsafe impl<T: Send> Send for OnceLock<T> {}
unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}

impl<T> OnceLock<T> {
    /// Create an uninitialized cell.
    ///
    /// This is `const` so it can be used directly in a `static`:
    /// ```rust
    /// static FOO: OnceLock<Console> = OnceLock::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(UNINITIALIZED),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Create a cell that is already initialized with `value`.
    ///
    /// This is not `const` because it calls `write()` on the `UnsafeCell`, which is not a `const fn`.
    /// ```rust
    /// let cell = OnceLock::new_with(Console::new(device));
    /// ```
    pub fn new_with(value: T) -> Self {
        let cell = Self::new();
        // SAFETY: We have exclusive access to the cell during construction.
        unsafe {
            (*cell.data.get()).write(value);
        }
        cell.state.store(INITIALIZED, Ordering::Release);
        cell
    }

    /// Initialize the cell with `value`.
    ///
    /// Returns `Ok(())` if this call won the race and initialized the cell.
    /// Returns `Err(value)` if the cell was already initialized or another
    /// core is currently initializing it, and the value is returned to the
    /// caller so it can be dropped or reused.
    ///
    /// # Panics
    ///
    /// Does not panic. All failure modes return `Err`.
    pub fn set(&self, value: T) -> Result<(), T> {
        // Attempt to claim the INITIALIZING slot.
        match self.state.compare_exchange(
            UNINITIALIZED,
            INITIALIZING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // We won the race — write the value.
                unsafe { (*self.data.get()).write(value) };
                // Publish: flip to INITIALIZED. Release ensures the write
                // above is visible to any core that observes INITIALIZED.
                self.state.store(INITIALIZED, Ordering::Release);
                Ok(())
            }
            Err(_) => {
                // Either already initialized or another core is mid-init.
                Err(value)
            }
        }
    }

    /// Returns a reference to the value if initialized, or `None`.
    ///
    /// The `Acquire` load synchronizes with the `Release` store in `set()`,
    /// so the returned reference is always fully initialized.
    pub fn get(&self) -> Option<&T> {
        if self.state.load(Ordering::Acquire) == INITIALIZED {
            // SAFETY: state == INITIALIZED guarantees the write in set()
            // completed and is visible due to Acquire/Release pairing.
            Some(unsafe { (*self.data.get()).assume_init_ref() })
        } else {
            None
        }
    }

    /// Gets the mutable reference to the underlying value.
    ///
    /// Returns `None` if the cell is uninitialized.
    ///
    /// This method never blocks. Since it borrows the `OnceLock` mutably,
    /// it is statically guaranteed that no active borrows to the `OnceLock`
    /// exist, including from other threads.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.initialized() {
            // Safe b/c checked initialized and we have a unique access
            Some(unsafe { (*self.data.get_mut()).assume_init_mut() })
        } else {
            None
        }
    }

    /// Returns a reference to the value, initializing it with `f` if needed.
    ///
    /// If two cores race here, only one will call `f` — the other will spin
    /// until the first core finishes, then return the initialized value.
    ///
    /// This is the right call when you want a guaranteed reference back:
    /// ```rust (notest)
    /// let console = CONSOLE.get_or_init(|| Console::new(init_uart(BASE)));
    /// ```
    pub fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        // Fast path — already initialized.
        if self.state.load(Ordering::Acquire) == INITIALIZED {
            return unsafe { (*self.data.get()).assume_init_ref() };
        }

        // Try to claim INITIALIZING.
        if self
            .state
            .compare_exchange(
                UNINITIALIZED,
                INITIALIZING,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            // We won — initialize.
            unsafe { (*self.data.get()).write(f()) };
            self.state.store(INITIALIZED, Ordering::Release);
        } else {
            // Another core is initializing — spin until it finishes.
            // In a kernel this is extremely short (a handful of cycles).
            while self.state.load(Ordering::Acquire) != INITIALIZED {
                core::hint::spin_loop();
            }
        }

        unsafe { (*self.data.get()).assume_init_ref() }
    }

    /// Takes the value out of the cell, leaving it uninitialized.
    ///
    /// Returns `Some(value)` if the cell was initialized, or `None` if it was uninitialized.
    ///
    /// This method never blocks. Since it borrows the `OnceLock` mutably,
    /// it is statically guaranteed that no active borrows to the `OnceLock`
    /// exist, including from other threads.
    pub fn take(&mut self) -> Option<T> {
        if self.initialized() {
            self.state.store(UNINITIALIZED, Ordering::Release);
            // SAFETY: `self.data` is initialized and contains a valid `T`.
            // `self.state` is reset, so `initialized()` will be false again
            // which prevents the value from being read twice.
            unsafe { Some((*self.data.get()).assume_init_read()) }
        } else {
            None
        }
    }

    /// Returns `true` if the cell has been initialized.
    pub fn initialized(&self) -> bool {
        self.state.load(Ordering::Acquire) == INITIALIZED
    }
}

impl<T> Drop for OnceLock<T> {
    fn drop(&mut self) {
        // `get_mut` bypasses atomics since we have exclusive access via &mut.
        if self.initialized() {
            // SAFETY: we have &mut self so no other references exist.
            unsafe { (*self.data.get()).assume_init_drop() };
        }
    }
}

/// A blended OnceLock and Mutex. This is for data that isn't known until
/// after initialization, but it still needs a mutex.
pub type OnceMutex<T> = OnceLock<Mutex<T>>;
