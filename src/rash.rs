#[macro_use]
extern crate lazy_static;

use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::str;

use libc::{dup, fileno, pclose, popen, WEXITSTATUS, WIFEXITED};

lazy_static! {
    static ref READ_MODE: CString = into_c_string(String::from("r"));
}

pub struct Output {
    pub return_value: i32,
    pub stdout: String,
}

pub fn command<S: AsRef<str>>(cmd: S) -> Output {
    let (f, exit_status) = unsafe {
        let stream = popen(
            into_c_string(into_bash_command(cmd)).as_ptr(),
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

    let output = read_file_into_string(f);

    let ret_val;
    if WIFEXITED(exit_status) {
        ret_val = WEXITSTATUS(exit_status);
    } else {
        return error("WIFEXITED was false. The call to popen didn't exit normally.");
    }

    Output {
        return_value: ret_val as i32,
        stdout: output.to_string(),
    }
}

fn into_c_string<S: AsRef<str>>(s: S) -> CString {
    let str = s.as_ref();
    CString::new(str).expect(format!("Couldn't convert {} into CString.", str).as_ref())
}

fn into_bash_command<S: AsRef<str>>(s: S) -> String {
    format!("/bin/bash -c \"{}\"", s.as_ref())
}

fn error<S: AsRef<str>>(description: S) -> Output {
    let (errno, strerror) = unsafe {
        let e = *libc::__errno_location();
        let ptr = libc::strerror(e);
        let s = CStr::from_ptr(ptr).to_str().expect("strerror didn't return valid utf-8.").to_string();
        (e, s)
    };
    Output {
        return_value: errno as i32,
        stdout: format!(
            "ERROR: received error code {}.\
            Description: {}.\
            strerror output: {}", errno.to_string(), description.as_ref(), strerror),
    }
}

fn read_file_into_string(mut f: File) -> String {
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).expect("Couldn't read file into buffer.");
    str::from_utf8(&buffer).expect("The buffer wasn't valid utf-8.").to_string()
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn test_various_commands_return_zero() {
        [
            "ls",
            "ls -l | cat -",
            "ls | cat $(echo '-')",
            "[[ 5 -eq $((3 + 2)) ]]",
            "exit 0"
        ]
            .iter()
            .for_each(|c| {
                let o = command(c);
                assert_eq!(o.return_value, 0);
            });
    }

    #[test]
    fn test_various_commands_that_should_fail() {
        [
            "i_am_not_a_valid_executable",
            "ls | grep blahblahblah",
            "exit 1;"
        ]
            .iter()
            .for_each(|c| {
                let o = command(c);
                assert_ne!(o.return_value, 0);
            });
    }

    #[test]
    fn test_stdout() {
        [
            ("", ""),
            ("echo hi", "hi\n"),
            ("echo hi >/dev/null", ""),
        ]
            .iter()
            .for_each(|c| {
                let o = command(c.0);
                assert_eq!(o.stdout, c.1);
            });
    }
}
