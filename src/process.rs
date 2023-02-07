use libc::{_exit, c_char, c_int, close, dup, execl, fork, pipe, waitpid, WEXITSTATUS, WIFEXITED};
use std::{ffi::CString, fs::File, io::Read, os::fd::FromRawFd};

lazy_static! {
    static ref SHELL_PATH: CString = CString::new("/bin/sh").unwrap();
    static ref SH: CString = CString::new("sh").unwrap();
    static ref COMMAND: CString = CString::new("-c").unwrap();
}

pub(crate) struct Process {
    fds: [c_int; 3],
    pid: c_int,
}

impl Process {
    pub(crate) fn new() -> Self {
        Self {
            fds: [-1, -1, -1],
            pid: -1,
        }
    }

    pub(crate) unsafe fn open(&mut self, command: CString) -> c_int {
        let mut in_fds: [c_int; 2] = [-1, -1];
        let mut out_fds: [c_int; 2] = [-1, -1];
        let mut err_fds: [c_int; 2] = [-1, -1];

        unsafe fn close_pipe(pipe: &[c_int; 2]) {
            close(pipe[0]);
            close(pipe[1]);
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
                self.fds[0] = in_fds[1];
                self.fds[1] = out_fds[0];
                self.fds[2] = err_fds[0];
                self.pid = pid;
                return pid;
            }
        }
    }

    pub(crate) unsafe fn close(&self) -> c_int {
        let mut exit_status = -1;
        close(self.fds[0]);
        close(self.fds[1]);
        close(self.fds[2]);
        waitpid(self.pid, &mut exit_status, 0);
        return match WIFEXITED(exit_status) {
            true => WEXITSTATUS(exit_status),
            false => -1, // TODO: handle me
        };
    }

    pub(crate) unsafe fn stdout(&self) -> String {
        Process::read_from_fd(self.fds[1])
    }

    pub(crate) unsafe fn stderr(&self) -> String {
        Process::read_from_fd(self.fds[2])
    }

    unsafe fn read_from_fd(fd: c_int) -> String {
        let mut file = File::from_raw_fd(fd);
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap(); // TODO: handle this error
        buffer
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use super::Process;

    #[test]
    fn test_process_with_no_output() -> () {
        let mut process = Process::new();
        let command = CString::new("exit 23").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "".to_string());
            assert_eq!(process.stderr(), "".to_string());
            assert_eq!(process.close(), 23);
        }
    }

    #[test]
    fn test_process_with_simple_command() -> () {
        let mut process = Process::new();
        let command = CString::new("echo -n hi").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "hi".to_string());
            assert_eq!(process.stderr(), "".to_string());
            assert_eq!(process.close(), 0);
        }
    }

    #[test]
    fn test_process_with_stderr() -> () {
        let mut process = Process::new();
        let command = CString::new("echo -n hi >&2").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "".to_string());
            assert_eq!(process.stderr(), "hi".to_string());
            assert_eq!(process.close(), 0);
        }
    }

    #[test]
    fn test_process_with_both_stdout_and_stderr() -> () {
        let mut process = Process::new();
        let command = CString::new("echo -n hi && echo -n bye >&2").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "hi".to_string());
            assert_eq!(process.stderr(), "bye".to_string());
            assert_eq!(process.close(), 0);
        }
    }

    #[test]
    fn test_process_with_non_zero_return_code() -> () {
        let mut process = Process::new();
        let command = CString::new("echo -n hi; exit 4;").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "hi".to_string());
            assert_eq!(process.stderr(), "".to_string());
            assert_eq!(process.close(), 4);
        }
    }

    #[test]
    fn test_process_with_bash_command() -> () {
        let mut process = Process::new();
        let command =
            CString::new("/usr/bin/env bash -c 'echo -n hi; echo -n bye >&2 && exit 55;'").unwrap();
        unsafe {
            assert!(process.open(command) > 0);
            assert_eq!(process.stdout(), "hi".to_string());
            assert_eq!(process.stderr(), "bye".to_string());
            assert_eq!(process.close(), 55);
        }
    }
}
