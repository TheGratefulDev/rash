use std::str;

use crate::{command::BashCommand, error::RashError, process::Process};

type Out = (i32, String, String);

#[cfg(unix)]
pub fn __command<S: AsRef<str>>(c: S) -> Result<Out, RashError> {
    let command = BashCommand::new(c)?;
    let mut process = Process::new();
    Ok(unsafe {
        process.open(command)?;
        let ret = process.close()?;
        let stdout = process.stdout()?;
        let stderr = process.stderr()?;
        (ret, stdout, stderr)
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    lazy_static! {
        static ref EMPTY_STRING: String = String::default();
    }

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
            let (r, _, _) = __command(c).unwrap();
            assert_eq!(r, 0);
        });
    }

    #[test]
    fn test_commands_return_non_zero() {
        [("i_am_not_a_valid_executable", 127), ("echo hi | grep 'bye'", 1), ("exit 54;", 54)]
            .iter()
            .for_each(move |(c, ret)| {
                let (r, _, _) = __command(c).unwrap();
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
            default_assertions(__command(c).unwrap(), out);
        });
    }

    #[test]
    fn test_commands_stderr() -> Result<(), RashError> {
        Ok(assert_eq!(__command("echo -n 'hi' >&2")?, (0, EMPTY_STRING.clone(), "hi".to_string())))
    }

    #[test]
    fn test_combined_stdout() -> Result<(), RashError> {
        Ok(default_assertions(__command("echo -n hi; echo -n bye 2>&1;")?, "hibye"))
    }

    #[test]
    fn test_redirect_to_and_read_from_file() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_str().unwrap();
        let script = format!("cd {path}; echo -n 'foo' > bar.txt; cat bar.txt;");

        default_assertions(__command(script)?, "foo");

        Ok(temp_dir.close()?)
    }

    #[test]
    fn test_run_script() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().to_str().unwrap();
        let message = "hi from within the script!";
        let script = format!(
            "cd {path}; echo -n \"echo -n '{message}'\" > bar.sh; chmod u+x bar.sh; ./bar.sh;"
        );

        default_assertions(__command(script)?, message);

        Ok(temp_dir.close()?)
    }

    #[test]
    fn test_quotes() -> Result<(), RashError> {
        let message = "a new line \n a day keeps the doctors away";
        default_assertions(__command(format!("echo -n '{}'", message))?, message);

        let message = "\"\"'blah' \'blah\' 'blah'''";
        let expected = "blah blah blah";
        default_assertions(__command(format!("echo -n {}", message))?, expected);

        let message = "hello world";
        let expected = "hello world\n";
        default_assertions(__command(format!("echo {}", message))?, expected);
        Ok(())
    }

    #[test]
    fn test_comments() -> Result<(), RashError> {
        Ok(default_assertions(__command("#echo 'i am silent'")?, EMPTY_STRING.as_ref()))
    }

    #[test]
    fn test_backslashes() -> Result<(), RashError> {
        let c = "echo \
        -n \
        hi \
        there";

        Ok(default_assertions(__command(c)?, "hi there"))
    }

    fn default_assertions(o: Out, expected_stdout: &str) -> () {
        assert_eq!(o, (0, expected_stdout.to_string(), EMPTY_STRING.clone()))
    }
}
