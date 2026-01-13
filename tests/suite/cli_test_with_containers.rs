#[cfg(feature = "test-with-containers")]
mod tests {
    use std::{
        env::consts::EXE_SUFFIX,
        process::{Command, Stdio},
    };

    use rustup::{
        process::Process,
        test::{CliTestContext, Scenario, TestContainer, TestContainerContext},
    };
    use tokio::sync::{Mutex, OnceCell};

    // TODO: Figure out how to programmatically determine the published ports in running containers and then
    // use that to set the 'RUSTUP_DIST_SERVER' and 'ALL_PROXY' environment variables.
    const RUSTUP_TEST_DIST_SERVER: &str = "http://localhost:8080";
    const RUSTUP_TEST_FORWARD_PROXY: &str = "http://localhost:9080";

    struct TestContainerContexts {
        dist_server_container_context: TestContainerContext,
        forward_proxy_container_context: TestContainerContext,
        counter: usize,
    }

    impl TestContainerContexts {
        async fn new() -> Self {
            let process = Process::os();
            Self {
                dist_server_container_context: TestContainerContext::new(
                    &process,
                    TestContainer::DistServer,
                )
                .await,
                forward_proxy_container_context: TestContainerContext::new(
                    &process,
                    TestContainer::ForwardProxy,
                )
                .await,
                counter: 0,
            }
        }

        async fn start_containers(&mut self) {
            self.dist_server_container_context.run().await.unwrap();
            self.forward_proxy_container_context.run().await.unwrap();
            self.counter += 1;
        }

        async fn stop_containers(&mut self) {
            self.counter -= 1;
            if self.counter == 0 {
                self.dist_server_container_context
                    .cleanup_container()
                    .unwrap();
                self.forward_proxy_container_context
                    .cleanup_container()
                    .unwrap();
            }
        }
    }

    static TEST_CONTAINER_CONTEXTS_ONCE: OnceCell<Mutex<TestContainerContexts>> =
        OnceCell::const_new();

    macro_rules! start_containers {
        () => {{
            let mut test_container_contexts_guard = TEST_CONTAINER_CONTEXTS_ONCE
                .get_or_init(|| async { Mutex::new(TestContainerContexts::new().await) })
                .await
                .lock()
                .await;
            test_container_contexts_guard.start_containers().await;
        }};
    }

    macro_rules! stop_containers {
        () => {{
            let mut test_container_contexts_guard = TEST_CONTAINER_CONTEXTS_ONCE
                .get_or_init(|| async { Mutex::new(TestContainerContexts::new().await) })
                .await
                .lock()
                .await;
            test_container_contexts_guard.stop_containers().await;
        }};
    }

    #[tokio::test]
    async fn test_start_containers() {
        start_containers!();
        stop_containers!();
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth_missing_creds() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth_incorrect_creds() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }

    #[tokio::test]
    async fn test_dist_server_require_basic_auth() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth_missing_creds() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth_incorrect_creds() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }

    #[tokio::test]
    async fn test_forward_proxy_require_basic_auth() {
        start_containers!();
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
            .args(["-y", "--no-modify-path"])
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
        stop_containers!();
    }
}
