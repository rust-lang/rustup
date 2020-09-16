//! Test cases of the rustup command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

pub mod mock;

use std::env::consts::EXE_SUFFIX;
use std::str;

use rustup::for_host;
use rustup::test::this_host_triple;
use rustup::utils::utils;

use crate::mock::clitools::{
    self, expect_component_executable, expect_component_not_executable, expect_err,
    expect_not_stderr_ok, expect_ok, expect_ok_contains, expect_ok_eq, expect_ok_ex,
    expect_stderr_ok, expect_stdout_ok, run, set_current_dist_date, Config, Scenario,
};

pub fn setup(f: &dyn Fn(&mut Config)) {
    clitools::setup(Scenario::SimpleV2, f);
}

#[test]
fn smoke_test() {
    setup(&|config| {
        expect_ok(config, &["rustup", "--version"]);
    });
}

#[test]
fn version_mentions_rustc_version_confusion() {
    setup(&|config| {
        let out = run(config, "rustup", &vec!["--version"], &[]);
        assert!(out.ok);
        assert!(out
            .stderr
            .contains("This is the version for the rustup toolchain manager"));

        let out = run(config, "rustup", &vec!["+nightly", "--version"], &[]);
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
        let out = run(config, "rustc", &args, &[]);
        assert!(!out.ok);
        assert!(!out.stderr.contains('\x1b'));
    });
}

#[test]
fn rustc_with_bad_rustup_toolchain_env_var() {
    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = run(config, "rustc", &args, &[("RUSTUP_TOOLCHAIN", "bogus")]);
        assert!(!out.ok);
        assert!(out.stderr.contains("toolchain 'bogus' is not installed"));
    });
}

#[test]
fn custom_invalid_names() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "toolchain", "link", "nightly", "foo"],
            for_host!("invalid custom toolchain name: 'nightly-{0}'"),
        );
        expect_err(
            config,
            &["rustup", "toolchain", "link", "beta", "foo"],
            for_host!("invalid custom toolchain name: 'beta-{0}'"),
        );
        expect_err(
            config,
            &["rustup", "toolchain", "link", "stable", "foo"],
            for_host!("invalid custom toolchain name: 'stable-{0}'"),
        );
    });
}

#[test]
fn custom_invalid_names_with_archive_dates() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "toolchain", "link", "nightly-2015-01-01", "foo"],
            for_host!("invalid custom toolchain name: 'nightly-2015-01-01-{0}'"),
        );
        expect_err(
            config,
            &["rustup", "toolchain", "link", "beta-2015-01-01", "foo"],
            for_host!("invalid custom toolchain name: 'beta-2015-01-01-{0}'"),
        );
        expect_err(
            config,
            &["rustup", "toolchain", "link", "stable-2015-01-01", "foo"],
            for_host!("invalid custom toolchain name: 'stable-2015-01-01-{0}'"),
        );
    });
}

// Regression test for newline placement
#[test]
fn update_all_no_update_whitespace() {
    setup(&|config| {
        expect_stdout_ok(
            config,
            &["rustup", "update", "nightly", "--no-self-update"],
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
        let mut cmd = clitools::cmd(config, "rustup", &["update", "nightly", "--no-self-update"]);
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
        let mut cmd = clitools::cmd(config, "rustup", &["show"]);
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
        let mut cmd = clitools::cmd(config, "rustup", &["target"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains(&"USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_toolchain() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["toolchain"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains(&"USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_override() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["override"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains(&"USAGE"));
    });
}

// Issue #2425
// Exit with error and help output when called without subcommand.
#[test]
fn subcommand_required_for_self() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["self"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_eq!(out.status.code().unwrap(), 1);
        assert!(str::from_utf8(&out.stdout).unwrap().contains(&"USAGE"));
    });
}

#[test]
fn multi_host_smoke_test() {
    // We cannot run this test if the current host triple is equal to the
    // multi-arch triple, but this should never be the case.  Check that just
    // to be sure.
    assert_ne!(this_host_triple(), clitools::MULTI_ARCH1);

    clitools::setup(Scenario::MultiHost, &|config| {
        let toolchain = format!("nightly-{}", clitools::MULTI_ARCH1);
        expect_ok(config, &["rustup", "default", &toolchain]);
        expect_stdout_ok(config, &["rustc", "--version"], "xxxx-nightly-2"); // cross-host mocks have their own versions
    });
}

#[test]
fn custom_toolchain_cargo_fallback_proxy() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");

        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "link",
                "mytoolchain",
                &path.to_string_lossy(),
            ],
        );
        expect_ok(config, &["rustup", "default", "mytoolchain"]);

        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_stdout_ok(config, &["cargo", "--version"], "hash-stable-1.1.0");

        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_stdout_ok(config, &["cargo", "--version"], "hash-beta-1.2.0");

        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["cargo", "--version"], "hash-nightly-2");
    });
}

