/// Disk IO abstraction for rustup.
///
/// This exists to facilitate high performance extraction even though OS's are
/// imperfect beasts. For detailed design notes see the module source.
//
// When performing IO we have a choice:
// - perform some IO in this thread
// - dispatch some or all IO to another thread
// known tradeoffs:
// NFS: network latency incurred on create, chmod, close calls
// WSLv1: Defender latency incurred on close calls; mutex shared with create calls
// Windows: Defender latency incurred on close calls
// Unix: limited open file count
// Defender : CPU limited, so more service points than cores brings no gain.
// Some machines: IO limited, more service points than cores brings more efficient
// Hello world footprint ~350MB, so around 400MB to install is considered ok.
// IO utilisation.
// All systems: dispatching to a thread has some overhead.
// Basic idea then is a locally measured congestion control problem.
// Underlying system has two
// dimensions - how much work we have queued, and how much work we execute
// at once. Queued work is both memory footprint, and unless each executor
// is performing complex logic, potentially concurrent work.
// Single core machines - thread anyway, they probably don't have SSDs?
// How many service points? Blocking latency due to networks and disks
// is independent of CPU: more threads will garner more throughput up
// to actual resource service capability.
// so:
// a) measure time around each IO op from dispatch to completion.
// b) create more threads than CPUs - 2x for now (because threadpool
//    doesn't allow creating dynamically), with very shallow stacks
//    (say 1MB)
// c) keep adding work while the P95? P80? of completion stays the same
//    when pNN starts to increase either (i) we've saturated the system
//    or (ii) other work coming in has saturated the system or (iii) this
//    sort of work is a lot harder to complete. We use NN<100 to avoid
//    having jitter throttle us inappropriately. We use a high NN to
//    avoid making the system perform poorly for the user / other users
//    on shared components. Perhaps time-to-completion should be scaled by size.
// d) if we have a lot of (iii) we should respond to it the same as (i), so
//    lets reduce this to (i) and (ii). Being unable to tell the difference
//    between load we created and others, we have to throttle back when
//    the system saturates. Our most throttled position will be one service
//    worker: dispatch IO, extract the next text, wait for IO completion,
//    repeat.
// e) scaling up and down: TCP's lessons here are pretty good. So exponential
//    up - single thread and measure. two, 4 etc. When Pnn goes bad back off
//    for a time and then try again with linear increase (it could be case (ii)
//    - lots of room to experiment here; working with a time based approach is important
//    as that is the only way we can detect saturation: we are not facing
//    loss or errors in this model.
// f) data gathering: record (name, bytes, start, duration)
//    write to disk afterwards as a csv file?
pub(crate) mod immediate;
#[cfg(test)]
mod test;
pub(crate) mod threaded;

use std::io::{self, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};
use std::{fmt::Debug, fs::OpenOptions};
use tracing::debug;

use anyhow::{Context, Result};
use sys_info;

use crate::process::Process;
use crate::utils::notifications::Notification;
use threaded::PoolReference;

/// Carries the implementation specific data for complete file transfers into the executor.
#[derive(Debug)]
pub(crate) enum FileBuffer {
    Immediate(Vec<u8>),
    // A reference to the object in the pool, and a handle to write to it
    Threaded(PoolReference),
}

impl PartialEq for FileBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl Eq for FileBuffer {}

impl PartialOrd for FileBuffer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileBuffer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl FileBuffer {
    /// All the buffers space to be re-used when the last reference to it is dropped.
    pub(crate) fn clear(&mut self) {
        if let FileBuffer::Threaded(contents) = self {
            contents.clear()
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            FileBuffer::Immediate(vec) => vec.len(),
            FileBuffer::Threaded(PoolReference::Owned(owned, _)) => owned.len(),
            FileBuffer::Threaded(PoolReference::Mut(mutable, _)) => mutable.len(),
        }
    }

    pub(crate) fn finished(self) -> Self {
        match self {
            FileBuffer::Threaded(PoolReference::Mut(mutable, pool)) => {
                FileBuffer::Threaded(PoolReference::Owned(mutable.downgrade(), pool))
            }
            _ => self,
        }
    }
}

impl Deref for FileBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        match self {
            FileBuffer::Immediate(vec) => vec,
            FileBuffer::Threaded(PoolReference::Owned(owned, _)) => owned,
            FileBuffer::Threaded(PoolReference::Mut(mutable, _)) => mutable,
        }
    }
}

impl DerefMut for FileBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            FileBuffer::Immediate(vec) => vec,
            FileBuffer::Threaded(PoolReference::Owned(_, _)) => {
                unimplemented!()
            }
            FileBuffer::Threaded(PoolReference::Mut(mutable, _)) => mutable,
        }
    }
}

