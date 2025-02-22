/// Immediate IO model: performs IO in the current thread.
///
/// Use for diagnosing bugs or working around any unexpected issues with the
/// threaded code paths.
use std::{
    fmt::Debug,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use super::{CompletedIo, Executor, FileBuffer, Item};

#[derive(Debug)]
pub(crate) struct _IncrementalFileState {
    completed_chunks: Vec<usize>,
    err: Option<io::Result<()>>,
    item: Option<Item>,
    finished: bool,
}

pub(super) type IncrementalFileState = Arc<Mutex<Option<_IncrementalFileState>>>;

#[derive(Default, Debug)]
pub(crate) struct ImmediateUnpacker {
    incremental_state: IncrementalFileState,
}

impl ImmediateUnpacker {
    pub(crate) fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    fn deque(&self) -> Box<dyn Iterator<Item = CompletedIo>> {
        let mut guard = self.incremental_state.lock().unwrap();
        // incremental file in progress
        if let Some(ref mut state) = *guard {
            // Case 1: pending errors
            if state.finished {
                let mut item = state.item.take().unwrap();
                if state.err.is_some() {
                    let err = state.err.take().unwrap();
                    item.result = err;
                }
                item.finish = item
                    .start
                    .map(|s| Instant::now().saturating_duration_since(s));
                if state.finished {
                    *guard = None;
                }
                Box::new(Some(CompletedIo::Item(item)).into_iter())
            } else {
                // Case 2: pending chunks (which might be empty)
                let mut completed_chunks = vec![];
                completed_chunks.append(&mut state.completed_chunks);
                Box::new(completed_chunks.into_iter().map(CompletedIo::Chunk))
            }
        } else {
            Box::new(None.into_iter())
        }
    }
}

impl Executor for ImmediateUnpacker {
    fn dispatch(&self, mut item: Item) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        item.result = match &mut item.kind {
            super::Kind::Directory => super::create_dir(&item.full_path),
            super::Kind::File(contents) => {
                if let super::FileBuffer::Immediate(contents) = &contents {
                    super::write_file(&item.full_path, contents, item.mode)
                } else {
                    unreachable!()
                }
            }
            super::Kind::IncrementalFile(_incremental_file) => {
                return {
                    // If there is a pending error, return it, otherwise stash the
                    // Item for eventual return when the file is finished.
                    let mut guard = self.incremental_state.lock().unwrap();
                    let Some(ref mut state) = *guard else {
                        unreachable!()
                    };
                    if state.err.is_some() {
                        let err = state.err.take().unwrap();
                        item.result = err;
                        item.finish = item
                            .start
                            .map(|s| Instant::now().saturating_duration_since(s));
                        *guard = None;
                        Box::new(Some(CompletedIo::Item(item)).into_iter())
                    } else {
                        state.item = Some(item);
                        Box::new(None.into_iter())
                    }
                };
            }
        };
        item.finish = item
            .start
            .map(|s| Instant::now().saturating_duration_since(s));
        Box::new(Some(CompletedIo::Item(item)).into_iter())
    }

    fn join(&mut self) -> Box<dyn Iterator<Item = CompletedIo>> {
        self.deque()
    }

    fn completed(&self) -> Box<dyn Iterator<Item = CompletedIo>> {
        self.deque()
    }

    fn incremental_file_state(&self) -> super::IncrementalFileState {
        let mut state = self.incremental_state.lock().unwrap();
        if state.is_some() {
            unreachable!();
        } else {
            *state = Some(_IncrementalFileState {
                completed_chunks: vec![],
                err: None,
                item: None,
                finished: false,
            });
            super::IncrementalFileState::Immediate(self.incremental_state.clone())
        }
    }

    fn get_buffer(&mut self, capacity: usize) -> super::FileBuffer {
        super::FileBuffer::Immediate(Vec::with_capacity(capacity))
    }

    fn buffer_available(&self, _len: usize) -> bool {
        true
    }

    #[cfg(test)]
    fn buffer_used(&self) -> usize {
        0
    }
}

/// The non-shared state for writing a file incrementally
#[derive(Debug)]
pub(super) struct IncrementalFileWriter {
    state: IncrementalFileState,
    file: Option<File>,
    path_display: String,
}

impl IncrementalFileWriter {
    #[allow(unused_variables)]
    pub(crate) fn new<P: AsRef<Path>>(
        path: P,
        mode: u32,
        state: IncrementalFileState,
    ) -> std::result::Result<Self, io::Error> {
        let mut opts = OpenOptions::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(mode);
        }
        let path = path.as_ref();
        let path_display = format!("{}", path.display());
        let file = Some({
            trace_scoped!("creat", "name": path_display);
            opts.write(true).create(true).truncate(true).open(path)?
        });
        Ok(IncrementalFileWriter {
            state,
            file,
            path_display,
        })
    }

    pub(crate) fn chunk_submit(&mut self, chunk: FileBuffer) -> bool {
        if (self.state.lock().unwrap()).is_none() {
            return false;
        }
        let FileBuffer::Immediate(chunk) = chunk else {
            unreachable!()
        };
        match self.write(chunk) {
            Ok(v) => v,
            Err(e) => {
                let mut state = self.state.lock().unwrap();
                if let Some(ref mut state) = *state {
                    state.err.replace(Err(e));
                    state.finished = true;
                    false
                } else {
                    false
                }
            }
        }
    }

    fn write(&mut self, chunk: Vec<u8>) -> std::result::Result<bool, io::Error> {
        let mut state = self.state.lock().unwrap();
        let Some(ref mut state) = *state else {
            unreachable!()
        };
        let Some(ref mut file) = self.file.as_mut() else {
            return Ok(false);
        };
        // Length 0 vector is used for clean EOF signalling.
        if chunk.is_empty() {
            trace_scoped!("close", "name:": self.path_display);
            drop(std::mem::take(&mut self.file));
            state.finished = true;
        } else {
            trace_scoped!("write_segment", "name": self.path_display, "len": chunk.len());
            file.write_all(&chunk)?;

            state.completed_chunks.push(chunk.len());
        }
        Ok(true)
    }
}
