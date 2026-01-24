# Test containers

Test containers are for validating functionality when needing to use a distribution server and/or forward proxy that require
authentication. For any of the test servers, the only valid credentials will be the username:password creds 'test:123?45>6'.
The tests requiring containers will be ran when the 'test-with-containers' feature is enabled. By default, the containers are
started with the 'docker' program on non-Windows machines and 'docker.exe' on Windows machines. The program can be changed
to a different program or a full path to a program using the `RUSTUP_TEST_DOCKER_PROGRAM` environment variable. As an example,
if you need to use podman, run the following.

```sh
export RUSTUP_TEST_DOCKER_PROGRAM="podman"
```

## Manual startup of test containers.

The test containers can be started and left running by running any of the container tests and setting the
`RUSTUP_TEST_LEAVE_CONTAINERS_RUNNING` environment variable to the case-sensitive string `true`. You can also
run the following to quickly start the containers.

```sh
export RUSTUP_TEST_LEAVE_CONTAINERS_RUNNING="true"
cargo test --features test-with-containers -- suite::cli_test_with_containers::tests::test_start_containers
```

Or in powershell...

```powershell
$env:RUSTUP_TEST_LEAVE_CONTAINERS_RUNNING = "true"
cargo test --features test-with-containers -- suite::cli_test_with_containers::tests::test_start_containers
```

Use the `docker inspect` command to determine what host port is being used for each of the containers. For the
dist server container, that is the following.

```sh
docker inspect --format '{{(index (index .NetworkSettings.Ports "8080/tcp") 0).HostPort}}' rustup-test-dist-server
```

And for the forward proxy container, the command is the following.

```sh
docker inspect --format '{{(index (index .NetworkSettings.Ports "9080/tcp") 0).HostPort}}' rustup-test-forward-proxy
```

When using the test containers, set the following environment variables.

```sh
export RUSTUP_DIST_SERVER="http://localhost:${host_port}"
export RUSTUP_UPDATE_ROOT="${RUSTUP_DIST_SERVER}/rustup"
export RUSTUP_AUTHORIZATION_HEADER="Basic $(printf 'test:123?45>6' | base64)"
export RUSTUP_HOME="${HOME}/test-rustup-init"
export CARGO_HOME="${RUSTUP_HOME}"
```

In powershell, the equivalent commmands are...

```powershell
$env:RUSTUP_DIST_SERVER = "http://localhost:${host_port}"
$env:RUSTUP_UPDATE_ROOT = "${env:RUSTUP_DIST_SERVER}/rustup"
$env:RUSTUP_AUTHORIZATION_HEADER = "Basic $([Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes('test:123?45>6')))"
$env:RUSTUP_HOME = "${env:USERPROFILE}/test-rustup-init"
$env:CARGO_HOME = "${env:RUSTUP_HOME}"
```

Note that the `${host_port}` value is determined from the `docker inspect` command above.

The 'RUSTUP_LOG' environment variable should also be set so you can see what server is being used for web requests.

For testing support to set the `Proxy-Authorization` header, set these environment variables.

```sh
export ALL_PROXY="http://127.0.0.1:${host_port}"
export RUSTUP_PROXY_AUTHORIZATION_HEADER="Basic $(printf 'test:123?45>6' | base64)"
export RUSTUP_DIST_SERVER="http://rustup-test-dist-server:8080"
export RUSTUP_UPDATE_ROOT="${RUSTUP_DIST_SERVER}/rustup"
```

In powershell, equivalent commands are...

```powershell
$env:ALL_PROXY = "http://127.0.0.1:${host_port}"
$env:RUSTUP_PROXY_AUTHORIZATION_HEADER = "Basic $([Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes('test:123?45>6')))"
$env:RUSTUP_DIST_SERVER = "http://rustup-test-dist-server:8080"
$env:RUSTUP_UPDATE_ROOT = "${env:RUSTUP_DIST_SERVER}/rustup"
```

To stop and remove the test containers, run the following.

```sh
docker stop rustup-test-dist-server && docker rm rustup-test-dist-server
docker stop rustup-test-forward-proxy && docker rm rustup-test-forward-proxy
```
