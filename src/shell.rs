use std::{fs, os::unix::io::FromRawFd, str};

use libc::{WEXITSTATUS, WIFEXITED};

use crate::{
    checked,
    error::RashError,
    utils,
    wrapper::{LibCWrapper, LibCWrapperImpl},
};

pub type Out = (i32, String);

pub fn command<S: AsRef<str>>(c: S) -> Result<Out, RashError> {
    let ref delegate = LibCWrapperImpl::new();
    run_command(c, delegate)
}

fn run_command<S, D>(command: S, delegate: &D) -> Result<Out, RashError>
where
    S: AsRef<str>,
    D: LibCWrapper,
{
    let command_as_c_string = utils::format_command_as_c_string(command)?;

    let c_stream = unsafe { checked::popen(command_as_c_string, delegate)? };
    let fd = checked::dup(c_stream, delegate)?;
    let exit_status = checked::pclose(c_stream, delegate)?;
    let stream = unsafe { fs::File::from_raw_fd(fd) };

    let return_code = get_process_return_code(exit_status, delegate)?;

    return match str::from_utf8(&utils::read_file_into_buffer(stream)?) {
        Ok(s) => Ok((return_code, s.to_string())),
        Err(e) => Err(RashError::FailedToReadStdout {
            message: e.to_string(),
        }),
    };
}

fn get_process_return_code<D>(
    process_exit_status: libc::c_int,
    delegate: &D,
) -> Result<i32, RashError>
where
    D: LibCWrapper,
{
    if WIFEXITED(process_exit_status) {
        return Ok(WEXITSTATUS(process_exit_status));
    }

    Err(RashError::KernelError {
        message: RashError::format_kernel_error_message(
            delegate,
            "WIFEXITED was false. The call to popen didn't exit normally.",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
