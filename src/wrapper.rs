use libc::{c_char, c_int, FILE, WEXITSTATUS, WIFEXITED};
use std::ffi::CString;

use crate::RashError;

lazy_static! {
    static ref READ_MODE: CString = CString::new("r").unwrap();
    static ref SHELL_PATH: CString = CString::new("/bin/sh").unwrap();
    static ref SH: CString = CString::new("sh").unwrap();
    static ref COMMAND: CString = CString::new("-c").unwrap();
}

pub(crate) trait LibCWrapper {
    unsafe fn popen(&self, command: *const c_char) -> *mut FILE;
    unsafe fn fileno(&self, stream: *mut FILE) -> c_int;
    unsafe fn dup(&self, fd: c_int) -> c_int;
    unsafe fn pclose(&self, stream: *mut FILE) -> c_int;
    unsafe fn __errno_location(&self) -> *mut c_int;
    unsafe fn strerror(&self, n: c_int) -> *mut c_char;
}

pub(crate) struct LibCWrapperImpl {}

impl LibCWrapper for LibCWrapperImpl {
    unsafe fn popen(&self, command: *const c_char) -> *mut FILE {
        libc::popen(command, READ_MODE.as_ptr())
    }

    unsafe fn fileno(&self, stream: *mut FILE) -> c_int {
        libc::fileno(stream)
    }

    unsafe fn dup(&self, fd: c_int) -> c_int {
        libc::dup(fd)
    }

    unsafe fn pclose(&self, stream: *mut FILE) -> c_int {
        libc::pclose(stream)
    }

    unsafe fn __errno_location(&self) -> *mut c_int {
        libc::__errno_location()
    }

    unsafe fn strerror(&self, n: c_int) -> *mut c_char {
        libc::strerror(n)
    }
}

pub(crate) trait CheckedLibCWrapper {
    unsafe fn popen(&self, command: CString) -> Result<*mut FILE, RashError>;
    unsafe fn pclose(&self, c_stream: *mut FILE) -> Result<c_int, RashError>;
    unsafe fn dup_fileno(&self, stream: *mut FILE) -> Result<c_int, RashError>;
    fn get_process_return_code(&self, process_exit_status: c_int) -> Result<i32, RashError>;
}

pub(crate) struct CheckedLibCWrapperImpl<L>
where
    L: LibCWrapper,
{
    delegate: L,
}

impl<L> CheckedLibCWrapperImpl<L>
where
    L: LibCWrapper,
{
    pub(crate) fn new(delegate: L) -> Self {
        Self {
            delegate,
        }
    }

    fn kernel_error<S: AsRef<str>>(&self, message: S) -> RashError {
        RashError::KernelError {
            message: RashError::format_kernel_error_message(&self.delegate, message),
        }
    }
}

impl<L> CheckedLibCWrapper for CheckedLibCWrapperImpl<L>
where
    L: LibCWrapper,
{
    unsafe fn popen(&self, command: CString) -> Result<*mut FILE, RashError> {
        let stream = self.delegate.popen(command.as_ptr());
        return if stream.is_null() {
            Err(self.kernel_error("The call to popen returned a null stream"))
        } else {
            Ok(stream)
        };
    }

    unsafe fn pclose(&self, c_stream: *mut FILE) -> Result<c_int, RashError> {
        let exit_status = self.delegate.pclose(c_stream);
        return if exit_status == -1 {
            Err(self.kernel_error("The call to pclose returned -1"))
        } else {
            Ok(exit_status)
        };
    }

    unsafe fn dup_fileno(&self, stream: *mut FILE) -> Result<c_int, RashError> {
        let fd = self.delegate.dup(self.delegate.fileno(stream));
        return if fd == -1 {
            self.delegate.pclose(stream);
            Err(self.kernel_error("The call to dup returned -1"))
        } else {
            Ok(fd)
        };
    }

    fn get_process_return_code(&self, process_exit_status: c_int) -> Result<i32, RashError> {
        if WIFEXITED(process_exit_status) {
            Ok(WEXITSTATUS(process_exit_status))
        } else {
            Err(self.kernel_error("WIFEXITED was false: The call to popen didn't exit normally"))
        }
    }
}

pub(crate) unsafe fn epopen(command: CString, fds: &mut [c_int; 3]) -> c_int {
    use libc::{_exit, close, dup, execl, fork, pipe};
    let mut in_fds: [c_int; 2] = [-1, -1];
    let mut out_fds: [c_int; 2] = [-1, -1];
    let mut err_fds: [c_int; 2] = [-1, -1];

    unsafe fn close_pipe(fds: &[c_int; 2]) {
        close(fds[0]);
        close(fds[1]);
    }

    let in_ret: c_int = pipe(in_fds.as_mut_ptr());
    if in_ret < 0 {
        return -1 as c_int;
    }

    let out_ret: c_int = pipe(out_fds.as_mut_ptr());
    if out_ret < 0 {
        close_pipe(&in_fds);
        return -1 as c_int;
    }

    let err_ret: c_int = pipe(err_fds.as_mut_ptr());
    if err_ret < 0 {
        close_pipe(&out_fds);
        close_pipe(&in_fds);
        return -1 as c_int;
    }

    match fork() {
        -1 => {
            close_pipe(&err_fds);
            close_pipe(&out_fds);
            close_pipe(&in_fds);
            return -1 as c_int;
        }
        0 => {
            close(in_fds[1]);
            close(out_fds[0]);
            close(err_fds[0]);

            close(0);
            dup(in_fds[0]);

            close(1);
            dup(out_fds[1]);

            close(2);
            dup(err_fds[1]);

            execl(
                SHELL_PATH.as_ptr(),
                SH.as_ptr(),
                COMMAND.as_ptr(),
                command.as_ptr(),
                std::ptr::null() as *const c_char,
            );
            _exit(1);
        }
        pid => {
            close(in_fds[0]);
            close(out_fds[1]);
            close(err_fds[1]);
            fds[0] = in_fds[1];
            fds[1] = out_fds[0];
            fds[2] = err_fds[0];
            return pid;
        }
    }
}

