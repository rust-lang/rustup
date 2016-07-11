use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::path::PathBuf;
use itertools::Itertools;

use errors::*;
use telemetry::{LogMessage, TelemetryEvent};
use rustc_serialize::json;

pub struct TelemetryAnalysis {
    telemetry_dir: PathBuf,
    rustc_statistics: RustcStatistics,
    rustc_success_statistics: RustcStatistics,
    rustc_error_statistics: RustcStatistics,
}

#[derive(Default)]
pub struct RustcStatistics {
    rustc_execution_count: u32,
    compile_time_ms_total: u64,
    compile_time_ms_mean: u64,
    compile_time_ms_ntile_75: u64,
    compile_time_ms_ntile_90: u64,
    compile_time_ms_ntile_95: u64,
    compile_time_ms_ntile_99: u64,
    compile_time_ms_stdev: f64,
    exit_codes_with_count: HashMap<i32, i32>,
    error_codes_with_counts: HashMap<String, i32>,
}

impl RustcStatistics {
    pub fn new() -> RustcStatistics {
        Default::default()
    }
}

impl fmt::Display for RustcStatistics {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut errors: String = String::new();

        if !self.error_codes_with_counts.is_empty() {
            errors = "  rustc errors\n".to_owned();
            for (error, count) in &self.error_codes_with_counts {
                errors = errors + &format!("    '{}': {}\n", error, count);
            }
        }

        let mut exits: String = String::new();

        if !self.exit_codes_with_count.is_empty() {
            exits = "  rustc exit codes\n".to_owned();

            for (exit, count) in &self.exit_codes_with_count {
                exits = exits + &format!("    {}: {}\n", exit, count);
            }
        }

        write!(f, r"
  Total compiles: {}
  Compile Time (ms)
    Total : {}
    Mean  : {}
    STDEV : {}
    75th  : {}
    90th  : {}
    95th  : {}
    99th  : {}

{}

{}",
            self.rustc_execution_count,
            self.compile_time_ms_total,
            self.compile_time_ms_mean,
            self.compile_time_ms_stdev,
            self.compile_time_ms_ntile_75,
            self.compile_time_ms_ntile_90,
            self.compile_time_ms_ntile_95,
            self.compile_time_ms_ntile_99,
            errors,
            exits
        )
    }
}

impl fmt::Display for TelemetryAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r"
Overall rustc statistics:
{}

rustc successful execution statistics
{}

rustc error statistics
{}",
            self.rustc_statistics,
            self.rustc_success_statistics,
            self.rustc_error_statistics
        )
    }
}

impl TelemetryAnalysis {
    pub fn new(telemetry_dir: PathBuf) -> TelemetryAnalysis {
        TelemetryAnalysis {
            telemetry_dir: telemetry_dir,
            rustc_statistics: RustcStatistics::new(),
            rustc_success_statistics: RustcStatistics::new(),
            rustc_error_statistics: RustcStatistics::new(),
        }
    }

    pub fn import_telemery(&mut self) -> Result<Vec<TelemetryEvent>> {
        let mut events: Vec<TelemetryEvent> = Vec::new();
        let contents = try!(self.telemetry_dir.read_dir().chain_err(|| ErrorKind::TelemetryAnalysisError));

        let mut telemetry_files: Vec<PathBuf> = Vec::new();

        for c in contents {
            let x = c.unwrap();
            let filename = x.path().file_name().unwrap().to_str().unwrap().to_owned();

            if filename.starts_with("log") && filename.ends_with("json") {
                telemetry_files.push(x.path());
                match self.read_telemetry_file(x.path()) {
                    Ok(y) => events.extend(y),
                    Err(e) => return Err(e),
                };
            }
        }

        Ok(events)
    }

    fn read_telemetry_file(&self, path: PathBuf) -> Result<Vec<TelemetryEvent>> {
        let mut events: Vec<TelemetryEvent> = Vec::new();

        let f = try!(File::open(&path).chain_err(|| ErrorKind::TelemetryAnalysisError));

        let file = BufReader::new(&f);

        for line in file.lines() {
            use std::result;
            use rustc_serialize::json::DecoderError;

            let l = line.unwrap();
            let log_message_result: result::Result<LogMessage, DecoderError> = json::decode(&l);

            if log_message_result.is_ok() {
                let log_message = log_message_result.unwrap();
                let event: TelemetryEvent = log_message.get_event();
                events.push(event);
            }
        }

        Ok(events)
    }

