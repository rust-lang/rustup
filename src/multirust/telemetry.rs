use time;
use config::Cfg;
use multirust_utils::utils;
use rustc_serialize::json;

#[derive(Debug, PartialEq)]
pub enum TelemetryMode {
    On,
    Off,
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
pub enum TelemetryEvent {
    RustcRun { duration_ms: u64, exit_code: i32, errors: Option<String> },
    ToolchainUpdate { toolchain: String, success: bool } ,
    TargetAdd { toolchain: String, target: String, success: bool },
}

#[derive(RustcDecodable, RustcEncodable, Debug)]
struct LogMessage {
    log_time_s: i64,
    event: TelemetryEvent,
}

pub fn log_telemetry(event: TelemetryEvent, cfg: &Cfg) {
    let ln = LogMessage { log_time_s: time::get_time().sec, event: event };

    let json = json::encode(&ln).unwrap();

    let now = time::now_utc();
    let filename = format!("telemetry-{}-{:02}-{:02}", now.tm_year + 1900, now.tm_mon, now.tm_mday);

    let _ = utils::append_file("telemetry", &cfg.multirust_dir.join(&filename), &json);
}