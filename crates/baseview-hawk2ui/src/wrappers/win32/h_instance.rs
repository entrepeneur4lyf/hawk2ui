use std::ffi::c_void;
use std::ptr::{null_mut, NonNull};
use windows_core::{Error, Result};
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;

#[derive(Copy, Clone, PartialEq)]
pub struct HInstance(NonNull<c_void>);

// SAFETY: This is actually a pointer to the memory image of the executable file. It is guaranteed
// to be valid for the process's lifetime.
// This getting invalidated would imply our own executable has been unloaded already. At that point,
// pointer invalidation would the least of our concerns anyway.
unsafe impl Send for HInstance {}
// SAFETY: same as above
unsafe impl Sync for HInstance {}

impl HInstance {
    pub fn get() -> Result<Self> {
        let result = unsafe { GetModuleHandleW(null_mut()) };

        let Some(result) = NonNull::new(result) else {
            return Err(Error::from_win32());
        };

        Ok(Self(result))
    }

    #[inline]
    pub fn as_raw(&self) -> HINSTANCE {
        self.0.as_ptr()
    }
}
