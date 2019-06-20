/// Immediate IO model: performs IO in the current thread.
///
/// Use for diagnosing bugs or working around any unexpected issues with the
/// threaded code paths.
use super::{perform, Executor, Item};

pub struct ImmediateUnpacker {}
impl ImmediateUnpacker {
    pub fn new() -> ImmediateUnpacker {
        ImmediateUnpacker {}
    }
}

impl Executor for ImmediateUnpacker {
    fn dispatch(&mut self, mut item: Item) -> Box<dyn '_ + Iterator<Item = Item>> {
        perform(&mut item);
        Box::new(Some(item).into_iter())
    }

    fn join(&mut self) -> Box<dyn Iterator<Item = Item>> {
        Box::new(None.into_iter())
    }

    fn completed(&mut self) -> Box<dyn Iterator<Item = Item>> {
        Box::new(None.into_iter())
    }
}
