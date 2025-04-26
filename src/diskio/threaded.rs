/// Threaded IO model: A pool of threads is used so that syscall latencies
/// due to (nonexhaustive list) Network file systems, virus scanners, and
/// operating system design, do not cause rustup to be significantly slower
/// than desired. In particular the docs workload with 20K files requires
/// very low latency per file, which even a few ms per syscall per file
/// will cause minutes of wall clock time.
use std::cell::{Cell, RefCell};
use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};
use enum_map::{Enum, EnumMap, enum_map};
use sharded_slab::pool::{OwnedRef, OwnedRefMut};
use tracing::{debug, info};

use super::{CompletedIo, Executor, Item, perform, IOPriority};
use crate::utils::notifications::Notification;
use crate::utils::units::Unit;

#[derive(Copy, Clone, Debug, Enum)]
pub(crate) enum Bucket {
    FourK,
    EightK,
    OneM,
    EightM,
    SixteenM,
}

#[derive(Debug)]
pub(crate) enum PoolReference {
    Owned(OwnedRef<Vec<u8>>, Arc<sharded_slab::Pool<Vec<u8>>>),
    Mut(OwnedRefMut<Vec<u8>>, Arc<sharded_slab::Pool<Vec<u8>>>),
}

impl PoolReference {
    pub(crate) fn clear(&mut self) {
        match self {
            PoolReference::Mut(orm, pool) => {
                pool.clear(orm.key());
            }
            PoolReference::Owned(rm, pool) => {
                pool.clear(rm.key());
            }
        }
    }
}

impl AsRef<[u8]> for PoolReference {
    fn as_ref(&self) -> &[u8] {
        match self {
            PoolReference::Owned(owned, _) => owned,
            PoolReference::Mut(mutable, _) => mutable,
        }
    }
}

enum Task {
    Request(CompletedIo),
    // Used to synchronise in the join method.
    Sentinel,
}

impl Default for Task {
    fn default() -> Self {
        Self::Sentinel
    }
}

struct Pool {
    pool: Arc<sharded_slab::Pool<Vec<u8>>>,
    high_watermark: RefCell<usize>,
    in_use: RefCell<usize>,
    size: usize,
}

impl Pool {
    fn claim(&self) {
        if self.in_use == self.high_watermark {
            *self.high_watermark.borrow_mut() += self.size;
        }
        *self.in_use.borrow_mut() += self.size;
    }

    fn reclaim(&self) {
        *self.in_use.borrow_mut() -= self.size;
    }
}

impl fmt::Debug for Pool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pool")
            .field("size", &self.size)
            .field("in_use", &self.in_use)
            .field("high_watermark", &self.high_watermark)
            .finish()
    }
}

pub(crate) struct Threaded<'a> {
    n_files: Arc<AtomicUsize>,
    pool: threadpool::ThreadPool,
    notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    rx: Receiver<Task>,
    tx: Sender<Task>,
    vec_pools: EnumMap<Bucket, Pool>,
    ram_budget: usize,
    last_adjustment: Instant,
    operation_times: RefCell<Vec<Duration>>,
    thread_count: usize,
    pending_items: RefCell<BinaryHeap<(IOPriority, Instant, Item)>>,
}

