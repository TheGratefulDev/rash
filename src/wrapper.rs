use libc::{c_char, c_int, FILE};
use std::ffi::CString;

lazy_static! {
    static ref READ_MODE: CString = CString::new("r").unwrap();
}

pub trait LibCWrapper {
    unsafe fn popen(&self, command: *const c_char) -> *mut FILE;
    unsafe fn fileno(&self, stream: *mut FILE) -> c_int;
    unsafe fn dup(&self, fd: c_int) -> c_int;
    unsafe fn pclose(&self, stream: *mut FILE) -> c_int;
    unsafe fn __errno_location(&self) -> *mut c_int;
    unsafe fn strerror(&self, n: c_int) -> *mut c_char;
}

pub struct LibCWrapperImpl {}

impl LibCWrapperImpl {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl LibCWrapper for LibCWrapperImpl {
    unsafe fn popen(&self, command: *const c_char) -> *mut FILE {
        libc::popen(command, READ_MODE.as_ptr())
    }

    unsafe fn fileno(&self, stream: *mut FILE) -> c_int {
        libc::fileno(stream)
    }

    unsafe fn dup(&self, fd: c_int) -> c_int {
        libc::dup(fd)
    }

    unsafe fn pclose(&self, stream: *mut FILE) -> c_int {
        libc::pclose(stream)
    }

    unsafe fn __errno_location(&self) -> *mut c_int {
        libc::__errno_location()
    }

    unsafe fn strerror(&self, n: c_int) -> *mut c_char {
        libc::strerror(n)
    }
}
