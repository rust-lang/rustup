use utils;
use temp;
use install::InstallPrefix;
use distv2::DistV2;
use errors::*;

use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use std::mem;

#[derive(Copy, Clone)]
pub struct Components<'a> {
	prefix: &'a InstallPrefix,
}

impl<'a> Components<'a> {
	pub fn new(dist: DistV2<'a>) -> Self {
		Components { prefix: dist.prefix() }
	}
	fn components_file(self) -> PathBuf {
		self.prefix.manifest_file("components")
	}
	pub fn list(self) -> Result<Vec<Component<'a>>> {
		let path = self.components_file();
		let content = try!(utils::read_file("components", &path));
		Ok(content.lines().map(|s| Component::new(self, s.to_owned()) ).collect())
	}
	pub fn add(self, name: &str, temp_cfg: &'a temp::Cfg, notify_handler: SharedNotifyHandler) -> Result<AddingComponent<'a>> {
		let result = try!(AddingComponent::new(self, name, temp_cfg, notify_handler));
		try!(utils::append_file("components", &self.components_file(), name));
		Ok(result)
	}
	pub fn find(self, name: &str) -> Result<Option<Component<'a>>> {
		let result = try!(self.list());
		Ok(result.into_iter().filter(|c| (c.name() == name)).next())
	}
}

pub struct AddingComponent<'a> {
	component: Option<Component<'a>>,
	file: Option<File>,
	temp_cfg: &'a temp::Cfg,
	notify_handler: SharedNotifyHandler,
}

impl<'a> AddingComponent<'a> {
	fn new(components: Components<'a>, name: &str, temp_cfg: &'a temp::Cfg, notify_handler: SharedNotifyHandler) -> Result<Self> {
		let component = Component::new(components, name.to_owned());
		let path = component.manifest_file();
		if utils::path_exists(&path) {
			return Err(Error::ComponentConflict { name: name.to_owned(), path: component.rel_manifest_file() });
		}
		let file = try!(File::create(&path)
			.map_err(|e| utils::Error::WritingFile { name: "component", path: path, error: e }));
		
		Ok(AddingComponent {
			component: Some(component),
			file: Some(file),
			temp_cfg: temp_cfg,
			notify_handler: notify_handler,
		})
	}
	pub fn add(&mut self, t: &str, relative_path: &str) -> Result<PathBuf> {
		let abs_path = self.component.as_ref().unwrap().components.prefix.path().join(relative_path);
		if utils::path_exists(&abs_path) {
			return Err(Error::ComponentConflict { name: self.component.as_ref().unwrap().name.clone(), path: relative_path.to_owned() });
		}
		try!(writeln!(self.file.as_mut().unwrap(), "{}:{}", t, relative_path)
			.map_err(|e| utils::Error::WritingFile { name: "component", path: abs_path.clone(), error: e }));
		Ok(abs_path)
	}
	pub fn add_file(&mut self, relative_path: &str) -> Result<File> {
		let path = try!(self.add("file", relative_path));
		Ok(try!(File::create(&path).map_err(|e| utils::Error::WritingFile { name: "component", path: path, error: e })))
	}
	pub fn add_dir(&mut self, relative_path: &str) -> Result<()> {
		let path = try!(self.add("file", relative_path));
		try!(utils::ensure_dir_exists("component", &path, utils::NotifyHandler::none()));
		Ok(())
	}
	pub fn commit(mut self) -> Component<'a> {
		self.component.take().unwrap()
	}
}

impl<'a> Drop for AddingComponent<'a> {
	fn drop(&mut self) {
		if let Some(ref c) = self.component {
			mem::drop(self.file.take().unwrap());
			self.notify_handler.call(Notification::RollingBack(&c.name));
			let _ = c.uninstall(self.temp_cfg, self.notify_handler.as_ref());
		}
	}
}

pub struct Component<'a> {
	components: Components<'a>,
	name: String,
}

impl<'a> Component<'a> {
	fn new(components: Components<'a>, name: String) -> Self {
		Component { components: components, name: name }
	}
	pub fn manifest_name(&self) -> String {
		format!("manifest-{}", self.name)
	}
	pub fn manifest_file(&self) -> PathBuf {
		self.components.prefix.manifest_file(&self.manifest_name())
	}
	pub fn rel_manifest_file(&self) -> String {
		self.components.prefix.rel_manifest_file(&self.manifest_name())
	}
	fn remove_item(&self, t: &str, rel_path: &str) -> Result<()> {
		let abs_path = self.components.prefix.path().join(rel_path);
		match t {
			"file" => try!(utils::remove_file("component", &abs_path)),
			"dir" => try!(utils::remove_dir("component", &abs_path, utils::NotifyHandler::none())),
			other => return Err(Error::UnknownItemType(other.to_owned(), rel_path.to_owned())),
		}
		Ok(())
	}
	pub fn uninstall(&self, temp_cfg: &temp::Cfg, notify_handler: NotifyHandler) -> Result<()> {
		let manifest = self.manifest_file();
		let content = try!(utils::read_file("component", &manifest));
		for item in content.lines() {
			if let Some(pos) = item.find(":") {
				let t = &item[0..pos];
				let rel_path = &item[(pos+1)..];
				
				ok_ntfy!(notify_handler, Notification::NonFatalError, self.remove_item(t, rel_path));
			} else {
				return Err(Error::CorruptComponent(self.name.clone()));
			}
		}
		
		try!(utils::remove_file("component", &manifest));
		
		let temp_file = try!(temp_cfg.new_file());
		let components_file = self.components.components_file();
		try!(utils::filter_file("components", &components_file, &temp_file, |c| (c != self.name)));
		try!(utils::rename_file("components", &temp_file, &components_file));
		
		Ok(())
	}
	pub fn name(&self) -> &str {
		&self.name
	}
}
