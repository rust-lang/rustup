mod cli_exact;
mod cli_inst_interactive;
mod cli_misc;
mod cli_paths;
mod cli_rustup;
mod cli_self_upd;
mod cli_ui;
mod cli_v1;
mod cli_v2;

// The test uses features only available in test mode
#[cfg(feature = "test")]
mod concurrent_io;

mod dist_install;
mod known_triples;
