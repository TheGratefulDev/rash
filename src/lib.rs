#[macro_use]
extern crate lazy_static;

pub use crate::{
    error::RashError,
    shell::{command, Out},
};

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