pub(crate) const IO_CHUNK_SIZE: usize = 16_777_216;

/// Carries the implementation specific channel data into the executor.
#[derive(Debug)]
pub(crate) enum IncrementalFile {
    ImmediateReceiver,
    ThreadedReceiver(Receiver<FileBuffer>),
}

impl PartialEq for IncrementalFile {
    fn eq(&self, other: &Self) -> bool {
        // Just compare discriminants since Receiver cannot be compared
        matches!(
            (self, other),
            (Self::ImmediateReceiver, Self::ImmediateReceiver) |
            (Self::ThreadedReceiver(_), Self::ThreadedReceiver(_))
        )
    }
}

impl Eq for IncrementalFile {}

impl PartialOrd for IncrementalFile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for IncrementalFile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // ImmediateReceiver is "less than" ThreadedReceiver
        match (self, other) {
            (Self::ImmediateReceiver, Self::ImmediateReceiver) => std::cmp::Ordering::Equal,
            (Self::ImmediateReceiver, Self::ThreadedReceiver(_)) => std::cmp::Ordering::Less,
            (Self::ThreadedReceiver(_), Self::ImmediateReceiver) => std::cmp::Ordering::Greater,
            (Self::ThreadedReceiver(_), Self::ThreadedReceiver(_)) => std::cmp::Ordering::Equal,
        }
    }
}

// The basic idea is that in single threaded mode we get this pattern:
// package budget io-layer
// +<-claim->
// +-submit--------+ | write
// +-complete------+
// +<reclaim>
// .. loop ..
// In thread mode with lots of memory we want the following:
// +<-claim->
// +-submit--------+
// +<-claim->
// +-submit--------+
// .. loop .. | writes
// +-complete------+
// +<reclaim>
// +-complete------+
// +<reclaim>
// In thread mode with limited memory we want the following:
// +<-claim->
// +-submit--------+
// +<-claim->
// +-submit--------+
// .. loop up to budget .. | writes
// +-complete------+
// +<reclaim>
// +<-claim->
// +-submit--------+
// .. loop etc ..
//
// lastly we want pending IOs such as directory creation to be able to complete in the same way, so a chunk completion
// needs to be able to report back in the same fashion; folding it into the same enum will make the driver code easier to write.
//
// The implementation is done via a pair of MPSC channels. One to send data to write. In
// the immediate model, acknowledgements are sent after doing the write immediately. In the threaded model,
// acknowledgements are sent after the write completes in the thread pool handler. In the packages code the inner that
// handles iops and continues processing incremental mode files handles the connection between the acks and the budget.
// Error reporting is passed through the regular completion port, to avoid creating a new special case.

/// What kind of IO operation to perform
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Kind {
    Directory,
    File(FileBuffer),
    IncrementalFile(IncrementalFile),
}

/// Priority level for I/O operations
/// Higher values indicate higher priority
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IOPriority {
    Critical,
    Normal,
    Background,
}

impl Default for IOPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// The details of the IO operation
#[derive(Debug)]
pub(crate) struct Item {
    /// The path to operate on
    pub(crate) full_path: PathBuf,
    /// The operation to perform
    pub(crate) kind: Kind,
    /// When the operation started
    start: Option<Instant>,
    /// Amount of time the operation took to finish
    finish: Option<Duration>,
    /// The result of the operation (could now be factored into CompletedIO...)
    pub(crate) result: io::Result<()>,
    /// The mode to apply
    mode: u32,
    /// Priority of this operation
    priority: IOPriority,
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.full_path == other.full_path
    }
}

impl Eq for Item {}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by priority first (higher priority comes first)
        match other.priority.cmp(&self.priority) {
            std::cmp::Ordering::Equal => self.full_path.cmp(&other.full_path),
            ordering => ordering,
        }
    }
}

#[derive(Debug)]
pub(crate) enum CompletedIo {
    /// A submitted Item has completed
    Item(Item),
    /// An IncrementalFile has completed a single chunk
    #[allow(dead_code)] // chunk size only used in test code
    Chunk(usize),
}

impl Item {
    pub(crate) fn make_dir(full_path: PathBuf, mode: u32) -> Self {
        Self {
            full_path,
            kind: Kind::Directory,
            start: None,
            finish: None,
            result: Ok(()),
            mode,
            priority: IOPriority::default(),
        }
    }

    pub(crate) fn write_file(full_path: PathBuf, mode: u32, content: FileBuffer) -> Self {
        Self {
            full_path,
            kind: Kind::File(content),
            start: None,
            finish: None,
            result: Ok(()),
            mode,
            priority: IOPriority::default(),
        }
    }

