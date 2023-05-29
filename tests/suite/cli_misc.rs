//! Test cases of the rustup command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

use std::str;
use std::{env::consts::EXE_SUFFIX, path::Path};

use rustup::for_host;
use rustup::test::{
    mock::clitools::{self, set_current_dist_date, Config, Scenario},
    this_host_triple,
};
use rustup::utils::utils;
use rustup_macros::integration_test as test;

pub fn setup(f: &dyn Fn(&mut Config)) {
    clitools::test(Scenario::SimpleV2, f);
}

#[test]
fn smoke_test() {
    setup(&|config| {
        config.expect_ok(&["rustup", "--version"]);
    });
}

#[test]
fn version_mentions_rustc_version_confusion() {
    setup(&|config| {
        let out = config.run("rustup", vec!["--version"], &[]);
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("This is the version for the rustup toolchain manager"));

        let out = config.run("rustup", vec!["+nightly", "--version"], &[]);
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("The currently active `rustc` version is `1.3.0"));
    });
}

#[test]
fn no_colors_in_piped_error_output() {
    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = config.run("rustc", args, &[]);
        assert!(!out.ok);
        assert!(!out.stderr.contains('\x1b'));
    });
}

#[test]
fn rustc_with_bad_rustup_toolchain_env_var() {
    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = config.run("rustc", args, &[("RUSTUP_TOOLCHAIN", "bogus")]);
        assert!(!out.ok);
        assert!(out.stderr.contains("toolchain 'bogus' is not installed"));
    });
}

#[test]
fn custom_invalid_names() {
    setup(&|config| {
        config.expect_err(
            &["rustup", "toolchain", "link", "nightly", "foo"],
            "invalid custom toolchain name 'nightly'",
        );
        config.expect_err(
            &["rustup", "toolchain", "link", "beta", "foo"],
            "invalid custom toolchain name 'beta'",
        );
        config.expect_err(
            &["rustup", "toolchain", "link", "stable", "foo"],
            "invalid custom toolchain name 'stable'",
        );
    });
}

#[test]
fn custom_invalid_names_with_archive_dates() {
    setup(&|config| {
        config.expect_err(
            &["rustup", "toolchain", "link", "nightly-2015-01-01", "foo"],
            "invalid custom toolchain name 'nightly-2015-01-01'",
        );
        config.expect_err(
            &["rustup", "toolchain", "link", "beta-2015-01-01", "foo"],
            "invalid custom toolchain name 'beta-2015-01-01'",
        );
        config.expect_err(
            &["rustup", "toolchain", "link", "stable-2015-01-01", "foo"],
            "invalid custom toolchain name 'stable-2015-01-01'",
        );
    });
}

// Regression test for newline placement
#[test]
fn update_all_no_update_whitespace() {
    setup(&|config| {
        config.expect_stdout_ok(
            &["rustup", "update", "nightly"],
            for_host!(
                r"
  nightly-{} installed - 1.3.0 (hash-nightly-2)

"
            ),
        );
    });
}

// Issue #145
#[test]
fn update_works_without_term() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["update", "nightly"]);
        clitools::env(config, &mut cmd);
        cmd.env_remove("TERM");

        let out = cmd.output().unwrap();
        assert!(out.status.success());
    });
}

