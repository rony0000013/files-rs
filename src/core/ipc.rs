use tracing;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

#[derive(Debug)]
pub struct SingleInstanceError {
    pub message: String,
}

impl std::fmt::Display for SingleInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SingleInstanceError {}

pub struct SingleInstance {
    _handle: HANDLE,
}

impl SingleInstance {
    const MUTEX_NAME: &'static str = "Global\\FilesRust_SingleInstance_Mutex";

    /// Attempt to acquire the single instance lock.
    /// Returns `Ok(SingleInstance)` if this is the first instance.
    /// Returns `Err` if another instance is already running.
    pub fn new() -> Result<Self, SingleInstanceError> {
        let name: Vec<u16> = Self::MUTEX_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let pcwstr = PCWSTR(name.as_ptr());

        unsafe {
            let handle = CreateMutexW(None, false, pcwstr).map_err(|e| SingleInstanceError {
                message: format!("Failed to create mutex: {}", e),
            })?;

            // WAIT_TIMEOUT is 0x102. We check if the mutex is already held.
            if WaitForSingleObject(handle, 0) == windows::Win32::Foundation::WAIT_EVENT(0x102) {
                CloseHandle(handle);
                return Err(SingleInstanceError {
                    message: "Another instance is already running.".to_string(),
                });
            }

            Ok(Self { _handle: handle })
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self._handle);
        }
    }
}
