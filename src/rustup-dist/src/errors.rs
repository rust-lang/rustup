use std::path::PathBuf;
use std::io::Write;
use temp;
use toml;
use rustup_utils;
use manifest::Component;
use dist::TargetTriple;

pub type Result<T> = ::std::result::Result<T, ErrorChain>;

easy_error! {
    #[derive(Debug)]
    pub chain_error ChainError;

    #[derive(Debug)]
    pub error_chain ErrorChain;

    #[derive(Debug)]
    pub error Error {
        Utils(e: rustup_utils::Error) {
            description(e.description())
            display("{}", e)
            from()
        }
        Temp {
            description("temporary file error")
        }
        InvalidFileExtension {
            description("invalid file extension")
        }
        InvalidInstaller {
            description("invalid installer")
        }
        InvalidTargetTriple(t: String) {
            description("invalid target triple")
            display("invalid target triple: '{}'", t)
        }
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        InvalidCustomToolchainName(t: String) {
            description("invalid custom toolchain name")
            display("invalid toolchain name: '{}'", t)
        }
        NotInstalledHere {
            description("not installed here")
        }
        UnsupportedHost(h: String) {
            description("binary package not provided for fost")
            display("a binary package was not provided for: '{}'", h)
        }
        ChecksumFailed {
            url: String,
            expected: String,
            calculated: String,
        } {
            description("checksum failed")
            display("checksum failed, expected: '{}', calculated: '{}'",
                    expected,
                    calculated)
        }
        ComponentConflict {
            name: String,
            path: PathBuf,
        } {
            description("conflicting component")
            display("failed to install component: '{}', detected conflict: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingFile {
            name: String,
            path: PathBuf,
        } {
            description("missing file in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        ComponentMissingDir {
            name: String,
            path: PathBuf,
        } {
            description("missing directory in component")
            display("failure removing component '{}', directory does not exist: '{:?}'",
                    name,
                    path)
        }
        CorruptComponent(name: String) {
            description("corrupt component manifest")
            display("component manifest for '{}' is corrupt", name)
        }
        ExtractingPackage {
            description("failed to extract package")
        }
        ExtensionNotFound(c: Component) {
            description("could not find extension")
            display("could not find extension: '{}-{}'", c.pkg, c.target)
        }
        InvalidChangeSet {
            description("invalid change-set")
        }
        NoGPG {
            description("could not find 'gpg' on PATH")
        }
        BadInstallerVersion(v: String) {
            description("unsupported installer version")
            display("unsupported installer version: {}", v)
        }
        BadInstalledMetadataVersion(v: String) {
            description("unsupported metadata version in existing installation")
            display("unsupported metadata version in existing installation: {}", v)
        }
        ComponentDirPermissionsFailed {
            description("I/O error walking directory during install")
        }
        ComponentFilePermissionsFailed {
            description("error setting file permissions during install")
        }
        ComponentDownloadFailed(c: Component) {
            description("component download failed")
            display("component download failed for {}-{}", c.pkg, c.target)
        }
        ObsoleteDistManifest {
            description("the server unexpectedly provided an obsolete version of the distribution manifest")
        }
        Parsing(e: Vec<toml::ParserError>) {
            description("error parsing manifest")
        }
        MissingKey(k: String) {
            description("missing key")
            display("missing key: '{}'", k)
        }
        ExpectedType(t: &'static str, n: String) {
            description("expected type")
            display("expected type: '{}' for '{}'", t, n)
        }
        PackageNotFound(p: String) {
            description("package not found")
            display("package not found: '{}'", p)
        }
        TargetNotFound(t: TargetTriple) {
            description("target not found")
            display("target not found: '{}'", t)
        }
        MissingRoot {
            description("mamnifest has no root package")
        }
        UnsupportedVersion(v: String) {
            description("unsupported manifest version")
            display("manifest version '{}' is not supported", v)
        }
        MissingPackageForComponent(c: Component) {
            description("missing package for component")
            display("server sent a broken manifest: missing package for component {}", c.name())
        }
        RequestedComponentsUnavailable(c: Vec<Component>) {
            description("some requested components are unavailable to download")
            display("{}", component_unavailable_msg(&c))
        }
        NoManifestFound(ch: String, e: Box<ErrorChain>) {
            description("no release found")
            display("{}", no_manifest_found_msg(&ch, &e))
        }
        CreatingFile(p: PathBuf) {
            description("error creating file")
            display("error creating file '{}'", p.display())
        }
    }
}

fn component_unavailable_msg(cs: &[Component]) -> String {
    assert!(!cs.is_empty());

    let mut buf = vec![];
    
    if cs.len() == 1 {
        let _ = write!(buf, "component '{}' for '{}' is unavailable for download",
                       cs[0].pkg, cs[0].target);
    } else {
        use itertools::Itertools;
        let same_target = cs.iter().all(|c| c.target == cs[0].target);
        if same_target {
            let mut cs_strs = cs.iter().map(|c| format!("'{}'", c.pkg));
            let cs_str = cs_strs.join(", ");
            let _ = write!(buf, "some components unavailable for download: {}",
                           cs_str);
        } else {
            let mut cs_strs = cs.iter().map(|c| format!("'{}' for '{}'", c.pkg, c.target));
            let cs_str = cs_strs.join(", ");
            let _ = write!(buf, "some components unavailable for download: {}",
                           cs_str);
        }
    }

    String::from_utf8(buf).expect("")
}

// FIXME This should be two different errors
fn no_manifest_found_msg(ch: &str, e: &ErrorChain) -> String {

    let mut buf = vec![];

    match *e {
        ErrorChain(Error::Utils(rustup_utils::Error::Download404 { .. }), _ ) => {
            let _ = write!(buf, "no release found for '{}'", ch);
        }
        _ => {
            // FIXME: Need handle other common cases nicely,
            // like dns lookup, network unavailable.
            let _ = write!(buf, "failed to download manifest for '{}': {}", ch, e);
        }
    }

    String::from_utf8(buf).expect("")
}

impl From<rustup_utils::ErrorChain> for ErrorChain {
    fn from(e: rustup_utils::ErrorChain) -> Self {
        ErrorChain(Error::Utils(e.0), e.1)
    }
}

impl From<temp::Error> for ErrorChain {
    fn from(e: temp::Error) -> Self {
        ErrorChain(Error::Temp, Some(Box::new(e)))
    }
}
