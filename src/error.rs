use std::ffi::CStr;

use thiserror::Error;

use crate::wrapper::LibCWrapper;

#[derive(Error, Debug, PartialEq)]
pub enum RashError {
    #[error("Null byte in command: {:?}", message)]
    NullByteInCommand {
        message: String,
    },
    #[error("{:?}", message)]
    KernelError {
        message: String,
    },
    #[error("Couldn't read stdout: {:?}", message)]
    FailedToReadStdout {
        message: String,
    },
}

impl RashError {
    pub(crate) fn format_kernel_error_message<D, S>(delegate: &D, description: S) -> String
    where
        D: LibCWrapper,
        S: AsRef<str>,
    {
        let (errno, strerror) = unsafe {
            let errno = *delegate.__errno_location();
            let ptr = delegate.strerror(errno);
            match CStr::from_ptr(ptr).to_str() {
                Ok(s) => (errno, s.to_string()),
                Err(err) => (errno, err.to_string()),
            }
        };

        format!(
            "Received errno {}, Description: {}, strerror output: {}.",
            errno.to_string(),
            description.as_ref(),
            strerror
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, mem::transmute, sync::Once};

    use libc::{c_char, c_int, FILE};

    use super::*;

    static START: Once = Once::new();
    static mut HELLO: *const CString = 0 as *const CString;

    struct MockLibCWrapper {}

    impl LibCWrapper for MockLibCWrapper {
        unsafe fn popen(&self, command: *const c_char) -> *mut FILE {
            let read_mode = CString::new("r").unwrap();
            libc::popen(command, read_mode.as_ptr())
        }

        unsafe fn fileno(&self, _stream: *mut FILE) -> c_int {
            7 as c_int
        }

        unsafe fn dup(&self, _fd: c_int) -> c_int {
            7 as c_int
        }

        unsafe fn pclose(&self, _stream: *mut FILE) -> c_int {
            7 as c_int
        }

        unsafe fn __errno_location(&self) -> *mut c_int {
            let b = Box::new(7);
            Box::into_raw(b) as *mut c_int
        }

        unsafe fn strerror(&self, _n: c_int) -> *mut c_char {
            START.call_once(|| unsafe {
                let boxed = Box::new("Hello\0");
                HELLO = transmute(boxed);
            });
            unsafe {
                return (&*HELLO).as_ptr() as *mut c_char;
            }
        }
    }

    #[test]
    fn test_format_kernel_error_message() {
        let mock_wrapper = MockLibCWrapper {};
        let result = RashError::format_kernel_error_message(&mock_wrapper, "My description");

        assert_eq!(
            result,
            "Received errno 7, Description: My description, strerror output: Hello."
        );
    }
}
