use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::str;

use libc::{dup, fileno, pclose, popen, WEXITSTATUS, WIFEXITED};

lazy_static! {
    static ref READ_MODE: CString = CString::new("r").unwrap();
}

pub type Out = (i32, String);

pub fn command<S: AsRef<str>>(cmd: S) -> Out {
    let (mut f, exit_status) = unsafe {
        let cmd_as_c_string = match CString::new(into_bash_command(cmd)) {
            Ok(c) => c,
            Err(e) => return (-1, e.to_string())
        };

        let stream = popen(
            cmd_as_c_string.as_ptr(),
            READ_MODE.as_ptr(),
        );
        if stream.is_null() {
            return error("popen returned null.");
        }

        let fd = dup(fileno(stream));
        if fd == -1 {
            pclose(stream);
            return error("dup returned -1.");
        }

        let exit_status = pclose(stream);
        (File::from_raw_fd(fd), exit_status)
    };

    let ret_val: i32;
    if WIFEXITED(exit_status) {
        ret_val = WEXITSTATUS(exit_status);
    } else {
        return error("WIFEXITED was false. The call to popen didn't exit normally.");
    }

    let mut buffer = Vec::new();
    if let Err(e) = f.read_to_end(&mut buffer) {
        return error(e.to_string());
    }

    return match str::from_utf8(&buffer) {
        Ok(s) => (ret_val, s.to_string()),
        Err(e) => error(e.to_string())
    };
}

fn into_bash_command<S: AsRef<str>>(s: S) -> String { format!("/bin/bash -c '{}'", s.as_ref()) }

fn error<S: AsRef<str>>(description: S) -> Out {
    let (errno, strerror) = unsafe {
        let errno = *libc::__errno_location();
        let ptr = libc::strerror(errno);
        match CStr::from_ptr(ptr).to_str() {
            Ok(s) => (errno, s.to_string()),
            Err(err) => (errno, err.to_string())
        }
    };

    let out = format!(
        "ERROR: received error code {}.\
            Description: {}.\
            strerror output: {}.", errno.to_string(), description.as_ref(), strerror);

    (errno, out)
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn test_commands_return_zero() {
        [
            "ls",
            "ls -l | cat -",
            "ls | cat $(echo '-')",
            "[[ 5 -eq $((3 + 2)) ]]",
            "/bin/sh -c 'echo hi'",
            "exit 0"
        ]
            .iter()
            .for_each(|c| {
                let (r, _) = command(c);
                assert_eq!(r, 0);
            });
    }

    #[test]
    fn test_commands_return_non_zero() {
        [
            ("i_am_not_a_valid_executable", 127),
            ("echo hi | grep 'bye'", 1),
            ("exit 54;", 54)
        ]
            .iter()
            .for_each(move |(c, ret)| {
                let (r, _) = command(c);
                assert_eq!(r, *ret);
            });
    }

    #[test]
    fn test_stdout() {
        [
            ("", ""),
            ("echo hi", "hi\n"),
            ("echo hi >/dev/null", ""),
            ("echo -n $((3 + 2 - 1))", "4")
        ]
            .iter()
            .for_each(move |(c, out)| {
                let (_, s) = command(c);
                assert_eq!(s, *out);
            });
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
}