impl<'a> Threaded<'a> {
    /// Construct a new Threaded executor.
    pub(crate) fn new(
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
        thread_count: usize,
        ram_budget: usize,
    ) -> Self {
        let pool = threadpool::Builder::new()
            .thread_name("CloseHandle".into())
            .num_threads(thread_count)
            .thread_stack_size(1_048_576)
            .build();
        let (tx, rx) = channel();
        let vec_pools = enum_map! {
            Bucket::FourK => Pool{
                pool: Arc::new(sharded_slab::Pool::new()),
                high_watermark: RefCell::new(4096),
                in_use: RefCell::new(0),
                size:4096
            },
            Bucket::EightK=> Pool{
                pool: Arc::new(sharded_slab::Pool::new()),
                high_watermark: RefCell::new(8192),
                in_use: RefCell::new(0),
                size:8192
            },
            Bucket::OneM=> Pool{
                pool: Arc::new(sharded_slab::Pool::new()),
                high_watermark: RefCell::new(1024*1024),
                in_use: RefCell::new(0),
                size:1024*1024
            },
            Bucket::EightM=> Pool{
                pool: Arc::new(sharded_slab::Pool::new()),
                high_watermark: RefCell::new(8*1024*1024),
                in_use: RefCell::new(0),
                size:8*1024*1024
            },
            Bucket::SixteenM=> Pool{
                pool: Arc::new(sharded_slab::Pool::new()),
                high_watermark: RefCell::new(16*1024*1024),
                in_use: RefCell::new(0),
                size: 16*1024*1024
            },
        };
        for (_, pool) in &vec_pools {
            let key = pool
                .pool
                .create_with(|vec| vec.reserve_exact(pool.size - vec.len()))
                .unwrap();
            pool.pool.clear(key);
        }
        assert!(Threaded::ram_highwater(&vec_pools) < ram_budget);
        Self {
            n_files: Arc::new(AtomicUsize::new(0)),
            pool,
            notify_handler,
            rx,
            tx,
            vec_pools,
            ram_budget,
            last_adjustment: Instant::now(),
            operation_times: RefCell::new(Vec::with_capacity(100)),
            thread_count,
            pending_items: RefCell::new(BinaryHeap::new()),
        }
    }

    /// How much RAM is allocated across all the pools right now
    fn ram_highwater(vec_pools: &EnumMap<Bucket, Pool>) -> usize {
        vec_pools
            .iter()
            .map(|(_, pool)| *pool.high_watermark.borrow())
            .sum()
    }

    fn reclaim(&self, op: &CompletedIo) {
        let size = match &op {
            CompletedIo::Item(op) => match &op.kind {
                super::Kind::Directory => return,
                super::Kind::File(content) => content.len(),
                super::Kind::IncrementalFile(_) => return,
            },
            CompletedIo::Chunk(_) => super::IO_CHUNK_SIZE,
        };
        let bucket = self.find_bucket(size);
        let pool = &self.vec_pools[bucket];
        pool.reclaim();
    }

    fn submit(&self, mut item: Item) {
        let tx = self.tx.clone();
        self.n_files.fetch_add(1, Ordering::Relaxed);
        let n_files = self.n_files.clone();
        let start_time = Instant::now();
        let operation_times = self.operation_times.clone();
        let priority = item.priority();

        self.pool.execute(move || {
            let chunk_complete_callback = |size| {
                tx.send(Task::Request(CompletedIo::Chunk(size)))
                    .expect("receiver should be listening")
            };
            perform(&mut item, chunk_complete_callback);
            n_files.fetch_sub(1, Ordering::Relaxed);

            let elapsed = start_time.elapsed();
            if let Ok(mut times) = operation_times.try_borrow_mut() {
                if times.len() < 100 {
                    times.push(elapsed);
                } else {
                    times.remove(0);
                    times.push(elapsed);
                }
            }

            debug!("Completed {:?} priority operation in {:?}", priority, elapsed);

            tx.send(Task::Request(CompletedIo::Item(item)))
                .expect("receiver should be listening");
        });
    }

    /// Queue an item for later processing
    fn queue_item(&self, item: Item) {
        let priority = item.priority();
        let timestamp = Instant::now();
        let mut queue = self.pending_items.borrow_mut();
        queue.push((priority, timestamp, item));
    }

    /// Process queued items with respect to priority
    fn process_queued_items(&self) -> bool {
        if self.pool.queued_count() >= self.thread_count {
            return false;
        }

        let mut queue = self.pending_items.borrow_mut();
        if queue.is_empty() {
            return false;
        }

        if let Some((priority, timestamp, item)) = queue.pop() {
            let wait_time = timestamp.elapsed();
            if wait_time > Duration::from_millis(100) {
                debug!("Processing {:?} priority item after waiting {:?}", priority, wait_time);
            }
            self.submit(item);
            true
        } else {
            false
        }
    }

