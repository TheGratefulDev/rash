#[macro_use]
extern crate lazy_static;

pub mod shell;
pub mod wrapper;

#[macro_export]
macro_rules! rash {
    () => {
        compile_error!("Expected only 1 argument to rash, received 0.")
    };
    ($arg:tt) => {
        $crate::shell::command(($arg))
    };
    ($($arg:tt)*) => {
        compile_error!("Expected only 1 argument to rash, received more than 1.")
    };
}

/// rash! failure cases.
///
/// # Passing a non-String or string literal as an argument.
/// ```compile_fail
/// rash!(35345);          // the trait `AsRef<str>` is not implemented for `{integer}`
/// ```
///
/// # Passing 0 or more than 1 argument.
/// ```compile_fail
/// rash!();               // "Expected only 1 argument to rash, received 0."
/// rash!("blah", "blah"); // "Expected only 1 argument to rash, received more than 1."
/// ```
#[cfg(test)]
mod test {
    const COMMAND: &'static str = "echo -n hi";

    #[test]
    fn test_rash() {
        let command = String::from("echo -n hi");
        let expected = (0, "hi".to_string());

        assert_eq!(rash!(command).unwrap(), expected);
        assert_eq!(rash!("echo -n hi").unwrap(), expected);
        assert_eq!(rash!(COMMAND).unwrap(), expected);
    }
}
