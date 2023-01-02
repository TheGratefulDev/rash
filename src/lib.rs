#[macro_use]
extern crate lazy_static;

pub mod shell;

#[macro_export]
macro_rules! rash {
    () => (compile_error!("Expected only one argument to rash, received 0."));
    ($arg:tt) => {
        $crate::shell::command(($arg))
    };
    ($($arg:tt)*) => (compile_error!("Expected only one argument to rash, received more than 1."));
}

#[cfg(test)]
mod test {
    const COMMAND: &'static str = "echo -n hi";

    #[test]
    fn test_rash() {
        let (ret, stdout) = rash!("echo -n hi");
        assert_eq!(ret, 0);
        assert_eq!(stdout, "hi");

        let (ret, stdout) = rash!(COMMAND);
        assert_eq!(ret, 0);
        assert_eq!(stdout, "hi");

        let cmd = String::from("echo -n hi");
        let (ret, stdout) = rash!(cmd);
        assert_eq!(ret, 0);
        assert_eq!(stdout, "hi");
    }
}