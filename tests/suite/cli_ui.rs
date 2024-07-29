use std::{fs, path::PathBuf};

#[test]
fn rustup_ui_doc_text_tests() {
    let t = trycmd::TestCases::new();
    let home = tempfile::TempDir::new().unwrap();
    let rustup_init = trycmd::cargo::cargo_bin("rustup-init");
    let rustup = trycmd::cargo::cargo_bin("rustup");
    // Copy rustup-init to rustup so that the tests can run it.
    fs::copy(rustup_init, &rustup).unwrap();
    t.register_bin("rustup", &rustup);
    t.case("tests/suite/cli-ui/rustup/*.toml");
    // once installed rustup asserts the presence of ~/.rustup/settings.toml if
    // Config is instantiated.
    t.env("HOME", home.path().to_string_lossy());
    #[cfg(target_os = "windows")]
    {
        // On windows, we don't have man command, so skip the test.
        t.skip("tests/suite/cli-ui/rustup/rustup_man_cmd_help_flag_stdout.toml");
    }
}

#[test]
fn rustup_init_ui_doc_text_tests() {
    let t = trycmd::TestCases::new();
    let rustup_init = trycmd::cargo::cargo_bin("rustup-init");
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    t.register_bin("rustup-init", &rustup_init);
    t.register_bin("rustup-init.sh", project_root.join("rustup-init.sh"));
    t.case("tests/suite/cli-ui/rustup-init/*.toml");
    #[cfg(target_os = "windows")]
    {
        // On windows, we don't use rustup-init.sh, so skip the test.
        t.skip("tests/suite/cli-ui/rustup-init/rustup-init_sh_help_flag_stdout.toml");
    }

    // On non-windows, we don't use rustup-init.sh, so skip the test.
    #[cfg(not(target_os = "windows"))]
    {
        let rustup_init_help_toml =
            project_root.join("tests/suite/cli-ui/rustup-init/rustup-init_help_flag_stdout.toml");
        let rustup_init_sh_help_toml = project_root
            .join("tests/suite/cli-ui/rustup-init/rustup-init_sh_help_flag_stdout.toml");

        #[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
        struct Stdout {
            #[serde(default)]
            pub(crate) stdout: Option<String>,
        }
        let rustup_init_help_std_out: Stdout =
            toml::from_str(fs::read_to_string(rustup_init_help_toml).unwrap().as_str()).unwrap();
        let rustup_init_sh_help_std_out: Stdout = toml::from_str(
            fs::read_to_string(rustup_init_sh_help_toml)
                .unwrap()
                .as_str(),
        )
        .unwrap();

        // Make sure that the help output of rustup-init and rustup-init.sh are the same.
        assert_eq!(
            rustup_init_help_std_out.stdout.unwrap(),
            rustup_init_sh_help_std_out.stdout.unwrap()
        )
    }
}