    pub fn analyze_telemetry_events(&mut self, events: &[TelemetryEvent]) -> Result<()> {
        let mut rustc_durations = Vec::new();
        let mut rustc_exit_codes = Vec::new();

        let mut rustc_successful_durations = Vec::new();

        let mut rustc_error_durations = Vec::new();
        let mut error_list: Vec<Vec<String>> = Vec::new();
        let mut error_codes_with_counts: HashMap<String, i32> = HashMap::new();

        let mut toolchains = Vec::new();
        let mut toolchains_with_errors = Vec::new();
        let mut targets = Vec::new();

        let mut updated_toolchains = Vec::new();
        let mut updated_toolchains_with_errors = Vec::new();

        for event in events {
            match *event {
                TelemetryEvent::RustcRun{ duration_ms, ref exit_code, ref errors } => {
                    self.rustc_statistics.rustc_execution_count += 1;
                    rustc_durations.push(duration_ms);

                    let exit_count = self.rustc_statistics.exit_codes_with_count.entry(*exit_code).or_insert(0);
                    *exit_count += 1;

                    rustc_exit_codes.push(exit_code);

                    if errors.is_some() {
                        let errors = errors.clone().unwrap();

                        for e in &errors {
                            let error_count = error_codes_with_counts.entry(e.to_owned()).or_insert(0);
                            *error_count += 1;
                        }

                        error_list.push(errors);
                        rustc_error_durations.push(duration_ms);
                    } else {
                        rustc_successful_durations.push(duration_ms);
                    }
                },
                TelemetryEvent::TargetAdd{ ref toolchain, ref target, success } => {
                    toolchains.push(toolchain.to_owned());
                    targets.push(target.to_owned());
                    if !success {
                        toolchains_with_errors.push(toolchain.to_owned());
                    }
                },
                TelemetryEvent::ToolchainUpdate{ ref toolchain, success } => {
                    updated_toolchains.push(toolchain.to_owned());
                    if !success {
                        updated_toolchains_with_errors.push(toolchain.to_owned());
                    }
                },
            }
        };

        self.rustc_statistics = compute_rustc_percentiles(&rustc_durations);
        self.rustc_error_statistics = compute_rustc_percentiles(&rustc_error_durations);
        self.rustc_error_statistics.error_codes_with_counts = error_codes_with_counts;
        self.rustc_success_statistics = compute_rustc_percentiles(&rustc_successful_durations);

        let error_list = error_list.into_iter().flatten();

        for e in error_list {
            let error_count = self.rustc_statistics.error_codes_with_counts.entry(e).or_insert(0);
            *error_count += 1;
        }

        Ok(())
    }
}

pub fn compute_rustc_percentiles(values: &[u64]) -> RustcStatistics {
    RustcStatistics {
        rustc_execution_count: (values.len() as u32),
        compile_time_ms_total: values.iter().fold(0, |sum, val| sum + val),
        compile_time_ms_mean: mean(values),
        compile_time_ms_ntile_75: ntile(75, values),
        compile_time_ms_ntile_90: ntile(90, values),
        compile_time_ms_ntile_95: ntile(95, values),
        compile_time_ms_ntile_99: ntile(99, values),
        compile_time_ms_stdev: stdev(values),
        exit_codes_with_count: HashMap::new(),
        error_codes_with_counts: HashMap::new()
    }
}

pub fn ntile(percentile: i32, values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0u64;
    }

    let mut values = values.to_owned();
    values.sort();

    let count = values.len() as f32;
    let percentile = (percentile as f32) / 100f32;

    let n = (count * percentile).ceil() - 1f32;
    let n = n as usize;

    values[n]
}

pub fn mean(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let count = values.len() as f64;

    let sum = values.iter().fold(0, |sum, val| sum + val) as f64;

    (sum / count) as u64
}

pub fn variance(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0f64;
    }

    let mean = mean(values);

    let mut deviations: Vec<i64> = Vec::new();

    for v in values.iter() {
        let x = (*v as i64) - (mean as i64);

        deviations.push(x * x);
    }

    let sum = deviations.iter().fold(0, |sum, val| sum + val) as f64;

    sum / (values.len() as f64)
}

pub fn stdev(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0f64;
    }

    let variance = variance(values);

    variance.sqrt()
}