// Issue #1738
#[test]
fn show_works_with_dumb_term() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["show"]);
        clitools::env(config, &mut cmd);
        cmd.env("TERM", "dumb");
        assert!(cmd.spawn().unwrap().wait().unwrap().success());
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_target() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["target"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains("USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_toolchain() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["toolchain"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains("USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_override() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["override"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains("USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_self() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", ["self"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains("USAGE"));
    });
}

#[test]
fn multi_host_smoke_test() {
    // We cannot run this test if the current host triple is equal to the
    // multi-arch triple, but this should never be the case.  Check that just
    // to be sure.
    assert_ne!(this_host_triple(), clitools::MULTI_ARCH1);

    clitools::test(Scenario::MultiHost, &|config| {
        let toolchain = format!("nightly-{}", clitools::MULTI_ARCH1);
        config.expect_ok(&["rustup", "default", &toolchain]);
        config.expect_stdout_ok(&["rustc", "--version"], "xxxx-nightly-2"); // cross-host mocks have their own versions
    });
}

#[test]
fn custom_toolchain_cargo_fallback_proxy() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");

        config.expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ]);
        config.expect_ok(&["rustup", "default", "mytoolchain"]);

        config.expect_ok(&["rustup", "update", "stable"]);
        config.expect_stdout_ok(&["cargo", "--version"], "hash-stable-1.1.0");

        config.expect_ok(&["rustup", "update", "beta"]);
        config.expect_stdout_ok(&["cargo", "--version"], "hash-beta-1.2.0");

        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_stdout_ok(&["cargo", "--version"], "hash-nightly-2");
    });
}

#[test]
fn custom_toolchain_cargo_fallback_run() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");

        config.expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "mytoolchain",
            &path.to_string_lossy(),
        ]);
        config.expect_ok(&["rustup", "default", "mytoolchain"]);

        config.expect_ok(&["rustup", "update", "stable"]);
        config.expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-stable-1.1.0",
        );

        config.expect_ok(&["rustup", "update", "beta"]);
        config.expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-beta-1.2.0",
        );

        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_stdout_ok(
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-nightly-2",
        );
    });
}

#[test]
fn rustup_run_searches_path() {
    setup(&|config| {
        #[cfg(windows)]
        let hello_cmd = &["rustup", "run", "nightly", "cmd", "/C", "echo hello"];
        #[cfg(not(windows))]
        let hello_cmd = &["rustup", "run", "nightly", "sh", "-c", "echo hello"];

        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_stdout_ok(hello_cmd, "hello");
    });
}

#[test]
fn rustup_doesnt_prepend_path_unnecessarily() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);

        let expect_stderr_ok_env_first_then =
            |config: &Config,
             args: &[&str],
             env: &[(&str, &str)],
             first: &Path,
             second: Option<&Path>| {
                let out = config.run(args[0], &args[1..], env);
                let first_then_second = |list: &str| -> bool {
                    let mut saw_first = false;
                    let mut saw_second = false;
                    for path in std::env::split_paths(list) {
                        if path == first {
                            if saw_second {
                                return false;
                            }
                            saw_first = true;
                        }
                        if Some(&*path) == second {
                            if !saw_first {
                                return false;
                            }
                            saw_second = true;
                        }
                    }
                    true
                };
                if !out.ok || !first_then_second(&out.stderr) {
                    clitools::print_command(args, &out);
                    println!("expected.ok: true");
                    clitools::print_indented(
                        "expected.stderr.first_then",
                        &format!("{} comes before {:?}", first.display(), second),
                    );
                    panic!();
                }
            };

        // For all of these, CARGO_HOME/bin will be auto-prepended.
        let cargo_home_bin = config.cargodir.join("bin");
        expect_stderr_ok_env_first_then(
            config,
            &["cargo", "--echo-path"],
            &[],
            &cargo_home_bin,
            None,
        );
        expect_stderr_ok_env_first_then(
            config,
            &["cargo", "--echo-path"],
            &[("PATH", "")],
            &cargo_home_bin,
            None,
        );

        // Check that CARGO_HOME/bin is prepended to path.
        expect_stderr_ok_env_first_then(
            config,
            &["cargo", "--echo-path"],
            &[("PATH", &format!("{}", config.exedir.display()))],
            &cargo_home_bin,
            Some(&config.exedir),
        );

        // But if CARGO_HOME/bin is already on PATH, it will not be prepended again,
        // so exedir will take precedence.
        expect_stderr_ok_env_first_then(
            config,
            &["cargo", "--echo-path"],
            &[(
                "PATH",
                std::env::join_paths([&config.exedir, &cargo_home_bin])
                    .unwrap()
                    .to_str()
                    .unwrap(),
            )],
            &config.exedir,
            Some(&cargo_home_bin),
        );
    });
}

