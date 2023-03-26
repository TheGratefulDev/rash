use libc::{_exit, c_char, c_int, close, dup, execl, fork, pipe, waitpid, WEXITSTATUS, WIFEXITED};
use std::{
    ffi::CString,
    fs::File,
    io::Read,
    os::unix::io::FromRawFd,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};
use thiserror::Error;

use crate::command::BashCommand;

lazy_static! {
    static ref SHELL_PATH: CString = CString::new("/bin/sh").expect("/bin/sh CString failed.");
    static ref SH: CString = CString::new("sh").expect("sh CString failed.");
    static ref COMMAND: CString = CString::new("-c").expect("-c CString failed.");
}

pub(crate) struct Process {
    fds: [c_int; 3],
    pid: c_int,
    stdout: String,
    stderr: String,
    stdout_handle: Option<JoinHandle<String>>,
    stderr_handle: Option<JoinHandle<String>>,
    stdout_pair: Arc<(Mutex<bool>, Condvar)>,
    stderr_pair: Arc<(Mutex<bool>, Condvar)>,
}

#[derive(Error, Debug, PartialEq)]
pub(crate) enum ProcessError {
    #[error("Couldn't fork.")]
    CouldNotFork,
    #[error("Couldn't create pipe.")]
    CouldNotCreatePipe,
    #[error("Couldn't dup fd {0}")]
    CouldNotDupFd(c_int),
    #[error("process::open didn't close normally - WIFEXITED was false.")]
    OpenDidNotCloseNormally,
    #[error("Couldn't get stderr.")]
    CouldNotGetStderr,
    #[error("Couldn't get stdout.")]
    CouldNotGetStdout,
}

impl Process {
    pub(crate) fn new() -> Self {
        Self {
            fds: [-1, -1, -1],
            pid: -1,
            stdout: String::default(),
            stderr: String::default(),
            stdout_handle: None,
            stderr_handle: None,
            stdout_pair: Arc::new((Mutex::new(false), Condvar::new())),
            stderr_pair: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    pub(crate) unsafe fn open(&mut self, command: BashCommand) -> Result<(), ProcessError> {
        let mut in_fds: [c_int; 2] = [-1, -1];
        let mut out_fds: [c_int; 2] = [-1, -1];
        let mut err_fds: [c_int; 2] = [-1, -1];

        unsafe fn close_pipe(pipe: &[c_int; 2]) {
            close(pipe[0]);
            close(pipe[1]);
        }

        self.pipe(&mut in_fds, || {})?;

        self.pipe(&mut out_fds, || {
            close_pipe(&in_fds);
        })?;

        self.pipe(&mut err_fds, || {
            close_pipe(&out_fds);
            close_pipe(&in_fds);
        })?;

        match fork() {
            -1 => {
                close_pipe(&err_fds);
                close_pipe(&out_fds);
                close_pipe(&in_fds);
                Err(ProcessError::CouldNotFork)
            }
            0 => {
                close(in_fds[1]);
                close(out_fds[0]);
                close(err_fds[0]);

                close(0);
                self.dup(in_fds[0])?;

                close(1);
                self.dup(out_fds[1])?;

                close(2);
                self.dup(err_fds[1])?;

                execl(
                    SHELL_PATH.as_ptr(),
                    SH.as_ptr(),
                    COMMAND.as_ptr(),
                    command.command().as_ptr(),
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

                let mut stdout_file = File::from_raw_fd(self.fds[1]);
                let stdout_pair = self.stdout_pair.clone();
                self.stdout_handle = Some(std::thread::spawn(move || {
                    let mut stdout = String::default();
                    let &(ref lock, ref cvar) = &*stdout_pair;
                    loop {
                        stdout_file.read_to_string(&mut stdout).unwrap();
                        let mut stop = lock.lock().unwrap();
                        let result = cvar.wait_timeout(stop, Duration::from_millis(25)).unwrap();
                        stop = result.0;
                        if *stop == true {
                            break;
                        }
                    }
                    stdout
                }));

                let mut stderr_file = File::from_raw_fd(self.fds[2]);
                let stderr_pair = self.stderr_pair.clone();
                self.stderr_handle = Some(std::thread::spawn(move || {
                    let mut stderr = String::default();
                    let &(ref lock, ref cvar) = &*stderr_pair;
                    loop {
                        stderr_file.read_to_string(&mut stderr).unwrap();
                        let mut stop = lock.lock().unwrap();
                        let result = cvar.wait_timeout(stop, Duration::from_millis(25)).unwrap();
                        stop = result.0;
                        if *stop == true {
                            break;
                        }
                    }
                    stderr
                }));
                Ok(())
            }
        }
    }

    pub(crate) unsafe fn close(&mut self) -> Result<c_int, ProcessError> {
        close(self.fds[0]);
        let mut status = -1;
        waitpid(self.pid, &mut status, 0);

        let &(ref lock, ref cvar) = &*self.stdout_pair;
        {
            let mut stop = lock.lock().unwrap();
            *stop = true;
            cvar.notify_one();
        }

        let &(ref elock, ref ecvar) = &*self.stderr_pair;
        {
            let mut estop = elock.lock().unwrap();
            *estop = true;
            ecvar.notify_one();
        }

        self.stdout = self
            .stdout_handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| ProcessError::CouldNotGetStdout)?;
        self.stderr = self
            .stderr_handle
            .take()
            .unwrap()
            .join()
            .map_err(|_| ProcessError::CouldNotGetStderr)?;

        return match WIFEXITED(status) {
            true => Ok(WEXITSTATUS(status)),
            false => Err(ProcessError::OpenDidNotCloseNormally),
        };
    }

    pub(crate) fn stdout(&self) -> Result<String, ProcessError> {
        Ok(self.stdout.clone())
    }

    pub(crate) fn stderr(&self) -> Result<String, ProcessError> {
        Ok(self.stderr.clone())
    }

    unsafe fn dup(&self, fd: c_int) -> Result<(), ProcessError> {
        match dup(fd) {
            -1 => Err(ProcessError::CouldNotDupFd(fd)),
            _ => Ok(()),
        }
    }

    unsafe fn pipe(
        &self,
        fds: &mut [c_int; 2],
        on_error: impl FnOnce() -> (),
    ) -> Result<(), ProcessError> {
        match pipe(fds.as_mut_ptr()) {
            -1 => {
                on_error();
                Err(ProcessError::CouldNotCreatePipe)
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BashCommand, Process, ProcessError};

    #[test]
    fn test_process_with_no_output() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("exit 23")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 23);
            assert_eq!(process.stdout()?, "".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_simple_command() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "hi".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_stderr() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "".to_string());
            assert_eq!(process.stderr()?, "hi".to_string());
        })
    }