#[test]
fn custom_toolchain_cargo_fallback_run() {
    setup(&|config| {
        let path = config.customdir.join("custom-1");

        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "link",
                "mytoolchain",
                &path.to_string_lossy(),
            ],
        );
        expect_ok(config, &["rustup", "default", "mytoolchain"]);

        expect_ok(config, &["rustup", "update", "stable", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-stable-1.1.0",
        );

        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-beta-1.2.0",
        );

        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
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

        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_stdout_ok(config, hello_cmd, "hello");
    });
}

#[test]
fn rustup_failed_path_search() {
    setup(&|config| {
        use std::env::consts::EXE_SUFFIX;

        let rustup_path = config.exedir.join(&format!("rustup{}", EXE_SUFFIX));
        let tool_path = config.exedir.join(&format!("fake_proxy{}", EXE_SUFFIX));
        utils::hardlink_file(&rustup_path, &tool_path)
            .expect("Failed to create fake proxy for test");

        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "link",
                "custom",
                &config.customdir.join("custom-1").to_string_lossy(),
            ],
        );

        expect_ok(config, &["rustup", "default", "custom"]);

        let broken = &["rustup", "run", "custom", "fake_proxy"];
        expect_err(
            config,
            broken,
            &format!(
                "'fake_proxy{}' is not installed for the toolchain 'custom'",
                EXE_SUFFIX
            ),
        );

        // Hardlink will be automatically cleaned up by test setup code
    });
}

#[test]
fn rustup_failed_path_search_toolchain() {
    setup(&|config| {
        use std::env::consts::EXE_SUFFIX;

        let rustup_path = config.exedir.join(&format!("rustup{}", EXE_SUFFIX));
        let tool_path = config.exedir.join(&format!("cargo-miri{}", EXE_SUFFIX));
        utils::hardlink_file(&rustup_path, &tool_path)
            .expect("Failed to create fake cargo-miri for test");

        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "link",
                "custom-1",
                &config.customdir.join("custom-1").to_string_lossy(),
            ],
        );

        expect_ok(
            config,
            &[
                "rustup",
                "toolchain",
                "link",
                "custom-2",
                &config.customdir.join("custom-2").to_string_lossy(),
            ],
        );

        expect_ok(config, &["rustup", "default", "custom-2"]);

        let broken = &["rustup", "run", "custom-1", "cargo-miri"];
        expect_err(config, broken, "cannot use `rustup component add`");

        let broken = &["rustup", "run", "custom-2", "cargo-miri"];
        expect_err(config, broken, "cannot use `rustup component add`");

        // Hardlink will be automatically cleaned up by test setup code
    });
}

#[test]
fn rustup_run_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "install", "stable", "--no-self-update"]);
        expect_err(
            config,
            &["rustup", "run", "nightly", "rustc", "--version"],
            for_host!("toolchain 'nightly-{0}' is not installed"),
        );
    });
}