#[test]
fn rustup_failed_path_search() {
    setup(&|config| {
        use std::env::consts::EXE_SUFFIX;

        let rustup_path = config.exedir.join(format!("rustup{EXE_SUFFIX}"));
        let tool_path = config.exedir.join(format!("fake_proxy{EXE_SUFFIX}"));
        utils::hardlink_file(&rustup_path, &tool_path)
            .expect("Failed to create fake proxy for test");

        config.expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom",
            &config.customdir.join("custom-1").to_string_lossy(),
        ]);

        config.expect_ok(&["rustup", "default", "custom"]);

        let broken = &["rustup", "run", "custom", "fake_proxy"];
        config.expect_err(
            broken,
            "unknown proxy name: 'fake_proxy'; valid proxy names are \
             'rustc', 'rustdoc', 'cargo', 'rust-lldb', 'rust-gdb', 'rust-gdbgui', \
             'rls', 'cargo-clippy', 'clippy-driver', 'cargo-miri', \
             'rust-analyzer', 'rustfmt', 'cargo-fmt'",
        );

        // Hardlink will be automatically cleaned up by test setup code
    });
}

#[test]
fn rustup_failed_path_search_toolchain() {
    setup(&|config| {
        use std::env::consts::EXE_SUFFIX;

        let rustup_path = config.exedir.join(format!("rustup{EXE_SUFFIX}"));
        let tool_path = config.exedir.join(format!("cargo-miri{EXE_SUFFIX}"));
        utils::hardlink_file(&rustup_path, &tool_path)
            .expect("Failed to create fake cargo-miri for test");

        config.expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom-1",
            &config.customdir.join("custom-1").to_string_lossy(),
        ]);

        config.expect_ok(&[
            "rustup",
            "toolchain",
            "link",
            "custom-2",
            &config.customdir.join("custom-2").to_string_lossy(),
        ]);

        config.expect_ok(&["rustup", "default", "custom-2"]);

        let broken = &["rustup", "run", "custom-1", "cargo-miri"];
        config.expect_err(broken, "cannot use `rustup component add`");

        let broken = &["rustup", "run", "custom-2", "cargo-miri"];
        config.expect_err(broken, "cannot use `rustup component add`");

        // Hardlink will be automatically cleaned up by test setup code
    });
}

#[test]
fn rustup_run_not_installed() {
    setup(&|config| {
        config.expect_ok(&["rustup", "install", "stable"]);
        config.expect_err(
            &["rustup", "run", "nightly", "rustc", "--version"],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}

#[test]
fn rustup_run_install() {
    setup(&|config| {
        config.expect_ok(&["rustup", "install", "stable"]);
        config.expect_stderr_ok(
            &[
                "rustup",
                "run",
                "--install",
                "nightly",
                "cargo",
                "--version",
            ],
            "info: installing component 'rustc'",
        );
    });
}

#[test]
fn toolchains_are_resolved_early() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);

        let full_toolchain = format!("nightly-{}", this_host_triple());
        config.expect_stderr_ok(
            &["rustup", "default", &full_toolchain],
            &format!("info: using existing install for '{full_toolchain}'"),
        );
    });
}

#[test]
fn no_panic_on_default_toolchain_missing() {
    setup(&|config| {
        config.expect_err(&["rustup", "default"], "no default toolchain configured");
    });
}

// #190
#[test]
fn proxies_pass_empty_args() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "run", "nightly", "rustc", "--empty-arg-test", ""]);
    });
}

#[test]
fn rls_exists_in_toolchain() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "stable"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);

        assert!(config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn run_rls_when_not_available_in_toolchain() {
    clitools::test(Scenario::UnavailableRls, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_err(
            &["rls", "--version"],
            &format!(
                "the 'rls' component which provides the command 'rls{}' is not available for the 'nightly-{}' toolchain",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);

        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn run_rls_when_not_installed() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "stable"]);
        config.expect_err(
            &["rls", "--version"],
            &format!(
                "'rls{}' is not installed for the toolchain 'stable-{}'.\nTo install, run `rustup component add rls`",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );
    });
}

