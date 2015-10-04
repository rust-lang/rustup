use std::ops::CoerceUnsized;
use std::sync::Arc;

#[derive(Debug)]
pub struct NotifyHandler<T: ?Sized>(Option<Arc<T>>);

impl<T: ?Sized> Clone for NotifyHandler<T> {
	fn clone(&self) -> Self {
		NotifyHandler(self.0.clone())
	}
}

impl<T: ?Sized> NotifyHandler<T> {
	pub fn from<F>(f: F) -> Self where Arc<F>: CoerceUnsized<Arc<T>> {
		let fb: Arc<F> = Arc::new(f);
		NotifyHandler(Some(fb))
	}
	pub fn none() -> Self {
		NotifyHandler(None)
	}
	pub fn call<U>(&self, arg: U) where T: Fn(U) {
		if let Some(ref f) = self.0 {
			f(arg);
		}
	}
}

pub enum NotificationLevel {
	Verbose,
	Normal,
	Info,
	Warn,
}
