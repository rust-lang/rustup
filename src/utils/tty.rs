// Copied from rustc. isatty crate did not work as expected.
#[cfg(unix)]
pub(crate) fn stderr_isatty() -> bool {
    isatty(libc::STDERR_FILENO)
}

#[cfg(windows)]
pub(crate) fn stderr_isatty() -> bool {
    isatty(winapi::um::winbase::STD_ERROR_HANDLE)
}

#[cfg(unix)]
pub(crate) fn stdout_isatty() -> bool {
    isatty(libc::STDOUT_FILENO)
}

#[cfg(windows)]
pub(crate) fn stdout_isatty() -> bool {
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
