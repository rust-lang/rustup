# Changelog

## [1.29.0] - Unreleased

This new release of rustup comes with significant changes.

The headline feature is improved UI and concurrency for certain operations:

- `rustup` now uses `indicatif`-based progress bars for toolchain update checks
  and downloads. This provides a more consistent and visually appealing
  experience for different commands like `rustup check`, `rustup update`,
  and `rustup toolchain install`. [pr#4388] [pr#4426]

- `rustup check` will check for toolchain updates concurrently. [pr#4388]

- `rustup update` and `rustup toolchain` subcommands will download toolchain
  components concurrently. [pr#4436] 

  - You can use the `RUSTUP_CONCURRENT_DOWNLOADS`
    environment variable to adjust the number of concurrent downloads to your
    specific environment. [pr#4455]

rustup now officially supports the following host platforms:

- `sparcv9-sun-solaris` [pr#4380]
- `x86_64-pc-solaris` [pr#4380]

This release also comes with other quality-of-life improvements, to name a few:

- When running rust-analyzer via a proxy, rustup will consider the
  `rust-analyzer` binary from `PATH` when the rustup-managed one is not found.
  [pr#4324]

- Empty environment variables are now treated as unset. This should help with
  resetting configuration values to default when an override is present.
  [pr#4422]

- Basic support for the `tcsh` shell has been added. [pr#4459]

[1.29.0]: https://github.com/rust-lang/rustup/releases/tag/1.29.0
[pr#4324]: https://github.com/rust-lang/rustup/pull/4324
[pr#4380]: https://github.com/rust-lang/rustup/pull/4380
[pr#4388]: https://github.com/rust-lang/rustup/pull/4388
[pr#4422]: https://github.com/rust-lang/rustup/pull/4422
[pr#4426]: https://github.com/rust-lang/rustup/pull/4426
[pr#4436]: https://github.com/rust-lang/rustup/pull/4436
[pr#4455]: https://github.com/rust-lang/rustup/pull/4455
[pr#4459]: https://github.com/rust-lang/rustup/pull/4459

### Detailed changes

* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4321
* docs(changelog): mirror changes from the release announcement, take 2 by @rami3l in https://github.com/rust-lang/rustup/pull/4322
* deps: update aws-lc-rs to 1.13.1 by @ognevny in https://github.com/rust-lang/rustup/pull/4326
* feat(toolchain): consider external `rust-analyzer` when calling a proxy by @rami3l in https://github.com/rust-lang/rustup/pull/4324
* chore(deps): bump semver-compatible dependencies by @rami3l in https://github.com/rust-lang/rustup/pull/4329
* toolchain: hoist binary name conditionals out of fallback functions by @djc in https://github.com/rust-lang/rustup/pull/4330
* Fix cargo lints on Windows by @ChrisDenton in https://github.com/rust-lang/rustup/pull/4335
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4341
* Log versions during self updates by @djc in https://github.com/rust-lang/rustup/pull/4331
* feat(custom-toolchains): `rustup show` now reporting installed targets by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4333
* refactor(test): add new APIs for easier snapshot testing by @rami3l in https://github.com/rust-lang/rustup/pull/4334
* style(test): clarify uses of `snapbox::str![]` by @rami3l in https://github.com/rust-lang/rustup/pull/4342
* feat(self_update): add proxy sanity checks by @manyinsects in https://github.com/rust-lang/rustup/pull/4338
* rustup check: add exit status and no-self-update logic by @tjkirch in https://github.com/rust-lang/rustup/pull/4340
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4348
* fix(deps): update rust crate opener to 0.8.0 by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4346
* Skip manifest loading if there are no components/targets to check by @Kobzol in https://github.com/rust-lang/rustup/pull/4350
* feat(custom-toolchains): targets and components are now reported for custom toolchains by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4347
* Custom list tweaks by @djc in https://github.com/rust-lang/rustup/pull/4351
* test(cli-self-upd): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4343
* test(cli-paths): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4354
* Unset RUSTUP_AUTO_INSTALL for tests by @ChrisDenton in https://github.com/rust-lang/rustup/pull/4360
* test(cli-inst-interactive): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4353
* Increase Windows main thread stack size to 2mb by @ChrisDenton in https://github.com/rust-lang/rustup/pull/4362
* Clean up installed components/targets list tweaks by @djc in https://github.com/rust-lang/rustup/pull/4361
* test(cli-exact): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4352
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4357
* Update platforms to 3.6 by @djc in https://github.com/rust-lang/rustup/pull/4364
* Fix CI image names for downloading ARM and PowerPC artifacts by @Kobzol in https://github.com/rust-lang/rustup/pull/4368
* test(cli-rustup): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4365
* fix(deps): update opentelemetry by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4371
* test(download): serialize tests with proxy-sensitive URLs by @cuviper in https://github.com/rust-lang/rustup/pull/4372
* test(cli-misc): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4363
* test(cli-v1): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4366
* test(cli-v2): migrate to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4367
* Upgrade to rustls-platform-verifier 0.6 by @djc in https://github.com/rust-lang/rustup/pull/4373
* docs: mention the `Assert` APIs, add misc fixes by @rami3l in https://github.com/rust-lang/rustup/pull/4374
* Update bash completions instructions and test by @rickhg12hs in https://github.com/rust-lang/rustup/pull/4378
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4379
* test: finish migration to `.expect()` APIs by @rami3l in https://github.com/rust-lang/rustup/pull/4376
* add Solaris support by @psumbera in https://github.com/rust-lang/rustup/pull/4380
* Fix rustup-init.sh cputype check for sparcv9 by @psumbera in https://github.com/rust-lang/rustup/pull/4382
* docs(README): update CI status badge by @rami3l in https://github.com/rust-lang/rustup/pull/4383
* Emphasize that `llvm-tools` dist component is not subject to compiler stability guarantees by @jieyouxu in https://github.com/rust-lang/rustup/pull/4384
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4386
* Upgrade to windows-sys 0.60 by @djc in https://github.com/rust-lang/rustup/pull/4387
* Block broken snap curl by @konstin in https://github.com/rust-lang/rustup/pull/4389
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4390
* docs: replace Discord links by @rami3l in https://github.com/rust-lang/rustup/pull/4393
* ci(run): install `codegen-cli` with `cargo-binstall` by @rami3l in https://github.com/rust-lang/rustup/pull/4394
* feat(www): improve "copy" button style compatibility with Chromium by @rami3l in https://github.com/rust-lang/rustup/pull/4395
* fix(ci/run): specify target triple for bindgen-cli installation by @rami3l in https://github.com/rust-lang/rustup/pull/4398
* style: migrate the codebase to `let-chains` by @rami3l in https://github.com/rust-lang/rustup/pull/4397
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4399
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4402
* Check for updates concurrently by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4388
* Bump `toml` to 0.9 by @Kobzol in https://github.com/rust-lang/rustup/pull/4405
* Limit the default number of I/O threads by @ChrisDenton in https://github.com/rust-lang/rustup/pull/4407
* Introduce `RUSTUP_TERM_WIDTH` and `RUSTUP_TERM_PROGRESS_WHEN` by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4406
* Simplify updates by @djc in https://github.com/rust-lang/rustup/pull/4404
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4410
* fix(ci/docker): update `CC` name for `powerpc64le-unknown-linux-gnu` by @rami3l in https://github.com/rust-lang/rustup/pull/4411
* fix(toolchain/distributable): refine handling of known targets with no prebuilt artifacts by @rami3l in https://github.com/rust-lang/rustup/pull/4409
* Set a maximum thread limit for the `remove_dir_all` crate by @ChrisDenton in https://github.com/rust-lang/rustup/pull/4413
* opt(err): show renaming file error source by @Binlogo in https://github.com/rust-lang/rustup/pull/4414
* Limit Tokio worker threads to I/O thread count by @djc in https://github.com/rust-lang/rustup/pull/4417
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4418
* docs(user-guide/environment-variables): update description of `RUSTUP_IO_THREADS` by @rami3l in https://github.com/rust-lang/rustup/pull/4427
* ci(macos): run x64 workflows with Rosetta 2 by @rami3l in https://github.com/rust-lang/rustup/pull/4428
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4430
* Refactor the DownloadTracker in favor of `indicatif` by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4426
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4437
* test: detach snapshots from component installation order by @rami3l in https://github.com/rust-lang/rustup/pull/4435
* feat: improve error message for `rustup which` by @Bogay in https://github.com/rust-lang/rustup/pull/4429
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4441
* Introduce `RUSTUP_DOWNLOAD_TIMEOUT` to override the download timeout by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4440
* chore(deps): update actions/checkout action to v5 by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4443
* Justify the presence of an `.unwrap()` on getting the content of an `OnceLock` by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4444
* Concurrently download components of a toolchain by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4436
* Delete unnecessary Download(Pop/Push)Unit notifications by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4447
* ci(check): make installation of `taplo-cli` faster by @AudaciousAxiom in https://github.com/rust-lang/rustup/pull/4449
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4451
* Introduce `RUSTUP_CONCURRENT_DOWNLOADS` to control concurrency by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4450
* chore(deps): disable default features for zstd by @klensy in https://github.com/rust-lang/rustup/pull/4453
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4456
* Introduce a `Semaphore` to fully honor `RUSTUP_CONCURRENT_DOWNLOADS` by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4455
* Treat empty environment variables as unset by @djc in https://github.com/rust-lang/rustup/pull/4422
* Report the real elapsed time of a download instead of the cumulative time by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4460
* Replace non_empty_env_var() with Process::var_opt() by @djc in https://github.com/rust-lang/rustup/pull/4461
* feat(config): add support for `tcsh` shell by @cachebag in https://github.com/rust-lang/rustup/pull/4459
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4467
* Correct `DownloadTracker` reporting on retries and failures by @FranciscoTGouveia in https://github.com/rust-lang/rustup/pull/4466
* Remove hardcoded dependency to the master branch by @Kobzol in https://github.com/rust-lang/rustup/pull/4470
* feat(install): warn if default linker (`cc`) is missing from PATH by @cachebag in https://github.com/rust-lang/rustup/pull/4469
* chore(deps): update aws-actions/configure-aws-credentials action to v5 by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4472
* fix(cli/rustup_mode): use ASCII-compatible spinner by @rami3l in https://github.com/rust-lang/rustup/pull/4473
* chore(deps): lock file maintenance by @renovate[bot] in https://github.com/rust-lang/rustup/pull/4478
* Upgrade windows crates by @djc in https://github.com/rust-lang/rustup/pull/4479
* chore(deps/renovate): group version bumps for `windows-rs` crates by @rami3l in https://github.com/rust-lang/rustup/pull/4480
* feat: adjust default concurrent download count by @rami3l in https://github.com/rust-lang/rustup/pull/4474
* docs(README): link CI status badge to GitHub Actions panel by @rami3l in https://github.com/rust-lang/rustup/pull/4482
* docs(dev-guide): mention the arg0 override trick on welcome page by @rami3l in https://github.com/rust-lang/rustup/pull/4484
* chore(deps): lock file maintenance by @rami3l in https://github.com/rust-lang/rustup/pull/4486
* Fix link in the bug reporting template by @LingMan in https://github.com/rust-lang/rustup/pull/4487
* Remove unneeded paranthesees by @DasMatus in https://github.com/rust-lang/rustup/pull/4488
* chore(deps): lock file maintenance by @rami3l in https://github.com/rust-lang/rustup/pull/4492
* Further refactoring of self update logic by @djc in https://github.com/rust-lang/rustup/pull/4412
* Simplify package unpacking code by @djc in https://github.com/rust-lang/rustup/pull/4490
* refactor: remove redundant references by @rami3l in https://github.com/rust-lang/rustup/pull/4494
* Simplify component downloads by @djc in https://github.com/rust-lang/rustup/pull/4489
* Flatten notification enums by @djc in https://github.com/rust-lang/rustup/pull/4496
* tests: deduplicate distribution installation tests by @djc in https://github.com/rust-lang/rustup/pull/4495
* tests: use DistContext for dist::component tests by @djc in https://github.com/rust-lang/rustup/pull/4500
* Start removing Notification variants by @djc in https://github.com/rust-lang/rustup/pull/4499
* refactor: Remove unused ColorableTerminal::carriage_return by @epage in https://github.com/rust-lang/rustup/pull/4506
* refactor: Switch logging to anstyle by @epage in https://github.com/rust-lang/rustup/pull/4507
* Remove more Notification variants by @djc in https://github.com/rust-lang/rustup/pull/4501
* ci: use macOS Intel runners by @djc in https://github.com/rust-lang/rustup/pull/4509
* Upgrade opentelemetry dependencies by @djc in https://github.com/rust-lang/rustup/pull/4508
* Move the default branch from `master` to `main` by @Kobzol in https://github.com/rust-lang/rustup/pull/4511
