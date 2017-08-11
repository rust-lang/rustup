use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

struct Ignore;

impl<E> From<E> for Ignore
    where E: Error
{
    fn from(_: E) -> Ignore {
        Ignore
    }
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    File::create(out_dir.join("commit-info.txt"))
        .unwrap()
        .write_all(commit_info().as_bytes())
        .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

// Try to get hash and date of the last commit on a best effort basis. If anything goes wrong
// (git not installed or if this is not a git repository) just return an empty string.
fn commit_info() -> String {
    match (commit_hash(), commit_date()) {
        (Ok(hash), Ok(date)) => format!(" ({} {})", hash.trim_right(), date),
        _ => String::new(),
    }
}

fn commit_hash() -> Result<String, Ignore> {
    Ok(try!(String::from_utf8(try!(Command::new("git")
                                       .args(&["rev-parse", "--short=9", "HEAD"])
                                       .output())
                                  .stdout)))
}

fn commit_date() -> Result<String, Ignore> {
    Ok(try!(String::from_utf8(try!(Command::new("git")
                                       .args(&["log",
                                               "-1",
                                               "--date=short",
                                               "--pretty=format:%cd"])
                                       .output())
                                  .stdout)))
}
