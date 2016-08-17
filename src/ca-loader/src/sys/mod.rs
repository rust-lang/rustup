cfg_if! {
    if #[cfg(windows)] {
        mod windows;
        pub use self::windows::CertBundle;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use self::macos::CertBundle;
    } else if #[cfg(unix)] {
        mod unix;
        pub use self::unix::CertBundle;
    } else {
        // Unknown
    }
}
