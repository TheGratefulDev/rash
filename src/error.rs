use libc::{__errno_location, c_int};
use std::ffi::{CStr, NulError};
use thiserror::Error;

use crate::process::ProcessError;

/// The error thrown if something went wrong in the processing of the command.
#[cfg(unix)]
#[derive(Error, Debug, PartialEq)]
pub enum RashError {
    /// The given command contained a null byte.
    /// Commands must **not** contain null bytes as they're converted into CStrings.
    ///
    /// If this error is thrown, the error message will contain the position
    /// of the null byte in the command.
    #[error("Null byte found in command at pos {}", pos)]
    NullByteInCommand {
        pos: usize,
    },
    /// A system call failed.
    ///
    /// If this error is thrown, the error message will contain the errno,
    /// a description of syscall that failed, and the strerror output.
    #[error("{:?}", message)]
    KernelError {
        message: String,
    },
    /// We couldn't obtain stdout.
    /// This can occur if the stdout is not valid UTF-8
    /// or for any standard IO error kind.
    ///
    /// If this error is thrown, the error message will be the error message
    /// given by calling `to_string()` on the source error.
    #[error("Couldn't read stdout: {:?}", message)]
    FailedToReadStdout {
        message: String,
    },
    /// We couldn't obtain stderr.
    /// This can occur if the stderr is not valid UTF-8
    /// or for any standard IO error kind.
    ///
    /// If this error is thrown, the error message will be the error message
    /// given by calling `to_string()` on the source error.
    #[error("Couldn't read stderr: {:?}", message)]
    FailedToReadStderr {
        message: String,
    },
}

impl From<ProcessError> for RashError {
    fn from(v: ProcessError) -> Self {
        fn into_kernel_error<S: AsRef<str>>(s: S) -> RashError {
            RashError::KernelError {
                message: unsafe { RashError::format_kernel_error_message(s) },
            }
        }
        match v {
            ProcessError::CouldNotFork => into_kernel_error(v.to_string()),
            ProcessError::CouldNotCreatePipe => into_kernel_error(v.to_string()),
            ProcessError::CouldNotDupFd(_) => into_kernel_error(v.to_string()),
            ProcessError::OpenDidNotCloseNormally => into_kernel_error(v.to_string()),
            ProcessError::CouldNotGetStderr => RashError::FailedToReadStderr {
                message: v.to_string(),
            },
            ProcessError::CouldNotGetStdout => RashError::FailedToReadStdout {
                message: v.to_string(),
            },
        }
    }
}

impl From<NulError> for RashError {
    fn from(v: NulError) -> Self {
        RashError::NullByteInCommand {
            pos: v.nul_position(),
        }
    }
}

impl RashError {
    pub(crate) unsafe fn format_kernel_error_message<S: AsRef<str>>(description: S) -> String {
        let errno = *__errno_location();
        let strerror = Self::strerror(errno);
        format!(
            "Received errno {}, Description: {}, strerror output: {strerror}.",
            errno.to_string(),
            description.as_ref()
        )
    }

    unsafe fn strerror(errno: c_int) -> String {
        let strerror = libc::strerror(errno);
        if strerror.is_null() {
            return "Couldn't get strerror - libc::strerror returned null.".to_string();
        }
        return match CStr::from_ptr(strerror).to_str() {
            Ok(s) => s.to_string(),
            Err(e) => e.to_string(),
        };
    }
}
