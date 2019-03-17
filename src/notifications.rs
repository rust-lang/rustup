use crate::config::Verbosity;

#[derive(Debug)]
pub enum Notification<'a> {
    Install(crate::dist::Notification<'a>),
    Utils(crate::utils::Notification<'a>),
}

impl<'a> From<crate::dist::Notification<'a>> for Notification<'a> {
    fn from(n: crate::dist::Notification<'a>) -> Notification<'a> {
        Notification::Install(n)
    }
}
impl<'a> From<crate::utils::Notification<'a>> for Notification<'a> {
    fn from(n: crate::utils::Notification<'a>) -> Notification<'a> {
        Notification::Utils(n)
    }
}

impl<'a> Notification<'a> {
    pub fn log_with_verbosity(&self, verbosity: Verbosity) {
        match self {
            Notification::Install(n) => n.log_with_verbosity(verbosity),
            Notification::Utils(n) => n.log_with_verbosity(verbosity),
        }
    }
}
