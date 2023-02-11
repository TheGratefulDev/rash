use std::{ffi::CString, fs::File, io::Read, os::fd::FromRawFd};

use libc::{_exit, c_char, c_int, close, dup, execl, fork, pipe, waitpid, WEXITSTATUS, WIFEXITED};
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
    closed: bool,
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
    #[error("Tried to read stdout before the process closed.")]
    StdoutReadPrematurely,
    #[error("Tried to read stderr before the process closed.")]
    StderrReadPrematurely,
}

impl Process {
    pub(crate) fn new() -> Self {
        Self {
            fds: [-1, -1, -1],
            pid: -1,
            stdout: "".to_string(),
            stderr: "".to_string(),
            closed: false,
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
                Ok(())
            }
        }
    }

    pub(crate) unsafe fn close(&mut self) -> Result<c_int, ProcessError> {
        close(self.fds[0]);
        let stdout = Self::read_from_fd(self.fds[1]).map_err(|_| ProcessError::CouldNotGetStdout);
        let stderr = Self::read_from_fd(self.fds[2]).map_err(|_| ProcessError::CouldNotGetStderr);
        self.closed = true;

        let mut exit_status = -1;
        waitpid(self.pid, &mut exit_status, 0);
        return match WIFEXITED(exit_status) {
            true => {
                if stdout.is_err() {
                    return Err(stdout.unwrap_err());
                }
                self.stdout = stdout.unwrap();
                if stderr.is_err() {
                    return Err(stderr.unwrap_err());
                }
                self.stderr = stderr.unwrap();
                Ok(WEXITSTATUS(exit_status))
            }
            false => Err(ProcessError::OpenDidNotCloseNormally),
        };
    }

    pub(crate) fn stdout(&self) -> Result<String, ProcessError> {
        if !self.closed {
            return Err(ProcessError::StdoutReadPrematurely);
        }
        Ok(self.stdout.clone())
    }

    pub(crate) fn stderr(&self) -> Result<String, ProcessError> {
        if !self.closed {
            return Err(ProcessError::StderrReadPrematurely);
        }
        Ok(self.stderr.clone())
    }

    unsafe fn read_from_fd(fd: c_int) -> anyhow::Result<String> {
        let mut file = File::from_raw_fd(fd);
        let mut buffer = String::new();
        file.read_to_string(&mut buffer)?;
        Ok(buffer)
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
    fn test_process_with_premature_read() -> anyhow::Result<()> {
        let mut process = Process::new();
        let command = BashCommand::new("echo -n hi")?;
        Ok(unsafe {
            assert!(process.open(command).is_ok());
            assert_eq!(process.stdout(), Err(ProcessError::StdoutReadPrematurely {}));
            assert_eq!(process.stderr(), Err(ProcessError::StderrReadPrematurely {}));
            assert_eq!(process.close()?, 0);
        })
    }
}
