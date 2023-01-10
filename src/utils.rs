use std::{ffi::CString, fs, io::Read};

use crate::error::RashError;

pub(crate) fn format_command_as_c_string<S: AsRef<str>>(cmd: S) -> Result<CString, RashError> {
    return CString::new(into_bash_command(cmd)).map_err(|e| RashError::NullByteInCommand {
        message: e.to_string(),
    });
}

pub(crate) fn into_bash_command<S: AsRef<str>>(s: S) -> String {
    format!("/usr/bin/env bash -c {}", shell_words::quote(s.as_ref()).to_string())
}

pub(crate) fn read_file_into_buffer(mut stream: fs::File) -> Result<Vec<u8>, RashError> {
    let mut buffer = Vec::new();
    match stream.read_to_end(&mut buffer) {
        Ok(_) => Ok(buffer),
        Err(e) => Err(RashError::FailedToReadStdout {
            message: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_command_as_c_string_with_null_byte_returns_error() {
        let command_with_null_byte =
            unsafe { std::str::from_utf8_unchecked([5, 6, 7, 8, 0, 10, 11, 12].as_ref()) };

        let result = format_command_as_c_string(command_with_null_byte);

        assert!(result.is_err());
        assert_eq!(
            result,
            Err(RashError::NullByteInCommand {
                message: "nul byte found in provided data at position: 26".to_string()
            })
        );
        assert_eq!(
            result.unwrap_err().to_string(),
            "Null byte in command: \"nul byte found in provided data at position: 26\""
        );
    }

    #[test]
    fn test_into_bash_command() {
        assert_eq!(into_bash_command("blah"), "/usr/bin/env bash -c blah");
    }
}