    fn maybe_adjust_thread_count(&mut self) {
        const ADJUSTMENT_INTERVAL: Duration = Duration::from_secs(5);

        if self.last_adjustment.elapsed() < ADJUSTMENT_INTERVAL {
            return;
        }

        let mut times = self.operation_times.borrow_mut();
        if times.len() < 10 {
            return;
        }

        times.sort();
        let median = times[times.len() / 2];
        let p95_idx = (times.len() as f32 * 0.95) as usize;
        let p95 = times[p95_idx.min(times.len() - 1)];

        let io_bound_factor = p95.as_millis() as f32 / median.as_millis().max(1) as f32;

        let new_thread_count = if io_bound_factor > 3.0 {
            let new_count = (self.thread_count + 2).min(32);
            if new_count != self.thread_count {
                debug!(
                    "I/O bound detected (factor: {:.2}), increasing threads from {} to {}",
                    io_bound_factor, self.thread_count, new_count
                );
            }
            new_count
        } else if io_bound_factor < 1.5 && self.thread_count > 2 {
            let new_count = (self.thread_count - 1).max(2);
            if new_count != self.thread_count {
                debug!(
                    "Low I/O variability (factor: {:.2}), decreasing threads from {} to {}",
                    io_bound_factor, self.thread_count, new_count
                );
            }
            new_count
        } else {
            self.thread_count
        };

        if new_thread_count != self.thread_count {
            self.pool.join();

            self.thread_count = new_thread_count;
            self.pool = threadpool::Builder::new()
                .thread_name("CloseHandle".into())
                .num_threads(new_thread_count)
                .thread_stack_size(1_048_576)
                .build();

            info!("Adjusted thread pool to {} threads", new_thread_count);
        }

        times.clear();
        self.last_adjustment = Instant::now();
    }

    fn find_bucket(&self, capacity: usize) -> Bucket {
        let mut bucket = Bucket::FourK;
        for (next_bucket, pool) in &self.vec_pools {
            bucket = next_bucket;
            if pool.size >= capacity {
                break;
            }
        }
        let pool = &self.vec_pools[bucket];
        assert!(
            capacity <= pool.size,
            "capacity <= pool.size: {} > {}",
            capacity,
            pool.size
        );
        bucket
    }
}