#[test]
fn rustup_run_install() {
    setup(&|config| {
        expect_ok(config, &["rustup", "install", "stable", "--no-self-update"]);
        expect_stderr_ok(
            config,
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
        expect_ok(config, &["rustup", "default", "nightly"]);

        let full_toolchain = format!("nightly-{}", this_host_triple());
        expect_stderr_ok(
            config,
            &["rustup", "default", &full_toolchain],
            &format!("info: using existing install for '{}'", full_toolchain),
        );
    });
}

#[test]
fn no_panic_on_default_toolchain_missing() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "default"],
            "no default toolchain configured",
        );
    });
}

// #190
#[test]
fn proxies_pass_empty_args() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(
            config,
            &["rustup", "run", "nightly", "rustc", "--empty-arg-test", ""],
        );
    });
}

#[test]
fn rls_exists_in_toolchain() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_ok(config, &["rustup", "component", "add", "rls"]);

        assert!(config.exedir.join(format!("rls{}", EXE_SUFFIX)).exists());
        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn run_rls_when_not_available_in_toolchain() {
    clitools::setup(Scenario::UnavailableRls, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
            &["rls", "--version"],
            &format!(
                "the 'rls' component which provides the command 'rls{}' is not available for the 'nightly-{}' toolchain",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);
        expect_ok(config, &["rustup", "component", "add", "rls"]);

        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn run_rls_when_not_installed() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "stable"]);
        expect_err(
            config,
            &["rls", "--version"],
            &format!(
                "'rls{}' is not installed for the toolchain 'stable-{}'\nTo install, run `rustup component add rls`",
                EXE_SUFFIX,
                this_host_triple(),
            ),
        );
    });
}

#[test]
fn run_rust_lldb_when_not_in_toolchain() {
    clitools::setup(Scenario::UnavailableRls, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_err(
            config,
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
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_ok(config, &["rustup", "component", "add", "rls"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);

        assert!(config.exedir.join(format!("rls{}", EXE_SUFFIX)).exists());
        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn rename_rls_after() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);
        expect_ok(config, &["rustup", "component", "add", "rls-preview"]);

        assert!(config.exedir.join(format!("rls{}", EXE_SUFFIX)).exists());
        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn rename_rls_add_old_name() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);
        expect_ok(config, &["rustup", "component", "add", "rls"]);

        assert!(config.exedir.join(format!("rls{}", EXE_SUFFIX)).exists());
        expect_ok(config, &["rls", "--version"]);
    });
}

#[test]
fn rename_rls_list() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);
        expect_ok(config, &["rustup", "component", "add", "rls"]);

        let out = run(config, "rustup", &["component", "list"], &[]);
        assert!(out.ok);
        assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
    });
}

#[test]
fn rename_rls_preview_list() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);
        expect_ok(config, &["rustup", "component", "add", "rls-preview"]);

        let out = run(config, "rustup", &["component", "list"], &[]);
        assert!(out.ok);
        assert!(out.stdout.contains(&format!("rls-{}", this_host_triple())));
    });
}

