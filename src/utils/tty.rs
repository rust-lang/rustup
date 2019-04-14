// Copied from rustc. atty crate did not work as expected
#[cfg(unix)]
pub fn stderr_isatty() -> bool {
    isatty(libc::STDERR_FILENO)
}

// FIXME: Unfortunately this doesn't detect msys terminals so rustup
// is always colorless there (just like rustc and cargo).
#[cfg(windows)]
pub fn stderr_isatty() -> bool {
    isatty(winapi::um::winbase::STD_ERROR_HANDLE)
}

#[cfg(unix)]
pub fn stdout_isatty() -> bool {
    isatty(libc::STDOUT_FILENO)
}

#[cfg(windows)]
pub fn stdout_isatty() -> bool {
    isatty(winapi::um::winbase::STD_OUTPUT_HANDLE)
}

#[inline]
#[cfg(unix)]
fn isatty(fd: libc::c_int) -> bool {
    unsafe { libc::isatty(fd) == 1 }
}

#[inline]
#[cfg(windows)]
fn isatty(fd: winapi::shared::minwindef::DWORD) -> bool {
    use winapi::um::{consoleapi::GetConsoleMode, processenv::GetStdHandle};
    unsafe {
        let handle = GetStdHandle(fd);
        let mut out = 0;
        GetConsoleMode(handle, &mut out) != 0
    }
}
