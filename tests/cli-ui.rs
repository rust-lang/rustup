use std::fs;

#[test]
fn ui_tests() {
    let t = trycmd::TestCases::new();
    let rustup_init = trycmd::cargo::cargo_bin("rustup-init");
    let rustup = trycmd::cargo::cargo_bin("rustup");
    t.register_bin("rustup-init", &rustup_init);
    // Copy rustup-init to rustup so that the tests can run it.
    fs::copy(&rustup_init, &rustup).unwrap();
    t.register_bin("rustup", &rustup);
    t.case("tests/cli-ui/*.toml");
    #[cfg(target_os = "windows")]
    {
        // On windows, we don't have man command, so skip the tests.
        t.skip("tests/cli-ui/rustup_man_cmd_help_flag_stdout.toml");
    }
}
