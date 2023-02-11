//! **rsbash:** run bash commands from rust.
//!
//! Our macros [`rash!`](macro@rash) and [`rashf!`](macro@rashf) allow you to call out to a bash shell, just as you would typically from a terminal.
//! Since this is accomplished by interacting with libc, these macros can only be used on unix-like platforms (Linux, macOS etc).
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
//! let (ret_val, stdout, stderr) = rash!("echo 'Hello world!' | grep 'Hello'").unwrap();
//! assert_eq!(stdout, "Hello world!\n");
//! ```
//!
//! See the [`rash!`](macro@rash) and [`rashf!`](macro@rashf) macros, and the [`RashError`](enum@RashError) for more information.
#[macro_use]
extern crate lazy_static;

pub use crate::error::RashError;

mod command;
mod error;
mod process;
#[doc(hidden)]
pub mod shell;

/// Run a bash command.
///
/// #### Arguments:
/// `rash!` expects a single argument of a String or string literal (more specifically, any `AsRef<str>`).
///
/// #### Returns:
/// `rash!` returns a `Result<(i32, String, String), RashError>`.
///
/// The `(i32, String, String)` tuple contains the **return value**, the **stdout** and the **stderr** of the command, respectively.
///
/// See [`RashError`](enum@RashError) for more details of the error.
///
/// # Examples
///#### A simple command:
///```
/// use rsbash::{rash, RashError};
///
/// pub fn simple() -> Result<(), RashError> {
///     let (ret_val, stdout, stderr) = rash!("echo -n 'Hello world!'")?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, "Hello world!");
///     assert_eq!(stderr, "");
///     Ok(())
/// }
/// ```
///
/// #### A _less_ simple command:
/// ```
/// use rsbash::{rash, RashError};
///
/// pub fn less_simple() -> Result<(), RashError> {
///     let (ret_val, stdout, _) =
///         rash!("echo -n 'Hello ' | cat - && printf '%s' $(echo -n 'world!')")?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, "Hello world!");
///     Ok(())
/// }
/// ```
///
/// #### Using non-string literals:
///
///```
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
///     let (ret_val, stdout, stderr) = rash!(SCRIPT)?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, OUTPUT);
///     assert_eq!(stderr, "");
///
///     Ok(assert_eq!(rash!(String::from("echo hi >&2"))?, (0, "".to_string(), "hi".to_string())))
/// }
/// ```
///
/// # Compile errors
/// #### Passing a non-string literal as an argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_type() -> Result<(), RashError> {
///     let (ret_val, stdout, stderr) = rash!(35345)?;          // the trait `AsRef<str>` is not implemented for `{integer}`
///     Ok(())
/// }
/// ```
///
/// #### Passing either no arguments, or more than one argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_arg_count() -> Result<(), RashError> {
///     let (ret_val, stdout, stderr) = rash!()?;               // "missing tokens in macro arguments."
///     let (ret_val, stdout, stderr) = rash!("blah", "blah")?; // "no rules expected this token in macro call."
///     Ok(())
/// }
/// ```
///
#[cfg(unix)]
#[macro_export]
macro_rules! rash {
    ($arg:expr) => {
        $crate::shell::__command($arg)
    };
}