#[test]
fn run_rust_lldb_when_not_in_toolchain() {
    clitools::test(Scenario::UnavailableRls, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_err(
            &["rust-lldb", "--version"],
            &format!(
                "the 'rust-lldb{}' binary, normally provided by the 'rustc' component, is not applicable to the 'nightly-{}' toolchain",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );
    });
}

#[test]
fn rename_rls_before() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);

        assert!(config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn rename_rls_after() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);
        config.expect_ok(&["rustup", "component", "add", "rls-preview"]);

        assert!(config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn rename_rls_add_old_name() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);

        assert!(config.exedir.join(format!("rls{EXE_SUFFIX}")).exists());
        config.expect_ok(&["rls", "--version"]);
    });
}

#[test]
fn rename_rls_list() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);
        config.expect_ok(&["rustup", "component", "add", "rls"]);

        let out = config.run("rustup", ["component", "list"], &[]);
        assert!(out.ok);
        assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
    });
}

#[test]
fn rename_rls_preview_list() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);
        config.expect_ok(&["rustup", "component", "add", "rls-preview"]);

        let out = config.run("rustup", ["component", "list"], &[]);
        assert!(out.ok);
        assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
    });
}

#[test]
fn rename_rls_remove() {
    clitools::test(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        config.expect_ok(&["rustup", "update"]);

        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_ok(&["rls", "--version"]);
        config.expect_ok(&["rustup", "component", "remove", "rls"]);
        config.expect_err(
            &["rls", "--version"],
            &format!("'rls{EXE_SUFFIX}' is not installed"),
        );

        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_ok(&["rls", "--version"]);
        config.expect_ok(&["rustup", "component", "remove", "rls-preview"]);
        config.expect_err(
            &["rls", "--version"],
            &format!("'rls{EXE_SUFFIX}' is not installed"),
        );
    });
}

// issue #1169
#[test]
#[cfg(any(unix, windows))]
fn toolchain_broken_symlink() {
    use std::fs;
    use std::path::Path;

    #[cfg(unix)]
    fn create_symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        use std::os::unix::fs;
        fs::symlink(src, dst).unwrap();
    }

    #[cfg(windows)]
    fn create_symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) {
        use std::os::windows::fs;
        fs::symlink_dir(src, dst).unwrap();
    }

    clitools::test(Scenario::None, &|config| {
        // We artificially create a broken symlink toolchain -- but this can also happen "legitimately"
        // by having a proper toolchain there, using "toolchain link", and later removing the directory.
        fs::create_dir(config.rustupdir.join("toolchains")).unwrap();
        create_symlink_dir(
            config.rustupdir.join("this-directory-does-not-exist"),
            config.rustupdir.join("toolchains").join("test"),
        );
        // Make sure this "fake install" actually worked
        config.expect_ok_ex(&["rustup", "toolchain", "list"], "test\n", "");
        // Now try to uninstall it.  That should work only once.
        config.expect_ok_ex(
            &["rustup", "toolchain", "uninstall", "test"],
            "",
            r"info: uninstalling toolchain 'test'
info: toolchain 'test' uninstalled
",
        );
        config.expect_stderr_ok(
            &["rustup", "toolchain", "uninstall", "test"],
            "no toolchain installed for 'test'",
        );
    });
}

// issue #1297
#[test]
fn update_unavailable_rustc() {
    clitools::test(Scenario::Unavailable, &|config| {
        set_current_dist_date(config, "2015-01-01");
        config.expect_ok(&["rustup", "default", "nightly"]);

        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");

        // latest nightly is unavailable
        set_current_dist_date(config, "2015-01-02");
        // update should do nothing
        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    });
}

