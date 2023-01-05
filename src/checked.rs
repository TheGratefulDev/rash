use std::ffi::CString;

use crate::{error::RashError, wrapper::LibCWrapper};

pub(crate) unsafe fn popen<D>(command: CString, delegate: &D) -> Result<*mut libc::FILE, RashError>
where
    D: LibCWrapper,
{
    let stream = delegate.popen(command.as_ptr());
    if stream.is_null() {
        return Err(RashError::KernelError {
            message: RashError::format_kernel_error_message(
                delegate,
                "The call to popen returned a null stream.",
            ),
        });
    }
    Ok(stream)
}

pub(crate) fn dup<D>(stream: *mut libc::FILE, delegate: &D) -> Result<libc::c_int, RashError>
where
    D: LibCWrapper,
{
    Ok(unsafe {
        let fd = delegate.dup(delegate.fileno(stream));
        if fd == -1 {
            delegate.pclose(stream);
            return Err(RashError::KernelError {
                message: RashError::format_kernel_error_message(
                    delegate,
                    "The call to dup returned -1.",
                ),
            });
        }
        fd
    })
}

pub(crate) fn pclose<D>(c_stream: *mut libc::FILE, delegate: &D) -> Result<libc::c_int, RashError>
where
    D: LibCWrapper,
{
    Ok(unsafe {
        let exit_status = delegate.pclose(c_stream);
        if exit_status == -1 {
            return Err(RashError::KernelError {
                message: RashError::format_kernel_error_message(
                    delegate,
                    "The call to pclose returned -1.",
                ),
            });
        }
        exit_status
    })
}

#[cfg(test)]
mod tests {
    use std::{ffi::CString, mem::transmute, sync::Mutex};

    use libc::{c_char, c_int, FILE};
    use once_cell::sync::Lazy;

    use crate::{error::RashError, utils::format_command_as_c_string};

    use super::*;

    static mut HELLO: *const CString = 0 as *const CString;

    pub struct NullLibCWrapper {}

    impl LibCWrapper for NullLibCWrapper {
        unsafe fn popen(&self, _command: *const c_char) -> *mut FILE {
            std::ptr::null_mut()
        }

        unsafe fn fileno(&self, _stream: *mut FILE) -> c_int {
            -1 as c_int
        }

        unsafe fn dup(&self, _fd: c_int) -> c_int {
            -1 as c_int
        }

        unsafe fn pclose(&self, _stream: *mut FILE) -> c_int {
            -1 as c_int
        }

        unsafe fn __errno_location(&self) -> *mut c_int {
            let b = Box::new(7);
            Box::into_raw(b) as *mut c_int
        }

        unsafe fn strerror(&self, _n: c_int) -> *mut c_char {
            let boxed = Box::new("Hello\0");
            HELLO = transmute(boxed);
            return (&*HELLO).as_ptr() as *mut c_char;
        }
    }

    #[test]
    fn test_popen() -> Result<(), RashError> {
        let result = unsafe { popen(format_command_as_c_string("hi")?, &NullLibCWrapper {}) };
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: The call to popen returned a null stream., strerror output: Hello.".to_string()
            })
        );
        Ok(())
    }

    #[test]
    fn test_dup() {
        static PCLOSE_CALLED: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));

        struct CountedNullLibCWrapper<D>
        where
            D: LibCWrapper,
        {
            delegate: D,
        }

        impl<D> LibCWrapper for CountedNullLibCWrapper<D>
        where
            D: LibCWrapper,
        {
            unsafe fn popen(&self, command: *const c_char) -> *mut FILE {
                self.delegate.popen(command)
            }

            unsafe fn fileno(&self, stream: *mut FILE) -> c_int {
                self.delegate.fileno(stream)
            }

            unsafe fn dup(&self, fd: c_int) -> c_int {
                self.delegate.dup(fd)
            }

            unsafe fn pclose(&self, stream: *mut FILE) -> c_int {
                *PCLOSE_CALLED.lock().unwrap() = true;
                self.delegate.pclose(stream)
            }

            unsafe fn __errno_location(&self) -> *mut c_int {
                self.delegate.__errno_location()
            }

            unsafe fn strerror(&self, n: c_int) -> *mut c_char {
                self.delegate.strerror(n)
            }
        }

        let ref delegate = CountedNullLibCWrapper {
            delegate: NullLibCWrapper {},
        };

        // Any mut ptr will work for the FILE.
        let result = dup(std::ptr::null_mut(), delegate);
        assert!(result.is_err());
        assert!(*PCLOSE_CALLED.lock().unwrap());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: The call to dup returned -1., strerror output: Hello.".to_string()
            })
        );
    }

    #[test]
    fn test_pclose() {
        let result = pclose(std::ptr::null_mut(), &NullLibCWrapper {});
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: The call to pclose returned -1., strerror output: Hello.".to_string()
            })
        );
    }
}