#[cfg(test)]
mod tests {
    use libc::{c_char, c_int, FILE};
    use once_cell::sync::Lazy;
    use rstest::{fixture, rstest};
    use std::{ffi::CString, fs::File, io::Read, mem::transmute, os::fd::FromRawFd, sync::Mutex};

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

    #[fixture]
    fn wrapper() -> impl CheckedLibCWrapper {
        CheckedLibCWrapperImpl::new(NullLibCWrapper {})
    }

    #[test]
    fn test_epopen_writes_to_stdout_with_simple_command() -> () {
        let mut fds: [c_int; 3] = [-1, -1, -1];
        unsafe {
            assert!(epopen(CString::new("echo -n hi").unwrap(), &mut fds) > 0);
            assert_eq!(read_from_fd(fds[1]), "hi".to_string());
        }
    }

    #[test]
    fn test_epopen_writes_to_stderr_with_simple_command() -> () {
        let mut fds: [c_int; 3] = [-1, -1, -1];
        unsafe {
            assert!(epopen(CString::new("echo -n hi >&2").unwrap(), &mut fds) > 0);
            assert_eq!(read_from_fd(fds[2]), "hi".to_string());
        }
    }

    #[test]
    fn test_epopen_writes_to_stdout_with_bash_command() -> () {
        let mut fds: [c_int; 3] = [-1, -1, -1];
        unsafe {
            assert!(
                epopen(CString::new("/usr/bin/env bash -c 'echo -n hi'").unwrap(), &mut fds) > 0
            );
            assert_eq!(read_from_fd(fds[1]), "hi".to_string());
        }
    }

    #[test]
    fn test_epopen_writes_to_both_stdout_and_stderr_with_bash_command() -> () {
        let mut fds: [c_int; 3] = [-1, -1, -1];
        unsafe {
            assert!(
                epopen(
                    CString::new("/usr/bin/env bash -c 'echo -n hi && echo -n bye >&2'").unwrap(),
                    &mut fds
                ) > 0
            );
            assert_eq!(read_from_fd(fds[1]), "hi".to_string());
            assert_eq!(read_from_fd(fds[2]), "bye".to_string());
        }
    }

    #[rstest]
    fn test_popen_returns_error_when_libc_popen_returns_a_null_ptr(
        wrapper: impl CheckedLibCWrapper,
    ) -> Result<(), RashError> {
        let result = unsafe { wrapper.popen(format_command_as_c_string("hi")?) };
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: \
                The call to popen returned a null stream, strerror output: Hello."
                    .to_string()
            })
        );
        Ok(())
    }

    #[test]
    fn test_dup_fd_returns_error_when_libc_dup_returns_minus_one() {
        static PCLOSE_CALLED_TIMES: Lazy<Mutex<i32>> = Lazy::new(|| Mutex::new(0));

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
                *PCLOSE_CALLED_TIMES.lock().unwrap() += 1;
                self.delegate.pclose(stream)
            }

            unsafe fn __errno_location(&self) -> *mut c_int {
                self.delegate.__errno_location()
            }

            unsafe fn strerror(&self, n: c_int) -> *mut c_char {
                self.delegate.strerror(n)
            }
        }

        let delegate = CountedNullLibCWrapper {
            delegate: NullLibCWrapper {},
        };

        let wrapper = CheckedLibCWrapperImpl::new(delegate);

        let result = unsafe { wrapper.dup_fileno(std::ptr::null_mut()) };
        assert!(result.is_err());
        assert_eq!(*PCLOSE_CALLED_TIMES.lock().unwrap(), 1);
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: \
                The call to dup returned -1, strerror output: Hello."
                    .to_string()
            })
        );
    }

    #[rstest]
    fn test_pclose_returns_error_when_libc_pclose_returns_minus_one(
        wrapper: impl CheckedLibCWrapper,
    ) {
        let result = unsafe { wrapper.pclose(std::ptr::null_mut()) };
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: \
                The call to pclose returned -1, strerror output: Hello."
                    .to_string()
            })
        );
    }

    #[rstest]
    fn test_get_process_return_code_returns_error_if_wifexited_is_false(
        wrapper: impl CheckedLibCWrapper,
    ) {
        let result = wrapper.get_process_return_code(128 + 1 as c_int);
        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::KernelError {
                message: "Received errno 7, Description: WIFEXITED was false: \
                The call to popen didn't exit normally, strerror output: Hello."
                    .to_string()
            })
        );
    }

    unsafe fn read_from_fd(fd: c_int) -> String {
        let mut file = File::from_raw_fd(fd);
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        buffer
    }
}