// issue 2562
#[test]
fn install_unavailable_platform() {
    clitools::test(Scenario::Unavailable, &|config| {
        set_current_dist_date(config, "2015-01-02");
        // explicit attempt to install should fail
        config.expect_err(
            &["rustup", "toolchain", "install", "nightly"],
            "is not installable",
        );
        // implicit attempt to install should fail
        config.expect_err(&["rustup", "default", "nightly"], "is not installable");
    });
}

#[test]
fn update_nightly_even_with_incompat() {
    clitools::test(Scenario::MissingComponent, &|config| {
        set_current_dist_date(config, "2019-09-12");
        config.expect_ok(&["rustup", "default", "nightly"]);

        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_component_executable("rls");

        // latest nightly is now one that does not have RLS
        set_current_dist_date(config, "2019-09-14");

        config.expect_component_executable("rls");
        // update should bring us to latest nightly that does
        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-2");
        config.expect_component_executable("rls");
    });
}

#[test]
fn nightly_backtrack_skips_missing() {
    clitools::test(Scenario::MissingNightly, &|config| {
        set_current_dist_date(config, "2019-09-16");
        config.expect_ok(&["rustup", "default", "nightly"]);

        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
        config.expect_ok(&["rustup", "component", "add", "rls"]);
        config.expect_component_executable("rls");

        // rls is missing on latest, nightly is missing on second-to-latest
        set_current_dist_date(config, "2019-09-18");

        // update should not change nightly, and should not error
        config.expect_ok(&["rustup", "update", "nightly"]);
        config.expect_stdout_ok(&["rustc", "--version"], "hash-nightly-1");
    });
}

#[test]
fn completion_rustup() {
    setup(&|config| {
        config.expect_ok(&["rustup", "completions", "bash", "rustup"]);
    });
}

#[test]
fn completion_cargo() {
    setup(&|config| {
        config.expect_ok(&["rustup", "completions", "bash", "cargo"]);
    });
}

#[test]
fn completion_default() {
    setup(&|config| {
        config.expect_ok_eq(
            &["rustup", "completions", "bash"],
            &["rustup", "completions", "bash", "rustup"],
        );
    });
}

#[test]
fn completion_bad_shell() {
    setup(&|config| {
        config.expect_err(
            &["rustup", "completions", "fake"],
            r#"error: "fake" isn't a valid value for '<shell>'"#,
        );
        config.expect_err(
            &["rustup", "completions", "fake", "cargo"],
            r#"error: "fake" isn't a valid value for '<shell>'"#,
        );
    });
}

#[test]
fn completion_bad_tool() {
    setup(&|config| {
        config.expect_err(
            &["rustup", "completions", "bash", "fake"],
            r#"error: "fake" isn't a valid value for '<command>'"#,
        );
    });
}

#[test]
fn completion_cargo_unsupported_shell() {
    setup(&|config| {
        config.expect_err(
            &["rustup", "completions", "fish", "cargo"],
            "error: cargo does not currently support completions for ",
        );
    });
}

#[test]
fn add_remove_component() {
    setup(&|config| {
        config.expect_ok(&["rustup", "default", "nightly"]);
        config.expect_component_executable("rustc");
        config.expect_ok(&["rustup", "component", "remove", "rustc"]);
        config.expect_component_not_executable("rustc");
        config.expect_ok(&["rustup", "component", "add", "rustc"]);
        config.expect_component_executable("rustc");
    });
}

#[test]
fn which() {
    setup(&|config| {
        let path_1 = config.customdir.join("custom-1");
        let path_1 = path_1.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1]);
        config.expect_ok(&["rustup", "default", "custom-1"]);
        #[cfg(windows)]
        config.expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "\\toolchains\\custom-1\\bin\\rustc",
        );
        #[cfg(not(windows))]
        config.expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "/toolchains/custom-1/bin/rustc",
        );
        let path_2 = config.customdir.join("custom-2");
        let path_2 = path_2.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom-2", &path_2]);
        #[cfg(windows)]
        config.expect_stdout_ok(
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "\\toolchains\\custom-2\\bin\\rustc",
        );
        #[cfg(not(windows))]
        config.expect_stdout_ok(
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "/toolchains/custom-2/bin/rustc",
        );
    });
}

