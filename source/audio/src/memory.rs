//! Hands physical memory back to Windows once a load is over. Loading touches
//! far more pages than a running game reads again (decode buffers, data used
//! once); without this they stay resident for as long as nothing else needs
//! the memory. A page still in use comes back at its next touch, without a
//! disk read. Here, not in the game crates, because this crate alone may call
//! the system directly. Other systems have no such call here: nothing is
//! trimmed and the game runs the same.
#![allow(unsafe_code)]

#[cfg(windows)]
mod system {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn K32EmptyWorkingSet(process: *mut c_void) -> i32;
    }

    pub fn trim() -> bool {
        // SAFETY: the pseudo handle needs no closing; the call only marks this
        // process's own pages as trimmable.
        unsafe { K32EmptyWorkingSet(GetCurrentProcess()) != 0 }
    }
}

/// Lets go of this process's resident pages; whether the system did.
pub fn trim_working_set() -> bool {
    #[cfg(windows)]
    {
        system::trim()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
