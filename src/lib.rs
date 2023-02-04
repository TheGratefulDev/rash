//! **rsbash:** run bash commands from rust.
//!
//! Our macro [`rash!`](macro@rash) allows you to call out to a bash shell, just as you would typically from a terminal.
//! Since this is accomplished by interacting with libc, [`rash!`](macro@rash) can only be used on unix-like platforms (Linux, macOS etc).
//!
//! ## Motivation
//!
//! Making a shell command with the native [`std::process::Command`](https://doc.rust-lang.org/std/process/struct.Command.html) builder is _quite_ involved.
//!
//! Suppose you wanted to write "Hello world!" to stdout.
//!```
//! use std::io::Write;
//! use std::process::Command;
//!
//! let command = Command::new("echo")
//!               .arg("Hello world!")
//!               .output()
//!               .expect("Uh oh, couldn't say hello!");
//! std::io::stdout().write_all(&command.stdout).unwrap();
//!
//! assert_eq!(std::str::from_utf8(&command.stdout).unwrap(), "Hello world!\n");
//! ```
//!
//! Now suppose you wanted to pipe the output to a second command, and then write the result to stdout:
//! ```
//! use std::process::{Command, Stdio};
//! use std::io::Write;
//!
//! let echo = Command::new("echo")
//!            .arg("Hello world!")
//! 		   .stdout(Stdio::piped())
//! 		   .spawn()
//! 		   .expect("Uh oh, couldn't say hello!");
//! 					   
//! let grep = Command::new("grep")
//!            .arg("Hello")
//!            .stdin(Stdio::from(echo.stdout.unwrap()))
//!            .output()
//!            .expect("Uh oh, couldn't grep for Hello!");
//!     
//! std::io::stdout().write_all(&grep.stdout).unwrap();
//!
//! assert_eq!(std::str::from_utf8(&grep.stdout).unwrap(), "Hello world!\n");
//! ```
//!
//! With [`rash!`](macro@rash) the same command is as simple as:
//!
//!```
//! use rsbash::rash;
//!
//! let (ret_val, stdout) = rash!("echo 'Hello world!' | grep 'Hello'").unwrap();
//! assert_eq!(stdout, "Hello world!\n");
//! ```
//!
//! See the [`rash!`](macro@rash) macro and [`RashError`](enum@RashError) for more information.
#[macro_use]
extern crate lazy_static;

pub use crate::error::RashError;

mod error;
#[doc(hidden)]
pub mod shell;
mod utils;
mod wrapper;

