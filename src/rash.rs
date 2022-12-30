use std::ffi::CString;

pub fn command<S: AsRef<str>>(cmd: S) -> u32 {
    unsafe { libc::system(into_c_string(into_bash_command(cmd)).as_ptr()) as u32 }
}

fn into_bash_command<S: AsRef<str>>(s: S) -> String {
    format!("/bin/bash -c \"{}\"", s.as_ref())
}

fn into_c_string(s: String) -> CString {
    CString::new(s.as_str()).expect(format!("Couldn't convert {} into CString.", s).as_ref())
}

mod tests {
    use super::command;

    #[test]
    fn test_simple() {
        assert_eq!(command("ls"), 0);
    }

    #[test]
    fn test_pipe() {
        assert_eq!(command("ls -l | cat -"), 0);
    }

    #[test]
    fn test_pipe_with_subshell() {
        assert_eq!(command("ls | cat $(echo '-')"), 0);
    }

    #[test]
    fn test_conditional() {
        assert_eq!(command("[[ 5 -eq $((3 + 2)) ]]"), 0);
    }
}