/// Format and run a bash command.
///
/// #### Arguments:
/// `rashf!` expects at least a single argument of a string literal representing the command to run.
/// Any further arguments should be formatting arguments to the command.
///
/// This syntax is the exact syntax of the well-known and well-loved `format!` macro, see [`std::fmt`](https://doc.rust-lang.org/stable/std/fmt/) for more details.
///
/// #### Returns:
/// `rashf!` returns a `Result<(i32, String, String), RashError>`.
///
/// The `(i32, String, String)` tuple contains the **return value**, the **stdout** and the **stderr** of the command, respectively.
///
/// See [`RashError`](enum@RashError) for more details of the error.
///
/// # Examples
///
/// #### Formatting:
///
/// Format `rashf!` commands just like you would [`format!`](https://doc.rust-lang.org/stable/std/fmt/) strings normally!
///
///```
/// use rsbash::{rashf, RashError};
///
/// pub fn simple_formatting() -> Result<(), RashError> {
///     let what = "Hello";
///     let who = "world!";
///     let (ret_val, stdout, stderr) = rashf!("echo -n '{} {}!'", what, who)?;
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, "Hello world!");
///     assert_eq!(stderr, "");
///     Ok(())
/// }
/// ```
///
/// ```
/// use rsbash::rashf;
/// use tempfile::TempDir;
///
/// const MESSAGE: &'static str = "Hi from within foo.sh!";
///
/// pub fn formatting() -> anyhow::Result<()> {
///     let dir = TempDir::new()?;
///     let path = dir.path().to_str().unwrap();
///     let (ret_val, stdout, stderr) = rashf!(
///        "cd {path}; echo -n \"echo -n '{msg}'\" > foo.sh; chmod u+x foo.sh; ./foo.sh;",
///        msg = MESSAGE
///     )?;
///
///     assert_eq!(ret_val, 0);
///     assert_eq!(stdout, MESSAGE);
///     assert_eq!(stderr, "");
///     Ok(())
/// }
/// ```
///
/// # Compile errors
/// #### Passing a non-string literal as an argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_type() -> Result<(), RashError> {
///     let (ret_val, stdout, stderr) = rash!(35345)?; // format argument must be a string literal
///     Ok(())
/// }
/// ```
///
/// #### Passing no arguments:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn no_args() -> Result<(), RashError> {
///     let (ret_val, stdout, stderr) = rash!()?;     // "requires at least a format string argument"
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
/// use rsbash::{rashf, RashError};
///
/// pub fn vulnerability() -> Result<(), RashError> {
///     let untrustworthy_user = "";                       // Suppose untrustworthy_user was set to "; reboot;"
///     let (ret_val, stdout, stderr) =                    // Uh oh! The command would have been formatted into
///         rashf!("echo -n Hello {untrustworthy_user}")?; // "echo -n Hello; reboot";
///     Ok(())
/// }
/// ```
///
/// Of course, best practices such as proper escaping, validating user input and so on would have circumvented
/// the above vulnerability. But, as a general rule only use formatted `rashf!` commands in situations
/// where you know for certain you can trust the inputs.
///
#[cfg(unix)]
#[macro_export]
macro_rules! rashf {
    ($($arg:tt)*) => {
        $crate::shell::__command(format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::RashError;

    const COMMAND: &'static str = "echo -n hi";

    lazy_static! {
        static ref EMPTY_STRING: String = String::default();
    }

    mod rash {
        use super::*;

        #[test]
        fn test_rash_with_a_single_string_literal() -> Result<(), RashError> {
            Ok(assert_eq!(rash!("echo -n hi")?, (0, "hi".to_string(), EMPTY_STRING.clone())))
        }

        #[test]
        fn test_rash_with_non_string_literals() -> Result<(), RashError> {
            let command = "echo -n hi".to_string();
            let expected = (0, "hi".to_string(), EMPTY_STRING.clone());

            assert_eq!(rash!(command)?, expected);
            assert_eq!(rash!(COMMAND)?, expected);
            Ok(())
        }

        #[test]
        fn test_rash_with_expressions() -> Result<(), RashError> {
            let message = "echo -n hi";
            let expected = (0, "hi".to_string(), EMPTY_STRING.clone());

            assert_eq!(rash!(message.to_string())?, expected);
            assert_eq!(rash!(format!("{message}"))?, expected);
            Ok(())
        }
    }

    mod rashf {
        use super::*;

        #[test]
        fn test_rashf_with_a_single_string_literal() -> Result<(), RashError> {
            Ok(assert_eq!(rashf!("echo -n hi")?, (0, "hi".to_string(), EMPTY_STRING.clone())))
        }

        #[test]
        fn test_rashf_with_formatted_non_string_literals() -> Result<(), RashError> {
            let command = "echo -n hi".to_string();
            let expected = (0, "hi".to_string(), EMPTY_STRING.clone());

            assert_eq!(rashf!("{}", command)?, expected);
            assert_eq!(rashf!("{}", COMMAND)?, expected);
            Ok(())
        }

        #[test]
        fn test_rashf_with_simple_formatting() -> Result<(), RashError> {
            let expected = (0, "hi bye".to_string(), EMPTY_STRING.clone());
            assert_eq!(rashf!("echo -n {} {}", "hi", "bye")?, expected);

            let hi = "hi";
            let bye = "bye".to_string();
            Ok(assert_eq!(rashf!("echo -n {} {}", hi, bye)?, expected))
        }

        #[test]
        fn test_rashf_with_variable_capture_formatting() -> Result<(), RashError> {
            let (one, two) = (1, 2);
            Ok(assert_eq!(
                rashf!("echo -n '{one} + {two}'")?,
                (0, "1 + 2".to_string(), EMPTY_STRING.clone())
            ))
        }

        #[test]
        fn test_rashf_with_positional_parameters() -> Result<(), RashError> {
            let (one, three) = (1, 3);
            Ok(assert_eq!(
                rashf!("echo -n '{one} + {two} + {three}'", two = 2)?,
                (0, "1 + 2 + 3".to_string(), EMPTY_STRING.clone())
            ))
        }
    }
}
