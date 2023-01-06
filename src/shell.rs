use std::{fs, os::unix::io::FromRawFd, str};

use crate::{
    checked,
    error::RashError,
    utils,
    wrapper::{LibCWrapper, LibCWrapperImpl},
};

#[cfg(unix)]
pub type Out = (i32, String);

#[cfg(unix)]
pub fn command<S: AsRef<str>>(c: S) -> Result<Out, RashError> {
    run_command(c, &LibCWrapperImpl::new())
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

    let return_code = checked::get_process_return_code(exit_status, delegate)?;

    return match str::from_utf8(&utils::read_file_into_buffer(stream)?) {
        Ok(s) => Ok((return_code, s.to_string())),
        Err(e) => Err(RashError::FailedToReadStdout {
            message: e.to_string(),
        }),
    };
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
    fn test_redirect_to_and_read_from_file() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_str().unwrap();

        let script = format!("cd {}; echo -n 'foo' > bar.txt; cat bar.txt;", path);

        assert_eq!(command(script)?, (0, String::from("foo")));

        temp_dir.close()?;
        Ok(())
    }

    #[test]
    fn test_script() -> Result<(), RashError> {
        assert_eq!(command(PRETTY_TRIANGLE_SCRIPT)?, (0, String::from("*\n* *\n* * *\n")));
        Ok(())
    }

    const PRETTY_TRIANGLE_SCRIPT: &str = r#"
        s="*"
        for i in {1..3}; do
            echo "$s"
            s="$s *"
        done;
        "#;

    #[test]
    fn test_quotes() -> Result<(), RashError> {
        assert_eq!(
            command("echo -n 'a new line \n a day keeps the doctors away'")?,
            (0, String::from("a new line \n a day keeps the doctors away"))
        );
        assert_eq!(
            command("\"\"echo -n 'blah' \'blah\' 'blah'''")?,
            (0, String::from("blah blah blah"))
        );
        assert_eq!(command("echo hello world")?, (0, String::from("hello world\n")));
        Ok(())
    }

    #[test]
    fn test_comments() -> Result<(), RashError> {
        assert_eq!(command("#echo 'i am silent'")?, (0, String::from("")));
        Ok(())
    }

    #[test]
    fn test_backslashes() -> Result<(), RashError> {
        assert_eq!(
            command(
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
