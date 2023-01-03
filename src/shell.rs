use std::{
    ffi::{CStr, CString},
    fs,
    io::Read,
    os::unix::io::FromRawFd,
    str,
};

use libc::{__errno_location, strerror, WEXITSTATUS, WIFEXITED};
use thiserror::Error;

use crate::wrapper::{LibCWrapper, LibCWrapperImpl};

#[derive(Error, Debug)]
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
    fn format_kernel_error_message<S: AsRef<str>>(description: S) -> String {
        let (errno, strerror) = unsafe {
            let errno = *__errno_location();
            let ptr = strerror(errno);
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

lazy_static! {
    static ref READ_MODE: CString = CString::new("r").unwrap();
}

pub type Out = (i32, String);

pub fn command<S: AsRef<str>>(c: S) -> Result<Out, RashError> {
    let ref delegate = LibCWrapperImpl::new();
    _command(c, delegate)
}

fn _command<S, D>(command: S, delegate: &D) -> Result<Out, RashError>
where
    S: AsRef<str>,
    D: LibCWrapper,
{
    let (stream, exit_status) = unsafe {
        let command_as_c_string = format_command_as_c_string(command)?;

        let c_stream = popen_checked(command_as_c_string, delegate)?;

        let fd = dup_fd_checked(c_stream, delegate)?;

        let exit_status = delegate.pclose(c_stream);
        (fs::File::from_raw_fd(fd), exit_status)
    };

    let return_code = get_process_return_code(exit_status)?;

    return match str::from_utf8(&read_stream_into_buffer(stream)?) {
        Ok(s) => Ok((return_code, s.to_string())),
        Err(e) => Err(RashError::FailedToReadStdout {
            message: e.to_string(),
        }),
    };
}

fn format_command_as_c_string<S: AsRef<str>>(cmd: S) -> Result<CString, RashError> {
    return CString::new(into_bash_command(cmd)).map_err(|e| RashError::NullByteInCommand {
        message: e.to_string(),
    });
}

fn into_bash_command<S: AsRef<str>>(s: S) -> String {
    format!("/usr/bin/env bash -c '{}'", s.as_ref())
}

unsafe fn popen_checked<D>(command: CString, delegate: &D) -> Result<*mut libc::FILE, RashError>
where
    D: LibCWrapper,
{
    let stream = delegate.popen(command.as_ptr());
    if stream.is_null() {
        return Err(RashError::KernelError {
            message: RashError::format_kernel_error_message(
                "The call to popen returned a null stream.",
            ),
        });
    }
    Ok(stream)
}

unsafe fn dup_fd_checked<D>(stream: *mut libc::FILE, delegate: &D) -> Result<libc::c_int, RashError>
where
    D: LibCWrapper,
{
    let fd = delegate.dup(delegate.fileno(stream));
    if fd == -1 {
        delegate.pclose(stream);
        return Err(RashError::KernelError {
            message: RashError::format_kernel_error_message("The call to dup returned -1."),
        });
    }
    Ok(fd)
}

fn get_process_return_code(process_exit_status: libc::c_int) -> Result<i32, RashError> {
    if WIFEXITED(process_exit_status) {
        return Ok(WEXITSTATUS(process_exit_status));
    }

    Err(RashError::KernelError {
        message: RashError::format_kernel_error_message(
            "WIFEXITED was false. The call to popen didn't exit normally.",
        ),
    })
}

fn read_stream_into_buffer(mut stream: fs::File) -> Result<Vec<u8>, RashError> {
    let mut buffer = Vec::new();
    match stream.read_to_end(&mut buffer) {
        Ok(_) => Ok(buffer),
        Err(e) => Err(RashError::FailedToReadStdout {
            message: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{command, into_bash_command, RashError};

    #[test]
    fn test_commands_return_zero() {
        [
            "ls",
            "ls -l | cat -",
            "ls | cat $(echo '-')",
            "[[ 5 -eq $((3 + 2)) ]]",
            "/bin/sh -c 'echo hi'",
            "exit 0",
        ]
        .iter()
        .for_each(|c| {
            let (r, _) = command(c).unwrap();
            assert_eq!(r, 0);
        });
    }

    #[test]
    fn test_commands_return_non_zero() {
        [("i_am_not_a_valid_executable", 127), ("echo hi | grep 'bye'", 1), ("exit 54;", 54)]
            .iter()
            .for_each(move |(c, ret)| {
                let (r, _) = command(c).unwrap();
                assert_eq!(r, *ret);
            });
    }

    #[test]
    fn test_commands_stdout() {
        [
            ("", ""),
            ("echo hi", "hi\n"),
            ("echo hi >/dev/null", ""),
            ("echo -n $((3 + 2 - 1))", "4"),
        ]
        .iter()
        .for_each(move |(c, out)| {
            let (_, s) = command(c).unwrap();
            assert_eq!(s, *out);
        });
    }

    /*
    #[test]
    fn test_command_with_null_byte() {
        let bytes_with_zero: Vec<u8> = [5, 6, 7, 8, 0, 10, 11, 12].to_vec();
        let command_with_null_byte =
            unsafe { std::str::from_utf8_unchecked(&*bytes_with_zero) }.to_string();
        assert_eq!(
            command(command_with_null_byte),
            (
                -1,
                format_null_byte_error(
                    "nul byte found in provided data at position: 26".to_string()
                )
            )
        )
    }
    */

    #[test]
    fn test_script() {
        assert_eq!(command(PRETTY_TRIANGLE_SCRIPT).unwrap(), (0, String::from("*\n* *\n* * *\n")));
    }

    const PRETTY_TRIANGLE_SCRIPT: &str = r#"
        s="*"
        for i in {1..3}; do
            echo "$s"
            s="$s *"
        done;
        "#;

    #[test]
    fn test_into_bash_command() {
        assert_eq!(into_bash_command("blah"), "/usr/bin/env bash -c 'blah'".to_string());
    }
}
