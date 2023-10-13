# Developer tips and tricks

## `RUSTUP_FORCE_ARG0`

The environment variable `RUSTUP_FORCE_ARG0` can be used to get rustup to think
it's a particular binary, rather than e.g. copying it, symlinking it or other
tricks with exec. This is handy when testing particular code paths from cargo
run.

```shell
RUSTUP_FORCE_ARG0=rustup cargo run -- uninstall nightly
```

## `RUSTUP_BACKTRACK_LIMIT`

If it's necessary to alter the backtracking limit from the default of half
a release cycle for some reason, you can set the `RUSTUP_BACKTRACK_LIMIT`
environment variable. If this is unparsable as an `i32` or if it's absent
then the default of 21 days (half a cycle) is used. If it parses and is less
than 1, it is clamped to 1 at minimum.

This is not meant for use by users, but can be suggested in diagnosing an issue
should one arise with the backtrack limits.

## `RUSTUP_MAX_RETRIES`

When downloading a file, rustup will retry the download a number of times. The
default is 3 times, but if this variable is set to a valid usize then it is the
max retry count. A value of `0` means no retries, thus the default of `3` will
mean a download is tried a total of four times before failing out.

## `RUSTUP_BACKTRACE`

By default while running tests, we unset some environment variables that will
break our testing (like `RUSTUP_TOOLCHAIN`, `SHELL`, `ZDOTDIR`, `RUST_BACKTRACE`).
But if you want to debug locally, you may need backtrace. `RUSTUP_BACKTRACE`
is used like `RUST_BACKTRACE` to enable backtraces of failed tests.

**NOTE**: This is a backtrace for the test, not for any subprocess invocation of
rustup process running in the test

```bash
$ RUSTUP_BACKTRACE=1 cargo test --release --test cli-v1 -- remove_toolchain_then_add_again
    Finished release [optimized] target(s) in 0.38s
     Running target\release\deps\cli_v1-1f29f824792f6dc1.exe

running 1 test
test remove_toolchain_then_add_again ... FAILED

failures:

---- remove_toolchain_then_add_again stdout ----
thread 'remove_toolchain_then_add_again' panicked at 'called `Result::unwrap()` on an `Err` value: Os { code: 1142, kind: Other, message: "An attempt was made to create more links on a file than the file system supports." }', src\libcore\result.rs:999:5
stack backtrace:
   0: backtrace::backtrace::trace_unsynchronized
             at C:\Users\appveyor\.cargo\registry\src\github.com-1ecc6299db9ec823\backtrace-0.3.29\src\backtrace\mod.rs:66
   1: std::sys_common::backtrace::_print
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\sys_common\backtrace.rs:47
   2: std::sys_common::backtrace::print
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\sys_common\backtrace.rs:36
   3: std::panicking::default_hook::{{closure}}
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:198
   4: std::panicking::default_hook
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:209
   5: std::panicking::rust_panic_with_hook
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:475
   6: std::panicking::continue_panic_fmt
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:382
   7: std::panicking::rust_begin_panic
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libstd\panicking.rs:309
   8: core::panicking::panic_fmt
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libcore\panicking.rs:85
   9: core::result::unwrap_failed
  10: cli_v1::mock::clitools::test
  11: alloc::boxed::{{impl}}::call_once<(),FnOnce<()>>
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\liballoc\boxed.rs:746
  12: panic_unwind::__rust_maybe_catch_panic
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libpanic_unwind\lib.rs:82
  13: std::panicking::try
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\libstd\panicking.rs:273
  14: std::panic::catch_unwind
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\src\libstd\panic.rs:388
  15: test::run_test::run_test_inner::{{closure}}
             at /rustc/de02101e6d949c4a9040211e9ce8c488a997497e\/src\libtest\lib.rs:1466
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.


failures:
    remove_toolchain_then_add_again

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 26 filtered out

error: test failed, to rerun pass '--test cli-v1'
```
