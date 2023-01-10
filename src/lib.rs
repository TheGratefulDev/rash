//! **rsbash:** run bash commands from rust. Simply.
//!
//! ## How simply?
//!```
//! use rsbash::{rash, RashError};
//!
//! pub fn simple() -> Result<(), RashError> {
//!     let (ret_val, stdout) = rash!("echo -n 'Hello world!'")?;
//!     assert_eq!(ret_val, 0);
//!     assert_eq!(stdout, "Hello world!");
//!     Ok(())
//! }
//! ```
//!
//! See the [`rash!`](macro@rash) macro and [`RashError`](enum@RashError) error for more information.
//!

#[macro_use]
extern crate lazy_static;

pub use crate::error::RashError;

mod error;
#[doc(hidden)]
pub mod shell;
mod utils;
mod wrapper;

/// Run a bash command.
///
/// #### Argument:
/// `rash!` expects a **single** argument of a String or string literal (more specifically, any `AsRef<str>`).
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
/// #### With a format!'d string:
///
/// ```
/// use rsbash::rash;
/// use tempfile::TempDir;
///
/// pub fn format() -> anyhow::Result<()> {
///     let dir = TempDir::new()?;
///     let path = dir.path().to_str().unwrap();
///     let message = "Hi from within bar.txt!";
///     let script = format!("cd {}; echo -n '{}' > bar.txt; cat bar.txt;", path, message);
///
///     assert_eq!(rash!(script)?, (0, String::from(message)));
///
///     dir.close()?;
///     Ok(())
/// }
/// ```
/// #### Execute a script:
/// ```
/// use rsbash::rash;
/// use tempfile::TempDir;
///
/// pub fn script() -> anyhow::Result<()> {
///     let dir = TempDir::new()?;
///     let path = dir.path().to_str().unwrap();
///     let message = "Hi from within foo.sh!";
///     let script = format!(
///        "cd {}; echo -n \"echo -n '{}'\" > foo.sh; chmod u+x foo.sh; ./foo.sh;",
///        path, message
///     );
///
///     assert_eq!(rash!(script)?, (0, String::from(message)));
///
///     dir.close()?;
///     Ok(())
/// }
/// ```
///
/// #### Using a raw string script:
///
/// ```
/// use rsbash::{rash, RashError};
///
/// const SCRIPT: &'static str = r#"
///     s="*"
///     for i in {1..3}; do
///         echo "$s"
///         s="$s *"
///     done;
///     "#;
///
/// pub fn raw_script() -> Result<(), RashError> {  // ... it prints a lovely triangle.
///     let (ret_val, stdout) = rash!(SCRIPT)?;     // *
///     assert_eq!(ret_val, 0);                     // * *
///     assert_eq!(stdout, "*\n* *\n* * *\n");      // * * *   
///     Ok(())
/// }
/// ```
///
/// # Compile errors.
/// #### Passing a non String or string literal as an argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_type() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!(35345)?;          // the trait `AsRef<str>` is not implemented for `{integer}`
///     Ok(())
/// }
/// ```
///
/// #### Passing either no arguments, or more than one argument:
/// ```compile_fail
/// use rsbash::{rash, RashError};
///
/// pub fn wrong_arg_count() -> Result<(), RashError> {
///     let (ret_val, stdout) = rash!()?;               // "missing tokens in macro arguments."
///     let (ret_val, stdout) = rash!("blah", "blah")?; // "no rules expected this token in macro call."
///     Ok(())
/// }
/// ```
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
    fn test_rash_with_various_arg_types() -> Result<(), RashError> {
        let command = String::from("echo -n hi");
        let expected = (0, "hi".to_string());

        assert_eq!(rash!("echo -n hi")?, expected);
        assert_eq!(rash!(command)?, expected);
        assert_eq!(rash!(COMMAND)?, expected);
        Ok(())
    }
}