#[cfg(unix)]
/// Run a bash command.
///
/// #### Arguments:
/// `rash!` expects at least a single argument of a string literal. Any further arguments should be formatting arguments.
///
/// `rash!` supports the exact syntax of the well-known and well-loved `format!` macro, see [`std::fmt`](https://doc.rust-lang.org/stable/std/fmt/) for more details.
///
/// #### Returns:
/// `rash!` returns a `Result<(i32, String), RashError>`.
///
/// The `(i32, String)` tuple contains the **return value** and the **stdout** of the command, respectively.
///
/// See [`RashError`](enum@RashError) for more details of the error.
///
/// # Examples
///#### A simple command:
///```
/// use rsbash::{rash, RashError};
///
/// pub fn simple() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, "Hello world!");
///     Ok(())
/// }
/// ```
///
/// #### A _less_ simple command:
/// ```
/// use rsbash::{rash, RashError};
///
/// pub fn less_simple() -> Result<(), RashError> {
///     let (ret_val, stdout) =
///         rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, "Hello world!");
///     Ok(())
/// }
/// ```
/// #### Using a formatted command:
///
/// Format `rash!` commands just like you would [`format!`](https://doc.rust-lang.org/stable/std/fmt/) strings normally!
///
/// ```
/// use rsbash::rash;
/// use tempfile::TempDir;
///
/// const MESSAGE: &'static str = "Hi from within foo.sh!";
///
/// pub fn with_formatting() -> anyhow::Result<()> {
///     let dir = TempDir::new()?;
///     let path = dir.path().to_str().unwrap();
///     let (ret_val, stdout) = rash!(
///        "cd {path}; echo -n \"echo -n '{msg}'\" > foo.sh; chmod u+x foo.sh; ./foo.sh;",
///        msg = MESSAGE
///     )?;
///
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, MESSAGE);
///     Ok(())
/// }
/// ```
///
/// #### Using non-string literals:
///
/// Similarly, use `rash!` with non-string literals just as you would [`format!`](https://doc.rust-lang.org/stable/std/fmt/).
///
/// ```
/// use rsbash::{rash, RashError};
///
/// const SCRIPT: &'static str = r#"
/// s="*"
/// for i in {1..3}; do
///     echo "$s"
///     s="$s *"
/// done;
/// "#;
///
/// const OUTPUT: &'static str = r#"\
/// *
/// * *
/// * * *"#;
///
/// pub fn non_string_literals() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!("{}", SCRIPT)?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, OUTPUT);   
///  
///     let string = String::from("echo -n 'Be sure to format me appropriately!'");
///     Ok(assert_eq!(rash!("{}", string)?, (0, "Be sure to format me appropriately!".to_string())))
/// }
/// ```
///
/// # Compile errors
/// #### Passing a non string literal as an argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_type() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!(35345)?;          // format argument must be a string literal
///     Ok(())
/// }
/// ```
///
/// #### Passing either no arguments, or more than one argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn no_args() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!()?;               // "requires at least a format string argument"
///     Ok(())
/// }
/// ```
///
/// # A word on security
/// Sometimes the ease and flexibility of bash is exactly what you're after.
/// But, with great power comes great responsibility, and so I'd be remiss if I wasn't to mention
/// that formatting bash commands in this manner exposes a vulnerability in the form of a SQL injection-like attack:
///
/// ```
/// use rsbash::{rash, RashError};
///
/// pub fn vulnerability() -> Result<(), RashError> {
///     let untrustworthy_user = "";                       // Suppose untrustworthy_user was set to "; reboot;"
///     let (ret_val, stdout) =                            // Uh oh! The command would have been formatted into
///         rash!("echo -n Hello {untrustworthy_user}")?;  // "echo -n Hello; reboot";
///     Ok(())
/// }
/// ```
///
/// Of course, best practices such as proper escaping, validating user input and so on would have circumvented
/// the above vulnerability. But, as a general rule only use formatted `rash!` commands in situations
/// where you know for certain you can trust the inputs.
///
#[macro_export]
macro_rules! rash {
    ($($arg:tt)*) => {
        $crate::shell::command(format!($($arg)*))
    };
}

#[cfg(test)]
mod test {
    use crate::RashError;

    const COMMAND: &'static str = "echo -n hi";

    #[test]
    fn test_rash_with_a_single_string_literal() -> Result<(), RashError> {
        Ok(assert_eq!(rash!("echo -n hi")?, (0, "hi".to_string())))
    }

    #[test]
    fn test_rash_with_non_string_literals() -> Result<(), RashError> {
        let command = "echo -n hi".to_string();
        let expected = (0, "hi".to_string());

        assert_eq!(rash!("{}", command)?, expected);
        assert_eq!(rash!("{}", COMMAND)?, expected);
        Ok(())
    }

    #[test]
    fn test_rash_with_simple_formatting() -> Result<(), RashError> {
        let expected = (0, "hi bye".to_string());
        assert_eq!(rash!("echo -n {} {}", "hi", "bye")?, expected);

        let hi = "hi";
        let bye = "bye".to_string();
        Ok(assert_eq!(rash!("echo -n {} {}", hi, bye)?, expected))
    }

    #[test]
    fn test_rash_with_variable_capture_formatting() -> Result<(), RashError> {
        let (one, two) = (1, 2);
        Ok(assert_eq!(rash!("echo -n '{one} + {two}'")?, (0, "1 + 2".to_string())))
    }

    #[test]
    fn test_rash_with_positional_parameters() -> Result<(), RashError> {
        let (one, three) = (1, 3);
        Ok(assert_eq!(
            rash!("echo -n '{one} + {two} + {three}'", two = 2)?,
            (0, "1 + 2 + 3".to_string())
        ))
    }
}