    #[test]
    fn test_process_with_both_stdout_and_stderr() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi && echo -n bye >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "hi".to_string());
            assert_eq!(process.stderr()?, "bye".to_string());
        })
    }

    #[test]
    fn test_process_only_read_stdout() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi && echo -n bye >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "hi".to_string());
        })
    }

    #[test]
    fn test_process_only_read_stderr() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi && echo -n bye >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stderr()?, "bye".to_string());
        })
    }

    #[test]
    fn test_process_dont_read_either_stdout_or_stderr() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi && echo -n bye >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
        })
    }

    #[test]
    fn test_process_with_non_zero_return_code() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi; exit 4;")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 4);
            assert_eq!(process.stdout()?, "hi".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_inner_bash_command() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command =
            BashCommand::new("/usr/bin/env bash -c 'echo -n hi; echo -n bye >&2 && exit 55;'")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 55);
            assert_eq!(process.stdout()?, "hi".to_string());
            assert_eq!(process.stderr()?, "bye".to_string());
        })
    }

    #[test]
    fn test_process_with_multiline_command() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new(MULTILINE)?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 2);
            assert_eq!(process.stdout()?, "hibye".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_multiline_bash_command() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new(format!("bash -c '{MULTILINE}'"))?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 2);
            assert_eq!(process.stdout()?, "hibye".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_long_running_command() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi; sleep 2; echo -n bye >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "hi".to_string());
            assert_eq!(process.stderr()?, "bye".to_string());
        })
    }

    #[test]
    fn test_process_with_stdin_larger_than_64kb() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("head -c 65537 /dev/zero | cat > /dev/null")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "".to_string());
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_stdout_larger_than_64kb() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("head -c 65537 /dev/zero")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?.len(), 65537);
            assert_eq!(process.stderr()?, "".to_string());
        })
    }

    #[test]
    fn test_process_with_stderr_larger_than_64kb() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("head -c 65537 /dev/zero >&2")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.close()?, 0);
            assert_eq!(process.stdout()?, "".to_string());
            assert_eq!(process.stderr()?.len(), 65537);
        })
    }

    const MULTILINE: &'static str = r#"
            echo -n hi && \
            echo -n bye && \
            exit 2
            "#;
}
