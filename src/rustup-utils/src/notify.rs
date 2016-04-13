//! Notification handlers.
//!
//! These are used to bubble up information about what is happening
//! inside the program. They are composed as error types commonly are.
//!
//! The major types:
//!
//! * `Notifyable<N>`. Types implementing this can receive
//!   notifications of `N` through the `call` method.  These are
//!   passed around as trait objects. Closures over `N` automatically
//!   implement `Notifyable<N>`.
//! * `NotifyHandler<'a, N>(Option<&N>)`. A wrapper around
//!   `Notifyable` objects, passed as values.  Functions that need to
//!   emit notifications take these by value.
//! * `Notification`. By convention, the name of an enum in each
//!   module that defines the notifications that module emits.
//!
//! Notifications are composed in a hierarchy. For example, this crate
//! defines `temp::Notification`, and `utils::Notification`, and
//! futher defines `errors::Notification`, the top-level notification
//! enum which encapsulates both in the variants
//! `Notification::Temp(temp::Notification)` and
//! `Notification::Utils(utils::Notification)`.
//!
//! A `NotifyHandler<_, error::Notification>` can be turned into a
//! `NotifyHandler<_, temp::Notification>` by calling
//! `temp::NotifyHandler::some(&my_error_notify_handler)`. This works
//! because `NotifyHandler<_, error::Notification>` implements
//! `Notifyable<temp::Notification>` via the `extend_notification!`
//! macro.

use std::fmt;
use std::intrinsics::type_name;
use std::sync::Arc;

#[fundamental]
pub trait Notifyable<N> {
    fn call(&self, n: N);
}

impl<N, F: ?Sized + Fn(N)> Notifyable<N> for F {
    fn call(&self, n: N) {
        self(n)
    }
}

#[fundamental]
pub struct NotifyHandler<'a, T: 'a + ?Sized>(Option<&'a T>);

impl<'a, T: 'a + ?Sized> fmt::Debug for NotifyHandler<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
	write!(f, "NotifyHandler<{}>", unsafe { type_name::<T>() })
    }
}

impl<'a, T: 'a + ?Sized> Copy for NotifyHandler<'a, T> {}
impl<'a, T: 'a + ?Sized> Clone for NotifyHandler<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[fundamental]
pub struct SharedNotifyHandler<T: ?Sized>(Option<Arc<T>>);

impl<T: ?Sized> Clone for SharedNotifyHandler<T> {
    fn clone(&self) -> Self {
        SharedNotifyHandler(self.0.clone())
    }
}

impl<T: ?Sized> fmt::Debug for SharedNotifyHandler<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
	write!(f, "SharedNotifyHandler<{}>", unsafe { type_name::<T>() })
    }
}

impl<'a, T: 'a + ?Sized> NotifyHandler<'a, T> {
    pub fn some(arg: &'a T) -> Self {
        NotifyHandler(Some(arg))
    }
    pub fn none() -> Self {
        NotifyHandler(None)
    }
    pub fn call<U>(&self, arg: U)
        where T: Notifyable<U>
    {
        if let Some(f) = self.0 {
            f.call(arg);
        }
    }
}

impl<T: ?Sized> SharedNotifyHandler<T> {
    pub fn some(arg: Arc<T>) -> Self {
        SharedNotifyHandler(Some(arg))
    }
    pub fn none() -> Self {
        SharedNotifyHandler(None)
    }
    pub fn as_ref<'a>(&'a self) -> NotifyHandler<'a, T> {
        match self.0 {
            Some(ref f) => NotifyHandler(Some(f)),
            None => NotifyHandler(None),
        }
    }
    pub fn call<U>(&self, arg: U)
        where T: Notifyable<U>
    {
        self.as_ref().call(arg)
    }
}

#[derive(Debug)]
pub enum NotificationLevel {
    Verbose,
    Info,
    Warn,
    Error,
}
#[macro_export]
macro_rules! extend_error {
    ($cur:ty: $base:ty, $p:ident => $e:expr) => (
        impl From<$base> for $cur {
            fn from($p: $base) -> $cur {
                $e
            }
        }
    )
}
#[macro_export]
macro_rules! extend_notification {
    ($( $cur:ident )::*: $( $base:ident )::*, $p:ident => $e:expr) => (
        impl<'a, 'b> $crate::notify::Notifyable<$($base)::*<'a>> for $crate::notify::NotifyHandler<'b, for<'c> $crate::notify::Notifyable<$($cur)::*<'c>>> {
            fn call(&self, $p: $($base)::*<'a>) {
                self.call($e)
            }
        }
        impl<'a> $crate::notify::Notifyable<$($base)::*<'a>> for $crate::notify::SharedNotifyHandler<for<'b> $crate::notify::Notifyable<$($cur)::*<'b>>> {
            fn call(&self, $p: $($base)::*<'a>) {
                self.call($e)
            }
        }
    )
}
#[macro_export]
macro_rules! declare_notification {
    ($( $cur:ident )::*: $( $base:ident )::*, $p:ident => $e:expr) => (
        impl<'a, 'b> $crate::notify::Notifyable<$($base)::*<'a>> for $crate::notify::NotifyHandler<'b, for<'c> $crate::notify::Notifyable<$($cur)::*<'c>>> {
            fn call(&self, $p: $($base)::*<'a>) {
                self.call($e)
            }
        }
        impl<'a> $crate::notify::Notifyable<$($base)::*<'a>> for $crate::notify::SharedNotifyHandler<for<'b> $crate::notify::Notifyable<$($cur)::*<'b>>> {
            fn call(&self, $p: $($base)::*<'a>) {
                self.call($e)
            }
        }
    )
}
#[macro_export]
macro_rules! ntfy {
    ($e:expr) => (
        $crate::notify::NotifyHandler::some($e)
    )
}
#[macro_export]
macro_rules! shared_ntfy {
    ($e:expr) => (
        $crate::notify::SharedNotifyHandler::some(::std::sync::Arc::new($e))
    )
}
#[macro_export]
macro_rules! ok_ntfy {
    ($n:expr, $w:path, $e:expr) => (
        match $e {
            Err(e) => { $n.call($w(&e)); None },
            Ok(r) => Some(r)
        }
    )
}
