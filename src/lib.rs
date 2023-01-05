#[macro_use]
extern crate lazy_static;

mod checked;
mod error;
pub mod shell;
mod utils;
mod wrapper;

#[cfg(unix)]
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
