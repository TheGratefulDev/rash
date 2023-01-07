#[macro_use]
extern crate lazy_static;

pub use crate::{error::RashError, shell::command};

mod checked;
mod error;
mod shell;
mod utils;
mod wrapper;

#[cfg(unix)]
#[macro_export]
macro_rules! rash {
    ($arg:tt) => {
        $crate::shell::command(($arg))
    };
}

#[cfg(test)]
mod test {
    use crate::RashError;

    const COMMAND: &'static str = "echo -n hi";

    #[test]
    fn test_rash() -> Result<(), RashError> {
        let command = String::from("echo -n hi");
        let expected = (0, "hi".to_string());

        assert_eq!(rash!(command)?, expected);
        assert_eq!(rash!("echo -n hi")?, expected);
        assert_eq!(rash!(COMMAND)?, expected);
        Ok(())
    }

    #[cfg(test)]
    mod readme {
        use tempfile::TempDir;

        use crate::RashError;

        #[test]
        fn test_simple() -> Result<(), RashError> {
            let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
            assert_eq!(ret_val, 0);
            assert_eq!(stdout, "Hello world!");
            Ok(())
        }

        #[test]
        fn test_less_simple() -> Result<(), RashError> {
            let (ret_val, stdout) =
                rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
            assert_eq!(ret_val, 0);
            assert_eq!(stdout, "Hello world!");
            Ok(())
        }

        #[test]
        fn test_format() -> anyhow::Result<()> {
            let dir = TempDir::new()?;
            let path = dir.path().to_str().unwrap();
            let message = "Hi from within bar.txt!";
            let script = format!("cd {}; echo -n '{}' > bar.txt; cat bar.txt;", path, message);

            assert_eq!(rash!(script)?, (0, String::from(message)));

            dir.close()?;
            Ok(())
        }

        #[test]
        fn test_script() -> anyhow::Result<()> {
            let dir = TempDir::new()?;
            let path = dir.path().to_str().unwrap();
            let message = "Hi from within foo.sh!";
            let script = format!(
                "cd {}; echo -n \"echo -n '{}'\" > foo.sh; chmod u+x foo.sh; ./foo.sh;",
                path, message
            );

            assert_eq!(rash!(script)?, (0, String::from(message)));

            dir.close()?;
            Ok(())
        }

        #[test]
        fn test_raw_script() -> Result<(), RashError> {
            const SCRIPT: &'static str = r#"
            s="*"
            for i in {1..3}; do
                echo "$s"
                s="$s *"
            done;
            "#;

            let (ret_val, stdout) = rash!(SCRIPT)?;
            assert_eq!(ret_val, 0);
            assert_eq!(stdout, "*\n* *\n* * *\n");
            Ok(())
        }
    }
}