#[test]
fn which_asking_uninstalled_toolchain() {
    setup(&|config| {
        let path_1 = config.customdir.join("custom-1");
        let path_1 = path_1.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1]);
        config.expect_ok(&["rustup", "default", "custom-1"]);
        #[cfg(windows)]
        config.expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "\\toolchains\\custom-1\\bin\\rustc",
        );
        #[cfg(not(windows))]
        config.expect_stdout_ok(
            &["rustup", "which", "rustc"],
            "/toolchains/custom-1/bin/rustc",
        );
        config.expect_err(
            &["rustup", "which", "--toolchain=nightly", "rustc"],
            for_host!("toolchain 'nightly-{}' is not installed"),
        );
    });
}

#[test]
fn override_by_toolchain_on_the_command_line() {
    setup(&|config| {
        #[cfg(windows)]
        config.expect_stdout_ok(
            &["rustup", "+stable", "which", "rustc"],
            for_host!("\\toolchains\\stable-{}"),
        );
        #[cfg(windows)]
        config.expect_stdout_ok(&["rustup", "+stable", "which", "rustc"], "\\bin\\rustc");
        #[cfg(not(windows))]
        config.expect_stdout_ok(
            &["rustup", "+stable", "which", "rustc"],
            for_host!("/toolchains/stable-{}"),
        );
        #[cfg(not(windows))]
        config.expect_stdout_ok(&["rustup", "+stable", "which", "rustc"], "/bin/rustc");
        config.expect_ok(&["rustup", "default", "nightly"]);
        #[cfg(windows)]
        config.expect_stdout_ok(
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("\\toolchains\\nightly-{}"),
        );
        #[cfg(windows)]
        config.expect_stdout_ok(&["rustup", "+nightly", "which", "rustc"], "\\bin\\rustc");
        #[cfg(not(windows))]
        config.expect_stdout_ok(
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("/toolchains/nightly-{}"),
        );
        #[cfg(not(windows))]
        config.expect_stdout_ok(&["rustup", "+nightly", "which", "rustc"], "/bin/rustc");
        config.expect_stdout_ok(
            &["rustup", "+nightly", "show"],
            "(overridden by +toolchain on the command line)",
        );
        config.expect_err(
            &["rustup", "+foo", "which", "rustc"],
            "toolchain 'foo' is not installed",
        );
        config.expect_stderr_ok(
            &["rustup", "+stable", "set", "profile", "minimal"],
            "profile set to 'minimal'",
        );
        config.expect_stdout_ok(&["rustup", "default"], for_host!("nightly-{}"));
    });
}

#[test]
fn toolchain_link_then_list_verbose() {
    setup(&|config| {
        let path_1 = config.customdir.join("custom-1");
        let path_1 = path_1.to_string_lossy();
        config.expect_ok(&["rustup", "toolchain", "link", "custom-1", &path_1]);
        #[cfg(windows)]
        config.expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "\\custom-1");
        #[cfg(not(windows))]
        config.expect_stdout_ok(&["rustup", "toolchain", "list", "-v"], "/custom-1");
    });
}

#[test]
fn deprecated_interfaces() {
    setup(&|config| {
        // In verbose mode we want the deprecated interfaces to complain
        config.expect_ok_contains(
            &["rustup", "--verbose", "install", "nightly"],
            "",
            "Please use `rustup toolchain install` instead",
        );
        config.expect_ok_contains(
            &["rustup", "--verbose", "uninstall", "nightly"],
            "",
            "Please use `rustup toolchain uninstall` instead",
        );
        // But if not verbose then they should *NOT* complain
        config.expect_not_stderr_ok(
            &["rustup", "install", "nightly"],
            "Please use `rustup toolchain install` instead",
        );
        config.expect_not_stderr_ok(
            &["rustup", "uninstall", "nightly"],
            "Please use `rustup toolchain uninstall` instead",
        );
    })
}
