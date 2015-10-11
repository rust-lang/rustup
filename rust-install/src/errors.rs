
use std::path::{Path, PathBuf};
use std::io;
use std::process::ExitStatus;
use std::fmt::{self, Display};
use std::ffi::OsString;
use temp;
use hyper;

pub use self::Notification::*;
pub use ::notify::NotificationLevel;

pub enum Notification<'a> {
	CreatingDirectory(&'a str, &'a Path),
	Temp(temp::Notification<'a>),
	SetDefaultToolchain(&'a str),
	SetOverrideToolchain(&'a Path, &'a str),
	LookingForToolchain(&'a str),
	ToolchainDirectory(&'a Path, &'a str),
	UpdatingToolchain(&'a str),
	InstallingToolchain(&'a str),
	UsingExistingToolchain(&'a str),
	LinkingDirectory(&'a Path, &'a Path),
	CopyingDirectory(&'a Path, &'a Path),
	RemovingDirectory(&'a str, &'a Path),
	Extracting(&'a Path, &'a Path),
	UninstallingToolchain(&'a str),
	UninstalledToolchain(&'a str),
	ToolchainNotInstalled(&'a str),
	DownloadingFile(&'a hyper::Url, &'a Path),
	UpgradingMetadata(&'a str, &'a str),
	WritingMetadataVersion(&'a str),
	ReadMetadataVersion(&'a str),
	NoCanonicalPath(&'a Path),
	UpdateHashMatches(&'a str),
	CantReadUpdateHash(&'a Path),
	NoUpdateHash(&'a Path),
	ChecksumValid(&'a str),
	NonFatalError(&'a Error),
}

pub enum Error {
	LocatingHome,
	LocatingWorkingDir,
	ReadingFile { name: &'static str, path: PathBuf, error: io::Error },
	ReadingDirectory { name: &'static str, path: PathBuf, error: io::Error },
	WritingFile { name: &'static str, path: PathBuf, error: io::Error },
	CreatingFile { name: &'static str, path: PathBuf, error: io::Error },
	CreatingDirectory { name: &'static str, path: PathBuf, error: io::Error },
	FilteringFile { name: &'static str, src: PathBuf, dest: PathBuf, error: io::Error },
	RenamingFile { name: &'static str, src: PathBuf, dest: PathBuf, error: io::Error },
	RenamingDirectory { name: &'static str, src: PathBuf, dest: PathBuf, error: io::Error },
	DownloadingFile { url: hyper::Url, path: PathBuf },
	RunningCommand { name: OsString, error: io::Error },
	CommandStatus { name: OsString, status: ExitStatus },
	NotAFile { path: PathBuf },
	NotADirectory { path: PathBuf },
	LinkingDirectory(PathBuf, PathBuf),
	CopyingDirectory(PathBuf, PathBuf),
	CopyingFile(PathBuf, PathBuf),
	RemovingFile { name: &'static str, path: PathBuf, error: io::Error },
	RemovingDirectory { name: &'static str, path: PathBuf, error: io::Error },
	InvalidFileExtension,
	InvalidInstaller,
	InvalidToolchainName,
	InvalidInstallerUrl,
	OpeningBrowser,
	UnknownMetadataVersion(String),
	InvalidEnvironment,
	NoDefaultToolchain,
	NotInstalledHere,
	InstallTypeNotPossible,
	AlreadyInstalledHere,
	InvalidUrl,
	UnsupportedHost(String),
	PermissionDenied,
	SettingPermissions(PathBuf),
	ToolchainNotInstalled(String),
	ChecksumFailed { url: String, expected: String, calculated: String },
	Custom { id: String, desc: String },
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler = ::notify::NotifyHandler<for<'a> Fn(Notification<'a>)>;

impl From<temp::Error> for Error {
	fn from(e: temp::Error) -> Error {
		match e {
			temp::Error::CreatingRoot { path, error } =>
				Error::CreatingDirectory { name: "temp root", path: path, error: error },
			temp::Error::CreatingFile { path, error } =>
				Error::CreatingFile { name: "temp", path: path, error: error },
			temp::Error::CreatingDirectory { path, error } =>
				Error::CreatingDirectory { name: "temp", path: path, error: error },
		}
	}
}

impl<'a> Notification<'a> {
	pub fn level(&self) -> NotificationLevel {
		match *self {
			Temp(ref t) => t.level(),
			CreatingDirectory(_, _) | ToolchainDirectory(_, _) | LookingForToolchain(_) |
			RemovingDirectory(_, _) | WritingMetadataVersion(_) | ReadMetadataVersion(_) |
			NoUpdateHash(_) =>
				NotificationLevel::Verbose,
			LinkingDirectory(_, _) | CopyingDirectory(_, _) | DownloadingFile(_, _) |
			Extracting(_, _) | ChecksumValid(_) =>
				NotificationLevel::Normal,
			SetDefaultToolchain(_) | SetOverrideToolchain(_, _) | UpdatingToolchain(_) |
			InstallingToolchain(_) | UsingExistingToolchain(_) | UninstallingToolchain(_) |
			UninstalledToolchain(_) | ToolchainNotInstalled(_) | UpgradingMetadata(_, _) |
			UpdateHashMatches(_) =>
				NotificationLevel::Info,
			NoCanonicalPath(_) | CantReadUpdateHash(_) =>
				NotificationLevel::Warn,
			NonFatalError(_) =>
				NotificationLevel::Error,
		}
	}
}

impl<'a> Display for Notification<'a> {
	fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
		match *self {
			CreatingDirectory(name, path) => write!(f, "creating {} directory: '{}'", name, path.display()),
			Temp(ref t) => write!(f, "{}", t),
			SetDefaultToolchain(name) =>
				write!(f, "default toolchain set to '{}'", name),
			SetOverrideToolchain(path, name) =>
				write!(f, "override toolchain for '{}' set to '{}'", path.display(), name),
			LookingForToolchain(name) =>
				write!(f, "looking for installed toolchain '{}'", name),
			ToolchainDirectory(path, _) =>
				write!(f, "toolchain directory: '{}'", path.display()),
			UpdatingToolchain(name) =>
				write!(f, "updating existing install for '{}'", name),
			InstallingToolchain(name) =>
				write!(f, "installing toolchain '{}'", name),
			UsingExistingToolchain(name) =>
				write!(f, "using existing install for '{}'", name),
			LinkingDirectory(_, dest) =>
				write!(f, "linking directory from: '{}'", dest.display()),
			CopyingDirectory(src, _) =>
				write!(f, "coping directory from: '{}'", src.display()),
			RemovingDirectory(name, path) =>
				write!(f, "removing {} directory: '{}'", name, path.display()),
			Extracting(_, _) =>
				write!(f, "extracting..."),
			UninstallingToolchain(name) =>
				write!(f, "uninstalling toolchain '{}'", name),
			UninstalledToolchain(name) =>
				write!(f, "toolchain '{}' uninstalled", name),
			ToolchainNotInstalled(name) =>
				write!(f, "no toolchain installed for '{}'", name),
			DownloadingFile(url, _) =>
				write!(f, "downloading file from: '{}'", url),
			UpgradingMetadata(from_ver, to_ver) =>
				write!(f, "upgrading metadata version from '{}' to '{}'", from_ver, to_ver),
			WritingMetadataVersion(ver) =>
				write!(f, "writing metadata version: '{}'", ver),
			ReadMetadataVersion(ver) =>
				write!(f, "read metadata version: '{}'", ver),
			NoCanonicalPath(path) =>
				write!(f, "could not canonicalize path: '{}'", path.display()),
			UpdateHashMatches(hash) =>
				write!(f, "update hash matches: {}, skipping update...", hash),
			CantReadUpdateHash(path) =>
				write!(f, "can't read update hash file: '{}', can't skip update...", path.display()),
			NoUpdateHash(path) =>
				write!(f, "no update hash at: '{}'", path.display()),
			ChecksumValid(_) =>
				write!(f, "checksum passed"),
			NonFatalError(e) =>
				write!(f, "{}", e),
		}
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
		match *self {
			Error::LocatingHome
				=> write!(f, "could not locate home directory"),
			Error::LocatingWorkingDir
				=> write!(f, "could not locate working directory"),
			Error::ReadingFile { ref name, ref path, error: _ }
				=> write!(f, "could not read {} file: '{}'", name, path.display()),
			Error::ReadingDirectory { ref name, ref path, error: _ }
				=> write!(f, "could not read {} directory: '{}'", name, path.display()),
			Error::WritingFile { ref name, ref path, error: _ }
				=> write!(f, "could not write {} file: '{}'", name, path.display()),
			Error::CreatingFile { ref name, ref path, error: _ }
				=> write!(f, "could not create {} file: '{}'", name, path.display()),
			Error::CreatingDirectory { ref name, ref path, error: _ }
				=> write!(f, "could not create {} directory: '{}'", name, path.display()),
			Error::FilteringFile { ref name, ref src, ref dest, error: _ }
				=> write!(f, "could not copy {} file from '{}' to '{}'", name, src.display(), dest.display() ),
			Error::RenamingFile { ref name, ref src, ref dest, error: _ }
				=> write!(f, "could not rename {} file from '{}' to '{}'", name, src.display(), dest.display() ),
			Error::RenamingDirectory { ref name, ref src, ref dest, error: _ }
				=> write!(f, "could not rename {} directory from '{}' to '{}'", name, src.display(), dest.display() ),
			Error::DownloadingFile { ref url, ref path }
				=> write!(f, "could not download file from '{}' to '{}'", url, path.display()),
			Error::RunningCommand { ref name, error: _ }
				=> write!(f, "could not run command: '{}'", PathBuf::from(name).display()),
			Error::CommandStatus { ref name, ref status }
				=> write!(f, "command '{}' terminated with {}", PathBuf::from(name).display(), status),
			Error::NotAFile { ref path }
				=> write!(f, "not a file: '{}'", path.display()),
			Error::NotADirectory { ref path }
				=> write!(f, "not a directory: '{}'", path.display()),
			Error::LinkingDirectory(ref src, ref dest)
				=> write!(f, "could not create symlink from '{}' to '{}'", src.display(), dest.display()),
			Error::CopyingDirectory(ref src, ref dest)
				=> write!(f, "could not copy directory from '{}' to '{}'", src.display(), dest.display()),
			Error::CopyingFile(ref src, ref dest)
				=> write!(f, "could not copy file from '{}' to '{}'", src.display(), dest.display()),
			Error::RemovingFile { ref name, ref path, error: _ }
				=> write!(f, "could not remove {} file: '{}'", name, path.display()),
			Error::RemovingDirectory { ref name, ref path, error: _ }
				=> write!(f, "could not remove {} directory: '{}'", name, path.display()),
			Error::InvalidFileExtension
				=> write!(f, "invalid file extension"),
			Error::InvalidInstaller
				=> write!(f, "invalid installer"),
			Error::InvalidToolchainName
				=> write!(f, "invalid custom toolchain name"),
			Error::InvalidInstallerUrl
				=> write!(f, "invalid installer url"),
			Error::OpeningBrowser
				=> write!(f, "could not open browser"),
			Error::UnknownMetadataVersion(ref ver)
				=> write!(f, "unknown metadata version: '{}'", ver),
			Error::InvalidEnvironment
				=> write!(f, "invalid environment"),
			Error::NoDefaultToolchain
				=> write!(f, "no default toolchain configured"),
			Error::NotInstalledHere
				=> write!(f, "not installed here"),
			Error::InstallTypeNotPossible
				=> write!(f, "install type not possible"),
			Error::AlreadyInstalledHere
				=> write!(f, "already installed here"),
			Error::InvalidUrl
				=> write!(f, "invalid url"),
			Error::UnsupportedHost(ref spec)
				=> write!(f, "a binary package was not provided for: '{}'", spec),
			Error::PermissionDenied
				=> write!(f, "permission denied"),
			Error::SettingPermissions(ref path)
				=> write!(f, "failed to set permissions for: '{}'", path.display()),
			Error::ToolchainNotInstalled(ref name)
				=> write!(f, "toolchain '{}' is not installed", name),
			Error::ChecksumFailed { url: _, ref expected, ref calculated }
				=> write!(f, "checksum failed, expected: '{}', calculated: '{}'", expected, calculated),
			Error::Custom { id: _, ref desc }
				=> write!(f, "{}", desc),
		}
	}
}
