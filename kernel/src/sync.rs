//! Interrupt-safe spin lock.
//!
//! Single-CPU system: a lock disables interrupts for its hold duration and
//! restores the previous interrupt flag on release. This prevents both
//! re-entrancy and the classic "IRQ takes the lock the interrupted code
//! already holds" deadlock.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

pub struct IrqLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for IrqLock<T> {}

pub struct IrqGuard<'a, T> {
    lock: &'a IrqLock<T>,
    interrupts_were_enabled: bool,
}

impl<T> IrqLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> IrqGuard<'_, T> {
        let interrupts_were_enabled = crate::arch::cpu::interrupts_enabled();
        crate::arch::cpu::disable_interrupts();
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        IrqGuard {
            lock: self,
            interrupts_were_enabled,
        }
    }

    /// Lock without touching interrupt state. Only for contexts where
    /// interrupts are already disabled and this is documented.
    pub unsafe fn lock_no_irq(&self) -> IrqGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        IrqGuard {
            lock: self,
            interrupts_were_enabled: false,
        }
    }
}

impl<T> Deref for IrqGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for IrqGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for IrqGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        if self.interrupts_were_enabled {
            crate::arch::cpu::enable_interrupts();
        }
    }
}

/// A cell that can be mutated through a shared reference. Callers must
/// guarantee exclusive access (we use it for boot-time globals).
pub struct SyncUnsafeCell<T> {
    inner: UnsafeCell<T>,
}

unsafe impl<T> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }

    pub fn get(&self) -> *mut T {
        self.inner.get()
    }
}
