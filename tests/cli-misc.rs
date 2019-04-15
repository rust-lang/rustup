//! Test cases of the rustup command that do not depend on the
//! dist server, mostly derived from multirust/test-v2.sh

pub mod mock;

use crate::mock::clitools::{
    self, expect_err, expect_ok, expect_ok_eq, expect_ok_ex, expect_stderr_ok, expect_stdout_ok,
    run, set_current_dist_date, this_host_triple, Config, Scenario,
};
use rustup::errors::TOOLSTATE_MSG;
use rustup::utils::{raw, utils};

use std::env::consts::EXE_SUFFIX;
use tempdir::TempDir;

macro_rules! for_host {
    ($s: expr) => {
        &format!($s, this_host_triple())
    };
}

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
fn no_colors_in_piped_error_output() {
    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = run(config, "rustc", &args, &[]);
        assert!(!out.ok);
        assert!(!out.stderr.contains("\u{1b}"));
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
  nightly-{} installed - 1.3.0 (hash-n-2)

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

// Issue #140
// Don't panic when `target`, `update` etc. are called without subcommands.
#[test]
fn subcommand_required_for_target() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["target"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_ne!(out.status.code().unwrap(), 101);
    });
}

// Issue #140
// Don't panic when `target`, `update` etc. are called without subcommands.
#[test]
fn subcommand_required_for_toolchain() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["toolchain"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_ne!(out.status.code().unwrap(), 101);
    });
}

// Issue #140
// Don't panic when `target`, `update` etc. are called without subcommands.
#[test]
fn subcommand_required_for_override() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["override"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_ne!(out.status.code().unwrap(), 101);
    });
}

// Issue #140
// Don't panic when `target`, `update` etc. are called without subcommands.
#[test]
fn subcommand_required_for_self() {
    setup(&|config| {
        let mut cmd = clitools::cmd(config, "rustup", &["self"]);
        clitools::env(config, &mut cmd);
        let out = cmd.output().unwrap();
        assert!(!out.status.success());
        assert_ne!(out.status.code().unwrap(), 101);
    });
}

#[test]
fn multi_host_smoke_test() {
    // FIXME: Unfortunately the list of supported hosts is hard-coded,
    // so we have to use the triple of a host we actually test on. That means
    // that when we're testing on that host we can't test 'multi-host'.
    let trip = this_host_triple();
    if trip == clitools::MULTI_ARCH1 {
        return;
    }

    clitools::setup(Scenario::MultiHost, &|config| {
        let ref toolchain = format!("nightly-{}", clitools::MULTI_ARCH1);
        expect_ok(config, &["rustup", "default", toolchain]);
        expect_stdout_ok(config, &["rustc", "--version"], "xxxx-n-2"); // cross-host mocks have their own versions
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
        expect_stdout_ok(config, &["cargo", "--version"], "hash-s-2");

        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_stdout_ok(config, &["cargo", "--version"], "hash-b-2");

        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(config, &["cargo", "--version"], "hash-n-2");
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
            "hash-s-2",
        );

        expect_ok(config, &["rustup", "update", "beta", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-b-2",
        );

        expect_ok(config, &["rustup", "update", "nightly", "--no-self-update"]);
        expect_stdout_ok(
            config,
            &["rustup", "run", "mytoolchain", "cargo", "--version"],
            "hash-n-2",
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

        let ref rustup_path = config.exedir.join(&format!("rustup{}", EXE_SUFFIX));
        let ref tool_path = config.exedir.join(&format!("fake_proxy{}", EXE_SUFFIX));
        utils::hardlink_file(rustup_path, tool_path).expect("Failed to create fake proxy for test");

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

        let ref rustup_path = config.exedir.join(&format!("rustup{}", EXE_SUFFIX));
        let ref tool_path = config.exedir.join(&format!("cargo-miri{}", EXE_SUFFIX));
        utils::hardlink_file(rustup_path, tool_path)
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
        expect_err(
            config,
            broken,
            "rustup component add miri --toolchain custom-1",
        );

        let broken = &["rustup", "run", "custom-2", "cargo-miri"];
        expect_err(config, broken, "rustup component add miri");

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
fn rls_does_not_exist_in_toolchain() {
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

#[test]
fn install_stops_if_rustc_exists() {
    let temp_dir = TempDir::new("fakebin").unwrap();
    // Create fake executable
    let ref fake_exe = temp_dir.path().join(&format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = run(
            config,
            "rustup-init",
            &args,
            &[
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", &temp_dir_path),
            ],
        );
        assert!(!out.ok);
        assert!(out
            .stderr
            .contains("it looks like you have an existing installation of Rust at:"));
        assert!(out
            .stderr
            .contains("if this is what you want, restart the installation with `-y'"));
    });
}

#[test]
fn install_stops_if_cargo_exists() {
    let temp_dir = TempDir::new("fakebin").unwrap();
    // Create fake executable
    let ref fake_exe = temp_dir.path().join(&format!("{}{}", "cargo", EXE_SUFFIX));
    raw::append_file(fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    setup(&|config| {
        let args: Vec<&str> = vec![];
        let out = run(
            config,
            "rustup-init",
            &args,
            &[
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", &temp_dir_path),
            ],
        );
        assert!(!out.ok);
        assert!(out
            .stderr
            .contains("it looks like you have an existing installation of Rust at:"));
        assert!(out
            .stderr
            .contains("if this is what you want, restart the installation with `-y'"));
    });
}

#[test]
fn with_no_prompt_install_succeeds_if_rustc_exists() {
    let temp_dir = TempDir::new("fakebin").unwrap();
    // Create fake executable
    let ref fake_exe = temp_dir.path().join(&format!("{}{}", "rustc", EXE_SUFFIX));
    raw::append_file(fake_exe, "").unwrap();
    let temp_dir_path = temp_dir.path().to_str().unwrap();

    setup(&|config| {
        let out = run(
            config,
            "rustup-init",
            &["-y"],
            &[
                ("RUSTUP_INIT_SKIP_PATH_CHECK", "no"),
                ("PATH", &temp_dir_path),
            ],
        );
        assert!(out.ok);
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
        // We artifically create a broken symlink toolchain -- but this can also happen "legitimately"
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

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");

        set_current_dist_date(config, "2015-01-02");
        expect_err(
            config,
            &["rustup", "update", "nightly"],
            format!(
                "some components unavailable for download: 'rustc', 'cargo' for channel 'nightly'\n{}",
                TOOLSTATE_MSG
            )
            .as_str(),
        );

        expect_stdout_ok(config, &["rustc", "--version"], "hash-n-1");
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
