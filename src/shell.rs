use std::{fs::File, os::unix::io::FromRawFd, str};

use crate::{
    error::RashError,
    utils,
    wrapper::{CheckedLibCWrapper, CheckedLibCWrapperImpl, LibCWrapperImpl},
};

type Out = (i32, String);

#[cfg(unix)]
pub fn __command<S: AsRef<str>>(c: S) -> Result<Out, RashError> {
    run_command(c, &CheckedLibCWrapperImpl::new(LibCWrapperImpl {}))
}

fn run_command<S, W>(command: S, wrapper: &W) -> Result<Out, RashError>
where
    S: AsRef<str>,
    W: CheckedLibCWrapper,
{
    let command_as_c_string = utils::format_command_as_c_string(command)?;

    let run_process = move |wrapper: &W| -> Result<(File, i32), RashError> {
        let (stream, exit_status) = unsafe {
            let c_stream = wrapper.popen(command_as_c_string)?;
            let fd = wrapper.dup_fd(c_stream)?;
            let exit_status = wrapper.pclose(c_stream)?;
            let stream = File::from_raw_fd(fd);
            (stream, exit_status)
        };
        let return_code = wrapper.get_process_return_code(exit_status)?;
        Ok((stream, return_code))
    };

    let read_stdout = move |stream: File| -> Result<String, RashError> {
        return match str::from_utf8(&utils::read_file_into_buffer(stream)?) {
            Ok(s) => Ok(s.to_string()),
            Err(e) => Err(RashError::FailedToReadStdout {
                message: e.to_string(),
            }),
        };
    };

    let (stream, return_code) = run_process(wrapper)?;

    Ok((return_code, read_stdout(stream)?))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

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
            let (r, _) = __command(c).unwrap();
            assert_eq!(r, 0);
        });
    }

    #[test]
    fn test_commands_return_non_zero() {
        [("i_am_not_a_valid_executable", 127), ("echo hi | grep 'bye'", 1), ("exit 54;", 54)]
            .iter()
            .for_each(move |(c, ret)| {
                let (r, _) = __command(c).unwrap();
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
            let (_, s) = __command(c).unwrap();
            assert_eq!(s, *out);
        });
    }

    #[test]
    fn test_redirect_to_and_read_from_file() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_str().unwrap();

        let script = format!("cd {}; echo -n 'foo' > bar.txt; cat bar.txt;", path);

        assert_eq!(__command(script)?, (0, String::from("foo")));

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_run_script() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_str().unwrap();
        let message = "hi from within the script!";

        let script = format!(
            "cd {}; echo -n \"echo -n '{}'\" > bar.sh; chmod u+x bar.sh; ./bar.sh;",
            path, message
        );

        assert_eq!(__command(script)?, (0, String::from(message)));

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_raw_script() -> Result<(), RashError> {
        const PRETTY_TRIANGLE_SCRIPT: &str = r#"
        s="*"
        for i in {1..3}; do
            echo "$s"
            s="$s *"
        done;
        "#;

        assert_eq!(__command(PRETTY_TRIANGLE_SCRIPT)?, (0, String::from("*\n* *\n* * *\n")));
        Ok(())
    }

    #[test]
    fn test_quotes() -> Result<(), RashError> {
        assert_eq!(
            __command("echo -n 'a new line \n a day keeps the doctors away'")?,
            (0, String::from("a new line \n a day keeps the doctors away"))
        );
        assert_eq!(
            __command("\"\"echo -n 'blah' \'blah\' 'blah'''")?,
            (0, String::from("blah blah blah"))
        );
        assert_eq!(__command("echo hello world")?, (0, String::from("hello world\n")));
        Ok(())
    }

    #[test]
    fn test_comments() -> Result<(), RashError> {
        assert_eq!(__command("#echo 'i am silent'")?, (0, String::from("")));
        Ok(())
    }

    #[test]
    fn test_backslashes() -> Result<(), RashError> {
        assert_eq!(
            __command(
                "echo \
        -n \
        hi \
        there"
            )?,
            (0, String::from("hi there"))
        );
        Ok(())
    }
}