    pub(crate) fn write_file_segmented<'a>(
        full_path: PathBuf,
        mode: u32,
        state: IncrementalFileState,
    ) -> Result<(Self, Box<dyn FnMut(FileBuffer) -> bool + 'a>)> {
        let (chunk_submit, content_callback) = state.incremental_file_channel(&full_path, mode)?;
        let result = Self {
            full_path,
            kind: Kind::IncrementalFile(content_callback),
            start: None,
            finish: None,
            result: Ok(()),
            mode,
            priority: IOPriority::default(),
        };
        Ok((result, Box::new(chunk_submit)))
    }

    /// Set the priority of this I/O operation
    /// remove for now
    #[allow(dead_code)]
    pub(crate) fn with_priority(mut self, priority: IOPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Get the priority of this I/O operation
    pub(crate) fn priority(&self) -> IOPriority {
        self.priority
    }
}

// This could be a boxed trait object perhaps... but since we're looking at
// rewriting this all into an aio layer anyway, and not looking at plugging
// different backends in at this time, it can keep.
/// Implementation specific state for incremental file writes. This effectively
/// just allows the immediate codepath to get access to the Arc referenced state
/// without holding a lifetime reference to the executor, as the threaded code
/// path is all message passing.
pub(crate) enum IncrementalFileState {
    Threaded,
    Immediate(immediate::IncrementalFileState),
}

impl IncrementalFileState {
    /// Get a channel for submitting incremental file chunks to the executor
    fn incremental_file_channel(
        &self,
        path: &Path,
        mode: u32,
    ) -> Result<(Box<dyn FnMut(FileBuffer) -> bool>, IncrementalFile)> {
        use std::sync::mpsc::channel;
        match *self {
            IncrementalFileState::Threaded => {
                let (tx, rx) = channel::<FileBuffer>();
                let content_callback = IncrementalFile::ThreadedReceiver(rx);
                let chunk_submit = move |chunk: FileBuffer| tx.send(chunk).is_ok();
                Ok((Box::new(chunk_submit), content_callback))
            }
            IncrementalFileState::Immediate(ref state) => {
                let content_callback = IncrementalFile::ImmediateReceiver;
                let mut writer = immediate::IncrementalFileWriter::new(path, mode, state.clone())?;
                let chunk_submit = move |chunk: FileBuffer| writer.chunk_submit(chunk);
                Ok((Box::new(chunk_submit), content_callback))
            }
        }
    }
}

/// Trait object for performing IO. At this point the overhead
/// of trait invocation is not a bottleneck, but if it becomes
/// one we could consider an enum variant based approach instead.
pub(crate) trait Executor {
    /// Perform a single operation.
    /// During overload situations previously queued items may
    /// need to be completed before the item is accepted:
    /// consume the returned iterator.
    fn execute(&self, mut item: Item) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        item.start = Some(Instant::now());
        self.dispatch(item)
    }

    /// Actually dispatch an operation.
    /// This is called by the default execute() implementation and
    /// should not be called directly.
    fn dispatch(&self, item: Item) -> Box<dyn Iterator<Item = CompletedIo> + '_>;

    /// Wrap up any pending operations and iterate over them.
    /// All operations submitted before the join will have been
    /// returned either through ready/complete or join once join
    /// returns.
    fn join(&mut self) -> Box<dyn Iterator<Item = CompletedIo> + '_>;

    /// Iterate over completed items.
    fn completed(&self) -> Box<dyn Iterator<Item = CompletedIo> + '_>;

    /// Get any state needed for incremental file processing
    fn incremental_file_state(&self) -> IncrementalFileState;

    /// Get a disk buffer E.g. this gets the right sized pool object for
    /// optimized situations, or just a malloc when optimisations are off etc
    /// etc.
    fn get_buffer(&mut self, len: usize) -> FileBuffer;

    /// Query the memory budget to see if a particular size buffer is available
    fn buffer_available(&self, len: usize) -> bool;

    #[cfg(test)]
    /// Query the memory budget to see how much of the buffer pool is in use
    fn buffer_used(&self) -> usize;
}

/// Trivial single threaded IO to be used from executors.
/// (Crazy sophisticated ones can obviously ignore this)
pub(crate) fn perform<F: Fn(usize)>(item: &mut Item, chunk_complete_callback: F) {
    // directories: make them, TODO: register with the dir existence cache.
    // Files, write them.
    item.result = match &mut item.kind {
        Kind::Directory => create_dir(&item.full_path),
        Kind::File(contents) => {
            contents.clear();
            match contents {
                FileBuffer::Immediate(contents) => write_file(&item.full_path, contents, item.mode),
                FileBuffer::Threaded(contents) => write_file(&item.full_path, contents, item.mode),
            }
        }
        Kind::IncrementalFile(incremental_file) => write_file_incremental(
            &item.full_path,
            incremental_file,
            item.mode,
            chunk_complete_callback,
        ),
    };
    item.finish = item
        .start
        .map(|s| Instant::now().saturating_duration_since(s));
}