#[test]
fn rename_rls_remove() {
    clitools::setup(Scenario::ArchivesV2, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        set_current_dist_date(config, "2015-01-02");
        expect_ok(config, &["rustup", "update", "--no-self-update"]);

        expect_ok(config, &["rustup", "component", "add", "rls"]);
        expect_ok(config, &["rls", "--version"]);
        expect_ok(config, &["rustup", "component", "remove", "rls"]);
        expect_err(
            config,
            &["rls", "--version"],
            &format!("'rls{}' is not installed", EXE_SUFFIX),
        );

        expect_ok(config, &["rustup", "component", "add", "rls"]);
        expect_ok(config, &["rls", "--version"]);
        expect_ok(config, &["rustup", "component", "remove", "rls-preview"]);
        expect_err(
            config,
            &["rls", "--version"],
            &format!("'rls{}' is not installed", EXE_SUFFIX),
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

    setup(&|config| {
        // We artificially create a broken symlink toolchain -- but this can also happen "legitimately"
        // by having a proper toolchain there, using "toolchain link", and later removing the directory.
        fs::create_dir(config.rustupdir.join("toolchains")).unwrap();
        create_symlink_dir(
            config.rustupdir.join("this-directory-does-not-exist"),
            config.rustupdir.join("toolchains").join("test"),
        );
        // Make sure this "fake install" actually worked
        expect_ok_ex(config, &["rustup", "toolchain", "list"], "test\n", "");
        // Now try to uninstall it.  That should work only once.
        expect_ok_ex(
            config,
            &["rustup", "toolchain", "uninstall", "test"],
            "",
            r"info: uninstalling toolchain 'test'
info: toolchain 'test' uninstalled
",
        );
        expect_ok_ex(
            config,
            &["rustup", "toolchain", "uninstall", "test"],
            "",
            r"info: no toolchain installed for 'test'
",
        );
    });
}

// issue #1297
#[test]
fn update_unavailable_rustc() {
    clitools::setup(Scenario::Unavailable, &|config| {
        set_current_dist_date(config, "2015-01-01");
        expect_ok(config, &["rustup", "default", "nightly"]);

        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");

        // latest nightly is unavailable
        set_current_dist_date(config, "2015-01-02");
        // update should do nothing
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
    });
}

#[test]
fn update_nightly_even_with_incompat() {
    clitools::setup(Scenario::MissingComponent, &|config| {
        set_current_dist_date(config, "2019-09-12");
        expect_ok(config, &["rustup", "default", "nightly"]);

        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        expect_ok(config, &["rustup", "component", "add", "rls"]);
        expect_component_executable(config, "rls");

        // latest nightly is now one that does not have RLS
        set_current_dist_date(config, "2019-09-14");

        expect_component_executable(config, "rls");
        // update should bring us to latest nightly that does
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-2");
        expect_component_executable(config, "rls");
    });
}

#[test]
fn nightly_backtrack_skips_missing() {
    clitools::setup(Scenario::MissingNightly, &|config| {
        set_current_dist_date(config, "2019-09-16");
        expect_ok(config, &["rustup", "default", "nightly"]);

        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
        expect_ok(config, &["rustup", "component", "add", "rls"]);
        expect_component_executable(config, "rls");

        // rls is missing on latest, nightly is missing on second-to-latest
        set_current_dist_date(config, "2019-09-18");

        // update should not change nightly, and should not error
        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["rustc", "--version"], "hash-nightly-1");
    });
}

#[test]
fn completion_rustup() {
    setup(&|config| {
        expect_ok(config, &["rustup", "completions", "bash", "rustup"]);
    });
}

#[test]
fn completion_cargo() {
    setup(&|config| {
        expect_ok(config, &["rustup", "completions", "bash", "cargo"]);
    });
}

#[test]
fn completion_default() {
    setup(&|config| {
        expect_ok_eq(
            config,
            &["rustup", "completions", "bash"],
            &["rustup", "completions", "bash", "rustup"],
        );
    });
}

#[test]
fn completion_bad_shell() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "completions", "fake"],
            "error: 'fake' isn't a valid value for '<shell>'",
        );
        expect_err(
            config,
            &["rustup", "completions", "fake", "cargo"],
            "error: 'fake' isn't a valid value for '<shell>'",
        );
    });
}

#[test]
fn completion_bad_tool() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "completions", "bash", "fake"],
            "error: 'fake' isn't a valid value for '<command>'",
        );
    });
}

#[test]
fn completion_cargo_unsupported_shell() {
    setup(&|config| {
        expect_err(
            config,
            &["rustup", "completions", "fish", "cargo"],
            "error: cargo does not currently support completions for ",
        );
    });
}

#[test]
fn add_remove_component() {
    setup(&|config| {
        expect_ok(config, &["rustup", "default", "nightly"]);
        expect_component_executable(config, "rustc");
        expect_ok(config, &["rustup", "component", "remove", "rustc"]);
        expect_component_not_executable(config, "rustc");
        expect_ok(config, &["rustup", "component", "add", "rustc"]);
        expect_component_executable(config, "rustc");
    });
}

