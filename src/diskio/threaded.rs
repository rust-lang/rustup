/// Threaded IO model: A pool of threads is used so that syscall latencies
/// due to (nonexhaustive list) Network file systems, virus scanners, and
/// operating system design, do not cause rustup to be significantly slower
/// than desired. In particular the docs workload with 20K files requires
/// very low latency per file, which even a few ms per syscall per file
/// will cause minutes of wall clock time.
use std::cell::{Cell, RefCell};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use enum_map::{enum_map, Enum, EnumMap};
use sharded_slab::pool::{OwnedRef, OwnedRefMut};

use super::{perform, CompletedIo, Executor, Item};
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
}

impl<'a> Threaded<'a> {
    /// Construct a new Threaded executor.
    pub(crate) fn new(
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
        thread_count: usize,
        ram_budget: usize,
    ) -> Self {
        // Defaults to hardware thread count threads; this is suitable for
        // our needs as IO bound operations tend to show up as write latencies
        // rather than close latencies, so we don't need to look at
        // more threads to get more IO dispatched at this stage in the process.
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
        // Ensure there is at least one each size buffer, so we can always make forward progress.
        for (_, pool) in &vec_pools {
            let key = pool
                .pool
                .create_with(|vec| vec.reserve_exact(pool.size - vec.len()))
                .unwrap();
            pool.pool.clear(key);
        }
        // Since we've just *used* this memory, we had better have been allowed to!
        assert!(Threaded::ram_highwater(&vec_pools) < ram_budget);
        Self {
            n_files: Arc::new(AtomicUsize::new(0)),
            pool,
            notify_handler,
            rx,
            tx,
            vec_pools,
            ram_budget,
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
        self.pool.execute(move || {
            let chunk_complete_callback = |size| {
                tx.send(Task::Request(CompletedIo::Chunk(size)))
                    .expect("receiver should be listening")
            };
            perform(&mut item, chunk_complete_callback);
            n_files.fetch_sub(1, Ordering::Relaxed);
            tx.send(Task::Request(CompletedIo::Item(item)))
                .expect("receiver should be listening");
        });
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

impl<'a> Executor for Threaded<'a> {
    fn dispatch(&self, item: Item) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        // Yield any completed work before accepting new work - keep memory
        // pressure under control
        // - return an iterator that runs until we can submit and then submits
        //   as its last action
        Box::new(SubmitIterator {
            executor: self,
            item: Cell::new(Some(item)),
        })
    }

    fn join(&mut self) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
        // Some explanation is in order. Even though the tar we are reading from (if
        // any) will have had its FileWithProgress download tracking
        // completed before we hit drop, that is not true if we are unwinding due to a
        // failure, where the logical ownership of the progress bar is
        // ambiguous, and as the tracker itself is abstracted out behind
        // notifications etc we cannot just query for that. So: we assume no
        // more reads of the underlying tar will take place: either the
        // error unwinding will stop reads, or we completed; either way, we
        // notify finished to the tracker to force a reset to zero; we set
        // the units to files, show our progress, and set our units back
        // afterwards. The largest archives today - rust docs - have ~20k
        // items, and the download tracker's progress is confounded with
        // actual handling of data today, we synthesis a data buffer and
        // pretend to have bytes to deliver.
        let mut prev_files = self.n_files.load(Ordering::Relaxed);
        if let Some(handler) = self.notify_handler {
            handler(Notification::DownloadFinished);
            handler(Notification::DownloadPushUnit(Unit::IO));
            handler(Notification::DownloadContentLengthReceived(
                prev_files as u64,
            ));
        }
        if prev_files > 50 {
            eprintln!("{prev_files} deferred IO operations");
        }
        let buf: Vec<u8> = vec![0; prev_files];
        // Cheap wrap-around correctness check - we have 20k files, more than
        // 32K means we subtracted from 0 somewhere.
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
        // close the feedback channel so that blocking reads on it can
        // complete. send is atomic, and we know the threads completed from the
        // pool join, so this is race-free. It is possible that try_iter is safe
        // but the documentation is not clear: it says it will not wait, but not
        // whether a put done by another thread on a NUMA machine before (say)
        // the mutex in the thread pool is entirely synchronised; since this is
        // largely hidden from the clients, digging into check whether we can
        // make this tidier (e.g. remove the Marker variant) is left for another
        // day. I *have* checked that insertion is barried and ordered such that
        // sending the marker cannot come in before markers sent from other
        // threads we just joined.
        self.tx
            .send(Task::Sentinel)
            .expect("must still be listening");
        Box::new(JoinIterator {
            executor: self,
            consume_sentinel: false,
        })
    }

    fn completed(&self) -> Box<dyn Iterator<Item = CompletedIo> + '_> {
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
        // if either: there is room in the budget to assign a new slab entry of
        // this size, or there is an unused slab entry of this size.
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

impl<'a> Drop for Threaded<'a> {
    fn drop(&mut self) {
        // We are not permitted to fail - consume but do not handle the items.
        self.join().for_each(drop);
    }
}

struct JoinIterator<'a, 'b> {
    executor: &'a Threaded<'b>,
    consume_sentinel: bool,
}

impl<'a, 'b> JoinIterator<'a, 'b> {
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

impl<'a, 'b> Iterator for JoinIterator<'a, 'b> {
    type Item = CompletedIo;

    fn next(&mut self) -> Option<CompletedIo> {
        if self.consume_sentinel {
            self.inner(self.executor.rx.try_iter())
        } else {
            self.inner(self.executor.rx.iter())
        }
    }
}

struct SubmitIterator<'a, 'b> {
    executor: &'a Threaded<'b>,
    item: Cell<Option<Item>>,
}

impl<'a, 'b> Iterator for SubmitIterator<'a, 'b> {
    type Item = CompletedIo;

    fn next(&mut self) -> Option<CompletedIo> {
        // The number here is arbitrary; just a number to stop exhausting fd's on linux
        // and still allow rapid decompression to generate work to dispatch
        // This function could perhaps be tuned: e.g. it may wait in rx.iter()
        // unnecessarily blocking if many items complete at once but threads do
        // not pick up work quickly for some reason, until another thread
        // actually completes; however, results are presently ok.
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