#[allow(unused_variables)]
pub(crate) fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(
    path: P,
    contents: C,
    mode: u32,
) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    let path = path.as_ref();
    let path_display = format!("{}", path.display());
    let mut f = {
        trace_scoped!("creat", "name": path_display);
        opts.write(true).create(true).truncate(true).open(path)?
    };
    let contents = contents.as_ref();
    let len = contents.len();
    {
        trace_scoped!("write", "name": path_display, "len": len);
        f.write_all(contents)?;
    }
    {
        trace_scoped!("close", "name:": path_display);
        drop(f);
    }
    Ok(())
}

#[allow(unused_variables)]
pub(crate) fn write_file_incremental<P: AsRef<Path>, F: Fn(usize)>(
    path: P,
    content_callback: &mut IncrementalFile,
    mode: u32,
    chunk_complete_callback: F,
) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    let path = path.as_ref();
    let path_display = format!("{}", path.display());
    let mut f = {
        trace_scoped!("creat", "name": path_display);
        opts.write(true).create(true).truncate(true).open(path)?
    };
    if let IncrementalFile::ThreadedReceiver(recv) = content_callback {
        loop {
            // We unwrap here because the documented only reason for recv to fail is a close by the sender, which is reading
            // from the tar file: a failed read there will propagate the error in the main thread directly.
            let contents = recv.recv().unwrap();
            let len = contents.len();
            // Length 0 vector is used for clean EOF signalling.
            if len == 0 {
                trace_scoped!("EOF_chunk", "name": path_display, "len": len);
                drop(contents);
                chunk_complete_callback(len);
                break;
            } else {
                trace_scoped!("write_segment", "name": path_display, "len": len);
                f.write_all(&contents)?;
                drop(contents);
                chunk_complete_callback(len);
            }
        }
    } else {
        unreachable!();
    }
    {
        trace_scoped!("close", "name:": path_display);
        drop(f);
    }
    Ok(())
}

pub(crate) fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = path.as_ref();
    let path_display = format!("{}", path.display());
    trace_scoped!("create_dir", "name": path_display);
    std::fs::create_dir(path)
}

/// Get the executor for disk IO.
pub(crate) fn get_executor<'a>(
    notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    ram_budget: usize,
    process: &Process,
) -> Result<Box<dyn Executor + 'a>> {
    // Calculate optimal thread count based on system characteristics
    // Default is CPU count for CPU-bound systems, or 2x CPU count for I/O-bound operations
    let default_thread_count = available_parallelism()
        .map(|p| {
            let cpu_count = p.get();
            // Use more threads for I/O bound operations to hide latency
            // but cap it to avoid too much overhead
            std::cmp::min(cpu_count * 2, 16)
        })
        .unwrap_or(2);

    // If this gets lots of use, consider exposing via the config file.
    let thread_count = match process.var("RUSTUP_IO_THREADS") {
        Err(_) => default_thread_count,
        Ok(n) => n
            .parse::<usize>()
            .context("invalid value in RUSTUP_IO_THREADS. Must be a natural number")?,
    };

    // Calculate optimal memory budget based on system memory
    // Default to 10% of system memory, or fallback to 256MB
    let default_ram_budget = if ram_budget == 0 {
        match sys_info::mem_info() {
            Ok(mem) => {
                let total_mem = mem.total as usize * 1024; // Convert to bytes
                total_mem / 10 // Use 10% of system memory
            }
            Err(_) => 256 * 1024 * 1024, // Fallback to 256MB
        }
    } else {
        ram_budget
    };

    // Allow overriding the memory budget via environment variable (maybe keep this but useful for testing on different systems right now)
    let actual_ram_budget = match process.var("RUSTUP_RAM_BUDGET") {
        Err(_) => default_ram_budget,
        Ok(n) => n
            .parse::<usize>()
            .context("invalid value in RUSTUP_RAM_BUDGET. Must be in bytes")?,
    };

    // Log the chosen configuration for debugging
    debug!(
        "Using IO executor with thread_count={} and ram_budget={}MB",
        thread_count,
        actual_ram_budget / (1024 * 1024)
    );

    Ok(match thread_count {
        0 | 1 => Box::new(immediate::ImmediateUnpacker::new()),
        n => Box::new(threaded::Threaded::new(notify_handler, n, actual_ram_budget)),
    })
}
