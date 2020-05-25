#![allow(dead_code)]
use std::path::PathBuf;

#[derive(PartialEq)]
pub enum PathUpdateMethod {
    RcFile(PathBuf),
    Windows,
}