#[test]
fn which() {
    setup(&|config| {
        let path_1 = config.customdir.join("custom-1");
        let path_1 = path_1.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "custom-1", &path_1],
        );
        expect_ok(config, &["rustup", "default", "custom-1"]);
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "which", "rustc"],
            "\\toolchains\\custom-1\\bin\\rustc",
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "which", "rustc"],
            "/toolchains/custom-1/bin/rustc",
        );
        let path_2 = config.customdir.join("custom-2");
        let path_2 = path_2.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "custom-2", &path_2],
        );
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "\\toolchains\\custom-2\\bin\\rustc",
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "which", "--toolchain=custom-2", "rustc"],
            "/toolchains/custom-2/bin/rustc",
        );
    });
}

#[test]
fn override_by_toolchain_on_the_command_line() {
    setup(&|config| {
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "+stable", "which", "rustc"],
            for_host!("\\toolchains\\stable-{}"),
        );
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "+stable", "which", "rustc"],
            "\\bin\\rustc",
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "+stable", "which", "rustc"],
            for_host!("/toolchains/stable-{}"),
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "+stable", "which", "rustc"],
            "/bin/rustc",
        );
        expect_ok(config, &["rustup", "default", "nightly"]);
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("\\toolchains\\nightly-{}"),
        );
        #[cfg(windows)]
        expect_stdout_ok(
            config,
            &["rustup", "+nightly", "which", "rustc"],
            "\\bin\\rustc",
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "+nightly", "which", "rustc"],
            for_host!("/toolchains/nightly-{}"),
        );
        #[cfg(not(windows))]
        expect_stdout_ok(
            config,
            &["rustup", "+nightly", "which", "rustc"],
            "/bin/rustc",
        );
        expect_stdout_ok(
            config,
            &["rustup", "+nightly", "show"],
            "(overridden by +toolchain on the command line)",
        );
        expect_err(
            config,
            &["rustup", "+foo", "which", "rustc"],
            "toolchain 'foo' is not installed",
        );
        expect_err(
            config,
            &["rustup", "@stable", "which", "rustc"],
            "Invalid value for '<+toolchain>': Toolchain overrides must begin with '+'",
        );
        expect_stderr_ok(
            config,
            &["rustup", "+stable", "set", "profile", "minimal"],
            "profile set to 'minimal'",
        );
        expect_stdout_ok(config, &["rustup", "default"], for_host!("nightly-{}"));
    });
}

#[test]
fn toolchain_link_then_list_verbose() {
    setup(&|config| {
        let path_1 = config.customdir.join("custom-1");
        let path_1 = path_1.to_string_lossy();
        expect_ok(
            config,
            &["rustup", "toolchain", "link", "custom-1", &path_1],
        );
        #[cfg(windows)]
        expect_stdout_ok(config, &["rustup", "toolchain", "list", "-v"], "\\custom-1");
        #[cfg(not(windows))]
        expect_stdout_ok(config, &["rustup", "toolchain", "list", "-v"], "/custom-1");
    });
}

#[test]
fn deprecated_interfaces() {
    setup(&|config| {
        // In verbose mode we want the deprecated interfaces to complain
        expect_ok_contains(
            config,
            &[
                "rustup",
                "--verbose",
                "install",
                "nightly",
                "--no-self-update",
            ],
            "",
            "Please use `rustup toolchain install` instead",
        );
        expect_ok_contains(
            config,
            &["rustup", "--verbose", "uninstall", "nightly"],
            "",
            "Please use `rustup toolchain uninstall` instead",
        );
        // But if not verbose then they should *NOT* complain
        expect_not_stderr_ok(
            config,
            &["rustup", "install", "nightly", "--no-self-update"],
            "Please use `rustup toolchain install` instead",
        );
        expect_not_stderr_ok(
            config,
            &["rustup", "uninstall", "nightly"],
            "Please use `rustup toolchain uninstall` instead",
        );
    })
}