impl Executor for Threaded<'_> {
    fn dispatch(&self, item: Item) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        let priority = item.priority();
        let threshold = match priority {
            IOPriority::Critical => self.thread_count * 2,
            IOPriority::Normal => self.thread_count,
            IOPriority::Background => self.thread_count / 2,
        };

        if priority == IOPriority::Critical && self.pool.queued_count() < threshold {
            debug!("Dispatching critical operation directly");
            self.submit(item);
            return Box::new(self.rx.try_iter().filter_map(|task| {
                if let Task::Request(item) = task {
                    self.reclaim(&item);
                    Some(item)
                } else {
                    None
                }
            }));
        }

        if self.pool.queued_count() >= threshold {
            debug!("Queueing {:?} priority operation (pool has {} items)", 
                   priority, self.pool.queued_count());
            self.queue_item(item);

            self.process_queued_items();

            Box::new(self.rx.try_iter().filter_map(|task| {
                if let Task::Request(item) = task {
                    self.reclaim(&item);
                    Some(item)
                } else {
                    None
                }
            }))
        } else {
            self.submit(item);
            Box::new(self.rx.try_iter().filter_map(|task| {
                if let Task::Request(item) = task {
                    self.reclaim(&item);
                    Some(item)
                } else {
                    None
                }
            }))
        }
    }

    fn join(&mut self) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        self.maybe_adjust_thread_count();

        let mut prev_files = self.n_files.load(Ordering::Relaxed);
        if let Some(handler) = self.notify_handler {
            handler(Notification::DownloadFinished);
            handler(Notification::DownloadPushUnit(Unit::IO));
            handler(Notification::DownloadContentLengthReceived(
                prev_files as u64,
            ));
        }
        if prev_files > 50 {
            debug!("{prev_files} deferred IO operations");
        }
        let buf: Vec<u8> = vec![0; prev_files];
        assert!(32767 > prev_files);
        let mut current_files = prev_files;
        while current_files != 0 {
            use std::thread::sleep;
            sleep(std::time::Duration::from_millis(100));
            prev_files = current_files;
            current_files = self.n_files.load(Ordering::Relaxed);
            let step_count = prev_files - current_files;
            if let Some(handler) = self.notify_handler {
                handler(Notification::DownloadDataReceived(&buf[0..step_count]));
            }
        }
        self.pool.join();
        if let Some(handler) = self.notify_handler {
            handler(Notification::DownloadFinished);
            handler(Notification::DownloadPopUnit);
        }
        self.tx
            .send(Task::Sentinel)
            .expect("must still be listening");
        Box::new(JoinIterator {
            executor: self,
            consume_sentinel: false,
        })
    }

    fn completed(&self) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        while self.process_queued_items() {}

        Box::new(JoinIterator {
            executor: self,
            consume_sentinel: true,
        })
    }

    fn incremental_file_state(&self) -> super::IncrementalFileState {
        super::IncrementalFileState::Threaded
    }

    fn get_buffer(&mut self, capacity: usize) -> super::FileBuffer {
        let bucket = self.find_bucket(capacity);
        let pool = &mut self.vec_pools[bucket];
        let mut item = pool.pool.clone().create_owned().unwrap();
        item.reserve_exact(pool.size);
        pool.claim();
        super::FileBuffer::Threaded(PoolReference::Mut(item, pool.pool.clone()))
    }

    fn buffer_available(&self, len: usize) -> bool {
        let bucket = self.find_bucket(len);
        let pool = &self.vec_pools[bucket];
        if pool.in_use < pool.high_watermark {
            return true;
        }
        let size = pool.size;
        let total_used = Threaded::ram_highwater(&self.vec_pools);
        total_used + size < self.ram_budget
    }

    #[cfg(test)]
    fn buffer_used(&self) -> usize {
        self.vec_pools.iter().map(|(_, p)| *p.in_use.borrow()).sum()
    }
}

impl Drop for Threaded<'_> {
    fn drop(&mut self) {
        self.join().for_each(drop);
    }
}

struct JoinIterator<'a, 'b> {
    executor: &'a Threaded<'b>,
    consume_sentinel: bool,
}

impl JoinIterator<'_, '_> {
    fn inner<T: Iterator<Item = Task>>(&self, mut iter: T) -> Option<CompletedIo> {
        loop {
            let task_o = iter.next();
            match task_o {
                None => break None,
                Some(task) => match task {
                    Task::Sentinel => {
                        if self.consume_sentinel {
                            continue;
                        } else {
                            break None;
                        }
                    }
                    Task::Request(item) => {
                        self.executor.reclaim(&item);
                        break Some(item);
                    }
                },
            }
        }
    }
}

impl Iterator for JoinIterator<'_, '_> {
    type Item = CompletedIo;

    fn next(&mut self) -> Option<CompletedIo> {
        if self.consume_sentinel {
            self.inner(self.executor.rx.try_iter())
        } else {
            self.inner(self.executor.rx.iter())
        }
    }
}

#[allow(dead_code)]
struct SubmitIterator<'a, 'b> {
    executor: &'a Threaded<'b>,
    item: Cell<Option<Item>>,
}

#[allow(dead_code)]
impl Iterator for SubmitIterator<'_, '_> {
    type Item = CompletedIo;

    fn next(&mut self) -> Option<CompletedIo> {
        let threshold = 5;
        if self.executor.pool.queued_count() < threshold {
            if let Some(item) = self.item.take() {
                self.executor.submit(item);
            };
            None
        } else {
            for task in self.executor.rx.iter() {
                if let Task::Request(item) = task {
                    self.executor.reclaim(&item);
                    return Some(item);
                }
                if self.executor.pool.queued_count() < threshold {
                    if let Some(item) = self.item.take() {
                        self.executor.submit(item);
                    };
                    return None;
                }
            }
            unreachable!();
        }
    }
}
