//! Tests for rustup's proxy executables.

use std::{env::consts::EXE_SUFFIX, fs};

use rustup::{
    DUP_TOOLS, TOOLS,
    env_var::RUST_RECURSION_COUNT_MAX,
    test::{CliTestContext, Scenario, SelfUpdateTestContext, mock_bin, output_release_file},
    utils::{self, raw},
};
use snapbox::IntoData;

#[tokio::test]
async fn proxies_select_toolchain_and_forward_args() {
    let cx = setup_installed_proxies().await;

    // Custom toolchains let us provide every tool, including optional components
    // and debugger wrappers, without assuming they ship for the test host.
    for name in ["custom-1", "custom-2"] {
        let path = cx.config.customdir.join(name);
        for tool in TOOLS.iter().chain(DUP_TOOLS.iter()) {
            for file in mock_bin(tool, "1.0.0", name) {
                file.build(&path);
            }
        }
        cx.config
            .expect(["rustup", "toolchain", "link", name, path.to_str().unwrap()])
            .await
            .is_ok();
    }
    cx.config
        .expect(["rustup", "default", "custom-1"])
        .await
        .is_ok();

    for tool in TOOLS.iter().chain(DUP_TOOLS.iter()) {
        // Run the installed proxy, rather than a fixture link or `rustup run`.
        let proxy = cx.config.cargodir.join(format!("bin/{tool}{EXE_SUFFIX}"));
        for (toolchain, override_arg) in [("custom-1", None), ("custom-2", Some("+custom-2"))] {
            let mut args = vec![proxy.to_str().unwrap()];
            if let Some(override_arg) = override_arg {
                args.push(override_arg);
            }
            // Cargo passes the subcommand name to cargo-* executables.
            if let Some(subcommand) = tool.strip_prefix("cargo-") {
                args.push(subcommand);
            }
            let mut exe_args = args.clone();
            exe_args.push("--echo-current-exe");
            let output = cx.config.expect(exe_args).await;
            output.is_ok().with_stdout("");
            let expected = cx
                .config
                .customdir
                .join(toolchain)
                .join(format!("bin/{tool}{EXE_SUFFIX}"));
            assert!(
                same_file::is_same_file(output.output.stderr.trim(), &expected).unwrap(),
                "{tool} should execute {}",
                expected.display()
            );

            args.extend([
                "--echo-args",
                "first",
                "",
                "two words",
                "\"quoted\"",
                "back\\slash",
                "+not-a-toolchain",
                "--",
                "--flag",
            ]);
            cx.config
                .expect(args)
                .await
                .is_ok()
                .with_stdout("")
                .with_stderr(
                    snapbox::str![[r#"
first

two words
"quoted"
back\slash
+not-a-toolchain
--
--flag

"#]]
                    .raw(),
                );
        }
    }
}

#[tokio::test]
async fn proxies_report_missing_tools() {
    let cx = setup_installed_proxies().await;
    let path = cx.config.customdir.join("custom-1");
    cx.config
        .expect([
            "rustup",
            "toolchain",
            "link",
            "custom",
            path.to_str().unwrap(),
        ])
        .await
        .is_ok();
    cx.config
        .expect(["rustup", "default", "custom"])
        .await
        .is_ok();
    // The custom fixture only supplies rustc. Remove it after linking so every
    // proxy must report a missing tool, including rustc itself.
    fs::remove_file(path.join(format!("bin/rustc{EXE_SUFFIX}"))).unwrap();

    let bin_dir = cx.config.cargodir.join("bin");
    // Exercise one PATH re-entry before reporting the missing tool, rather than
    // launching a full chain of proxy processes for every tool.
    let recursion_count = (RUST_RECURSION_COUNT_MAX - 1).to_string();
    for tool in TOOLS.iter().chain(DUP_TOOLS.iter()) {
        let proxy = bin_dir.join(format!("{tool}{EXE_SUFFIX}"));
        cx.config
            .expect_with_env(
                [proxy.to_str().unwrap(), "--version"],
                [
                    // Prevent fallbacks to tools installed on the test host.
                    ("PATH", bin_dir.to_str().unwrap()),
                    ("RUST_RECURSION_COUNT", &recursion_count),
                ],
            )
            .await
            .extend_redactions([("[TOOL]", *tool)])
            .is_err()
            .with_stdout("")
            .with_stderr(snapbox::str![[r#"
error: '[TOOL][EXE]' is not installed for the custom toolchain 'custom'
note: this is a custom toolchain, which cannot use `rustup component add`
help: if you built this toolchain from source, and used `rustup toolchain link`, then you may be able to build the component with `x.py`

"#]]);
    }
}

#[tokio::test]
async fn update_does_not_overwrite_dup_tools() {
    let cx = SelfUpdateTestContext::new(TEST_VERSION).await;
    cx.config
        .expect(["rustup-init", "-y", "--no-modify-path"])
        .await
        .is_ok();
    let version = env!("CARGO_PKG_VERSION");
    output_release_file(cx.path(), "1", version);

    for tool in DUP_TOOLS {
        // Replace the installed proxy with a file that is not managed by rustup.
        let tool_path = cx.config.cargodir.join(format!("bin/{tool}{EXE_SUFFIX}"));
        assert!(tool_path.exists());
        fs::remove_file(&tool_path).unwrap();
        raw::write_file(&tool_path, "").unwrap();
        assert_eq!(utils::file_size(&tool_path).unwrap(), 0);

        // Updating must preserve the user's tool and explain how to replace it.
        cx.config
            .expect(["rustup", "self", "update"])
            .await
            .extend_redactions([("[TOOL]", *tool)])
            .with_stderr(snapbox::str![[r#"
info: checking for self-update (current version: [CURRENT_VERSION])
warn: tool `[TOOL]` is already installed, remove it from `[..]`, then run `rustup update` to have rustup manage this tool.

"#]])
            .is_ok();
        assert!(tool_path.exists());
        assert_eq!(utils::file_size(&tool_path).unwrap(), 0);

        // Once the user removes the tool, updating installs its proxy. Use the
        // installed rustup to also exercise updating while that file is open.
        fs::remove_file(&tool_path).unwrap();
        let installed_rustup = cx.config.cargodir.join(format!("bin/rustup{EXE_SUFFIX}"));
        cx.config
            .expect([installed_rustup.to_str().unwrap(), "self", "update"])
            .await
            .is_ok();
        assert!(same_file::is_same_file(&tool_path, &installed_rustup).unwrap());
    }
}

async fn setup_installed_proxies() -> CliTestContext {
    let cx = CliTestContext::new(Scenario::None).await;
    cx.config
        .expect([
            "rustup-init",
            "-y",
            "--no-modify-path",
            "--default-toolchain=none",
        ])
        .await
        .is_ok();
    cx
}

const TEST_VERSION: &str = "1.1.1";
