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

    const SCRIPT: &'static str = r#"
        s="*"
        for i in {1..3}; do
            echo "$s"
            s="$s *"
        done;
        "#;

    #[test]
    fn test_read_me_examples() -> Result<(), RashError> {
        let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
        assert_eq!(ret_val, 0);
        assert_eq!(stdout, "Hello world!");

        let (ret_val, stdout) =
            rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
        assert_eq!(ret_val, 0);
        assert_eq!(stdout, "Hello world!");

        let (ret_val, stdout) = rash!(SCRIPT)?;
        assert_eq!(ret_val, 0);
        assert_eq!(stdout, "*\n* *\n* * *\n");
        Ok(())
    }
}
