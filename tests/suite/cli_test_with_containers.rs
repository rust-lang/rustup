#[cfg(feature = "test-with-containers")]
mod tests {
    use std::{
        env::consts::EXE_SUFFIX,
        process::{Command, Stdio},
        sync::{Arc, LazyLock, Mutex},
    };

    use rustup::{
        process::Process,
        test::{CliTestContext, Scenario, TestContainer, TestContainerContext},
    };

    // TODO: Figure out how to programmatically determine the published ports in running containers and then
    // use that to set the 'RUSTUP_DIST_SERVER' and 'ALL_PROXY' environment variables.
    const RUSTUP_TEST_DIST_SERVER: &str = "http://localhost:8080";
    const RUSTUP_TEST_FORWARD_PROXY: &str = "http://localhost:9080";

    static RUSTUP_TEST_DIST_SERVER_CONTAINER_CONTEXT: LazyLock<Arc<Mutex<TestContainerContext>>> =
        LazyLock::new(|| {
            let process = Process::os();
            Arc::new(Mutex::new(TestContainerContext::new(
                &process,
                TestContainer::DistServer,
            )))
        });
    static RUSTUP_TEST_FORWARD_PROXY_CONTAINER_CONTEXT: LazyLock<Arc<Mutex<TestContainerContext>>> =
        LazyLock::new(|| {
            let process = Process::os();
            Arc::new(Mutex::new(TestContainerContext::new(
                &process,
                TestContainer::ForwardProxy,
            )))
        });

    async fn start_containers() {
        let mut test_dist_server_container_context_guard =
            (*RUSTUP_TEST_DIST_SERVER_CONTAINER_CONTEXT).lock().unwrap();
        test_dist_server_container_context_guard
            .run()
            .await
            .unwrap();
        let mut test_forward_proxy_container_context_guard =
            (*RUSTUP_TEST_FORWARD_PROXY_CONTAINER_CONTEXT)
                .lock()
                .unwrap();
        test_forward_proxy_container_context_guard
            .run()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_start_containers() {
        start_containers().await;
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth_missing_creds() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        let mut command = Command::new(rustup_init_path);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", RUSTUP_TEST_DIST_SERVER)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{RUSTUP_TEST_DIST_SERVER}/rustup").as_str(),
            );
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(!exit_status.success());
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth_incorrect_creds() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        let mut command = Command::new(rustup_init_path);
        // Basic creds derived from 'test:123?45>67'.
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", RUSTUP_TEST_DIST_SERVER)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{RUSTUP_TEST_DIST_SERVER}/rustup").as_str(),
            )
            .env("RUSTUP_AUTHORIZATION_HEADER", "Basic dGVzdDoxMjM/NDU+Njc=");
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(!exit_status.success());
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        let mut command = Command::new(rustup_init_path);
        // Basic creds derived from 'test:123?45>6'.
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", RUSTUP_TEST_DIST_SERVER)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{RUSTUP_TEST_DIST_SERVER}/rustup").as_str(),
            )
            .env("RUSTUP_AUTHORIZATION_HEADER", "Basic dGVzdDoxMjM/NDU+Ng==");
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(exit_status.success());
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth_missing_creds() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        // Inside of the container, the test dist server needs to be reached using its hostname.
        let rust_test_dist_server = format!("http://{}:8080", TestContainer::DistServer);
        let mut command = Command::new(rustup_init_path);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", &rust_test_dist_server)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{rust_test_dist_server}/rustup").as_str(),
            )
            .env("RUSTUP_AUTHORIZATION_HEADER", "Basic dGVzdDoxMjM/NDU+Ng==")
            .env("ALL_PROXY", RUSTUP_TEST_FORWARD_PROXY);
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(!exit_status.success());
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth_incorrect_creds() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        // Inside of the container, the test dist server needs to be reached using its hostname.
        let rust_test_dist_server = format!("http://{}:8080", TestContainer::DistServer);
        let mut command = Command::new(rustup_init_path);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", &rust_test_dist_server)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{rust_test_dist_server}/rustup").as_str(),
            )
            .env("RUSTUP_AUTHORIZATION_HEADER", "Basic dGVzdDoxMjM/NDU+Ng==")
            .env("ALL_PROXY", RUSTUP_TEST_FORWARD_PROXY)
            .env(
                "RUSTUP_PROXY_AUTHORIZATION_HEADER",
                "Basic dGVzdDoxMjM/NDU+Njc=",
            );
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(!exit_status.success());
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth() {
        start_containers().await;
        let cli_test_context = CliTestContext::new(Scenario::None).await;
        let rustup_init_path = cli_test_context
            .config
            .exedir
            .join(format!("rustup-init{EXE_SUFFIX}"));
        // Inside of the container, the test dist server needs to be reached using its hostname.
        let rust_test_dist_server = format!("http://{}:8080", TestContainer::DistServer);
        let mut command = Command::new(rustup_init_path);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .args(&["-y", "--no-modify-path"])
            .env(
                "RUSTUP_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env(
                "CARGO_HOME",
                cli_test_context.config.rustupdir.to_string().as_str(),
            )
            .env("RUSTUP_DIST_SERVER", &rust_test_dist_server)
            .env(
                "RUSTUP_UPDATE_ROOT",
                format!("{rust_test_dist_server}/rustup").as_str(),
            )
            .env("RUSTUP_AUTHORIZATION_HEADER", "Basic dGVzdDoxMjM/NDU+Ng==")
            .env("ALL_PROXY", RUSTUP_TEST_FORWARD_PROXY)
            .env(
                "RUSTUP_PROXY_AUTHORIZATION_HEADER",
                "Basic dGVzdDoxMjM/NDU+Ng==",
            );
        let mut child = command.spawn().unwrap();
        let exit_status = child.wait().unwrap();
        assert!(exit_status.success());
    }
}
