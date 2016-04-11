use time;
use multirust_utils::utils;
use rustc_serialize::json;

use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, PartialEq)]
pub enum TelemetryMode {
    On,
    Off,
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
pub enum TelemetryEvent {
    RustcRun { duration_ms: u64, exit_code: i32, errors: Option<HashSet<String>> },
    ToolchainUpdate { toolchain: String, success: bool } ,
    TargetAdd { toolchain: String, target: String, success: bool },
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
struct LogMessage {
    log_time_s: i64,
    event: TelemetryEvent,
}

#[derive(Debug)]
pub struct Telemetry {
    telemetry_dir: PathBuf
}

impl Telemetry {
    pub fn new(telemetry_dir: PathBuf) -> Telemetry {
        Telemetry { telemetry_dir: telemetry_dir }
    }   

    pub fn log_telemetry(&self, event: TelemetryEvent) {
        let current_time = time::now_utc();
        let ln = LogMessage { log_time_s: current_time.to_timespec().sec, event: event };

        let json = json::encode(&ln).unwrap();

        let filename = format!("log-{}-{:02}-{:02}.json", current_time.tm_year + 1900, current_time.tm_mon + 1, current_time.tm_mday);

        let _ = utils::append_file("telemetry", &self.telemetry_dir.join(&filename), &json);
    }
}