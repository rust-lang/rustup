// FIXME: stolen from cargo. Should be extracted into a common crate.

//! Job management (mostly for Windows)
//!
//! Most of the time when you're running cargo you expect Ctrl-C to actually
//! terminate the entire tree of processes in play, not just the one at the top
//! (cargo). This currently works "by default" on Unix platforms because Ctrl-C
//! actually sends a signal to the *process group* rather than the parent
//! process, so everything will get torn down. On Windows, however, this does
//! not happen and Ctrl-C just kills cargo.
//!
//! To achieve the same semantics on Windows we use Job Objects to ensure that
//! all processes die at the same time. Job objects have a mode of operation
//! where when all handles to the object are closed it causes all child
//! processes associated with the object to be terminated immediately.
//! Conveniently whenever a process in the job object spawns a new process the
//! child will be associated with the job object as well. This means if we add
//! ourselves to the job object we create then everything will get torn down!

#![allow(clippy::missing_safety_doc)]

pub(crate) use self::imp::Setup;

pub(crate) fn setup() -> Option<Setup> {
    unsafe { imp::setup() }
}

#[cfg(unix)]
mod imp {
    pub(crate) type Setup = ();

    pub(crate) unsafe fn setup() -> Option<()> {
        Some(())
    }
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::mem;
    use std::ptr;

    use tracing::info;
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::*;

    pub(crate) struct Setup {
        job: Handle,
    }

    pub(crate) struct Handle {
        inner: HANDLE,
    }

    fn last_err() -> io::Error {
        io::Error::last_os_error()
    }

    pub(crate) unsafe fn setup() -> Option<Setup> {
        // Creates a new job object for us to use and then adds ourselves to it.
        // Note that all errors are basically ignored in this function,
        // intentionally. Job objects are "relatively new" in Windows,
        // particularly the ability to support nested job objects. Older
        // Windows installs don't support this ability. We probably don't want
        // to force Cargo to abort in this situation or force others to *not*
        // use job objects, so we instead just ignore errors and assume that
        // we're otherwise part of someone else's job object in this case.

        let job = CreateJobObjectW(ptr::null_mut(), ptr::null());
        if job.is_null() {
            return None;
        }
        let job = Handle { inner: job };

        // Indicate that when all handles to the job object are gone that all
        // process in the object should be killed. Note that this includes our
        // entire process tree by default because we've added ourselves and
        // our children will reside in the job once we spawn a process.
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
        info = mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let r = SetInformationJobObject(
            job.inner,
            JobObjectExtendedLimitInformation,
            &mut info as *mut _ as *const std::ffi::c_void,
            mem::size_of_val(&info) as u32,
        );
        if r == 0 {
            return None;
        }

        // Assign our process to this job object, meaning that our children will
        // now live or die based on our existence.
        let me = GetCurrentProcess();
        let r = AssignProcessToJobObject(job.inner, me);
        if r == 0 {
            return None;
        }

        Some(Setup { job })
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            // On normal exits (not ctrl-c), we don't want to kill any child
            // processes. The destructor here configures our job object to
            // *not* kill everything on close, then closes the job object.
            unsafe {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
                info = mem::zeroed();
                let r = SetInformationJobObject(
                    self.job.inner,
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as *const std::ffi::c_void,
                    mem::size_of_val(&info) as u32,
                );
                if r == 0 {
                    info!("failed to configure job object to defaults: {}", last_err());
                }
            }
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.inner);
            }
        }
    }
}
