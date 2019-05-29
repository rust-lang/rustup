/// Immediate IO model: performs IO in the current thread.
///
/// Use for diagnosing bugs or working around any unexpected issues with the
/// threaded code paths.
use super::{perform, Executor, Item};

use std::cell::Cell;

pub struct ImmediateUnpacker {}
impl ImmediateUnpacker {
    pub fn new<'a>() -> ImmediateUnpacker {
        ImmediateUnpacker {}
    }
}

enum IterateOne {
    Item(Item),
    None,
}

impl Default for IterateOne {
    fn default() -> Self {
        IterateOne::None
    }
}

struct ImmediateIterator(Cell<IterateOne>);

impl Iterator for ImmediateIterator {
    type Item = Item;
    fn next(&mut self) -> Option<Item> {
        match self.0.take() {
            IterateOne::Item(item) => Some(item),
            IterateOne::None => None,
        }
    }
}

impl Executor for ImmediateUnpacker {
    fn dispatch(&mut self, mut item: Item) -> Box<dyn '_ + Iterator<Item = Item>> {
        perform(&mut item);
        Box::new(ImmediateIterator(Cell::new(IterateOne::Item(item))))
    }

    fn join(&mut self) -> Option<Box<dyn Iterator<Item = Item>>> {
        None
    }

    fn completed(&mut self) -> Option<Box<dyn Iterator<Item = Item>>> {
        None
    }
}
