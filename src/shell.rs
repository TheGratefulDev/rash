use std::{
    ffi::{CStr, CString},
    fs::File,
    io::Read,
    os::unix::io::FromRawFd,
    str,
};

use libc::{dup, fileno, pclose, popen, WEXITSTATUS, WIFEXITED};

lazy_static! {
    static ref READ_MODE: CString = CString::new("r").unwrap();
}

pub type Out = (i32, String);

pub fn command<S: AsRef<str>>(cmd: S) -> Out {
    let (mut f, exit_status) = unsafe {
        let cmd_as_c_string = match CString::new(into_bash_command(cmd)) {
            Ok(c) => c,
            Err(e) => return (-1, format_null_byte_error(e.to_string())),
        };

        let stream = popen(cmd_as_c_string.as_ptr(), READ_MODE.as_ptr());
        if stream.is_null() {
            return kernel_error("The call to popen returned a null stream.");
        }

        let fd = dup(fileno(stream));
        if fd == -1 {
            pclose(stream);
            return kernel_error("The call to dup returned -1.");
        }

        let exit_status = pclose(stream);
        (File::from_raw_fd(fd), exit_status)
    };

    let ret_val: i32;
    if WIFEXITED(exit_status) {
        ret_val = WEXITSTATUS(exit_status);
    } else {
        return kernel_error("WIFEXITED was false. The call to popen didn't exit normally.");
    }

    let mut buffer = Vec::new();
    if let Err(e) = f.read_to_end(&mut buffer) {
        return (ret_val, format_stdout_error(e.to_string()));
    }

    return match str::from_utf8(&buffer) {
        Ok(s) => (ret_val, s.to_string()),
        Err(e) => (ret_val, format_stdout_error(e.to_string())),
    };
}

fn into_bash_command<S: AsRef<str>>(s: S) -> String {
    format!("/usr/bin/env bash -c '{}'", s.as_ref())
}

fn kernel_error<S: AsRef<str>>(description: S) -> Out {
    let (errno, strerror) = unsafe {
        let errno = *libc::__errno_location();
        let ptr = libc::strerror(errno);
        match CStr::from_ptr(ptr).to_str() {
            Ok(s) => (errno, s.to_string()),
            Err(err) => (errno, err.to_string()),
        }
    };

    let error_message = format!(
        "\n\
        ERROR: received errno {}.\n\
        Description: {}.\n\
        strerror output: {}.\n\
        ",
        errno.to_string(),
        description.as_ref(),
        strerror
    );

    (errno, error_message)
}

fn format_stdout_error(error_message: String) -> String {
    format!(
        "\n\
        ERROR: Couldn't obtain stdout.\n\
        Error message: {}\n\
        ",
        error_message
    )
}

fn format_null_byte_error(error_message: String) -> String {
    format!(
        "\n\
        ERROR: Command contained a null byte.\n\
        Error message: {}\n\
        ",
        error_message
    )
}

#[cfg(test)]
mod tests {
    use super::{
        command, format_null_byte_error, format_stdout_error, into_bash_command, kernel_error,
    };

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
            let (r, _) = command(c);
            assert_eq!(r, 0);
        });
    }

    #[test]
    fn test_commands_return_non_zero() {
        [("i_am_not_a_valid_executable", 127), ("echo hi | grep 'bye'", 1), ("exit 54;", 54)]
            .iter()
            .for_each(move |(c, ret)| {
                let (r, _) = command(c);
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
            let (_, s) = command(c);
            assert_eq!(s, *out);
        });
    }

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

    #[test]
    fn test_script() {
        let (r, s) = command(PRETTY_TRIANGLE_SCRIPT);
        assert_eq!(r, 0);
        assert_eq!(s, String::from("*\n* *\n* * *\n"))
    }

    const PRETTY_TRIANGLE_SCRIPT: &str = r#"
        s="*"
        for i in {1..3}; do
            echo "$s"
            s="$s *"
        done;
        "#;

    #[test]
    fn test_format_stdout_error() {
        assert_eq!(
            format_stdout_error("my error message".to_string()),
            EXPECTED_STDOUT_ERROR_MESSAGE.to_string()
        );
    }

    const EXPECTED_STDOUT_ERROR_MESSAGE: &str = "\n\
        ERROR: Couldn't obtain stdout.\n\
        Error message: my error message\n\
    ";

    #[test]
    fn test_format_null_byte_error() {
        assert_eq!(
            format_null_byte_error("my error message".to_string()),
            EXPECTED_NULL_BYTE_ERROR_MESSAGE.to_string()
        );
    }

    const EXPECTED_NULL_BYTE_ERROR_MESSAGE: &str = "\n\
        ERROR: Command contained a null byte.\n\
        Error message: my error message\n\
    ";

    #[test]
    fn test_kernel_error_with_no_error() {
        assert_eq!(
            kernel_error("my description"),
            (0, EXPECTED_KERNEL_ERROR_MESSAGE_WHEN_NO_ERROR.to_string())
        );
    }

    const EXPECTED_KERNEL_ERROR_MESSAGE_WHEN_NO_ERROR: &str = "\n\
        ERROR: received errno 0.\n\
        Description: my description.\n\
        strerror output: Success.\n\
    ";

    #[test]
    fn test_into_bash_command() {
        assert_eq!(into_bash_command("blah"), "/usr/bin/env bash -c 'blah'".to_string());
    }
}
