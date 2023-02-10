use std::ffi::{CString, NulError};

#[derive(Debug)]
pub(crate) struct BashCommand {
    command: CString,
}

impl BashCommand {
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self, NulError> {
        let quoted = BashCommand::quote(s.as_ref());
        Ok(Self {
            command: CString::new(BashCommand::format(quoted))?,
        })
    }

    pub fn command(&self) -> CString {
        self.command.clone()
    }

    fn format(s: String) -> String {
        format!("/usr/bin/env bash -c {}", s)
    }

    fn quote(s: &str) -> String {
        shell_words::quote(s).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::BashCommand;

    #[test]
    fn test_bash_command_formats_correctly() {
        let input = String::from("hi");
        let expected = String::from("/usr/bin/env bash -c hi");
        assert_eq!(BashCommand::format(input), expected);
    }

    #[test]
    fn test_bash_command_quotes_correctly() {
        assert_eq!(BashCommand::quote("hi"), "hi".to_string());

        let input = "\"\"'blah' \'blah\' 'blah'''";
        let expected = "'\"\"'\\''blah'\\'' '\\''blah'\\'' '\\''blah'\\'''\\'''\\'''";
        assert_eq!(BashCommand::quote(input), expected.to_string());
    }

    #[test]
    fn test_bash_command_formats_cstring_correctly() -> anyhow::Result<()> {
        let command = BashCommand::new("hello")?.command();
        Ok(assert_eq!(command.into_string()?, "/usr/bin/env bash -c hello".to_string()))
    }
}
