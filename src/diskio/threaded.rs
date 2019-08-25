/// Threaded IO model: A pool of threads is used so that syscall latencies
/// due to (nonexhaustive list) Network file systems, virus scanners, and
/// operating system design, do not cause rustup to be significantly slower
/// than desired. In particular the docs workload with 20K files requires
/// very low latency per file, which even a few ms per syscall per file
/// will cause minutes of wall clock time.
use super::{perform, Executor, Item};
use crate::utils::notifications::Notification;

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

enum Task {
    Request(Item),
    // Used to synchronise in the join method.
    Sentinel,
}

impl Default for Task {
    fn default() -> Self {
        Self::Sentinel
    }
}

pub struct Threaded<'a> {
    n_files: Arc<AtomicUsize>,
    pool: threadpool::ThreadPool,
    notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    rx: Receiver<Task>,
    tx: Sender<Task>,
}

impl<'a> Threaded<'a> {
    pub fn new(notify_handler: Option<&'a dyn Fn(Notification<'_>)>) -> Self {
        // Defaults to hardware thread count threads; this is suitable for
        // our needs as IO bound operations tend to show up as write latencies
        // rather than close latencies, so we don't need to look at
        // more threads to get more IO dispatched at this stage in the process.
        let pool = threadpool::Builder::new()
            .thread_name("CloseHandle".into())
            .thread_stack_size(1_048_576)
            .build();
        let (tx, rx) = channel();
        Self {
            n_files: Arc::new(AtomicUsize::new(0)),
            pool,
            notify_handler,
            rx,
            tx,
        }
    }

    pub fn new_with_threads(
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
        thread_count: usize,
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
        Self {
            n_files: Arc::new(AtomicUsize::new(0)),
            pool,
            notify_handler,
            rx,
            tx,
        }
    }

    fn submit(&mut self, mut item: Item) {
        let tx = self.tx.clone();
        self.n_files.fetch_add(1, Ordering::Relaxed);
        let n_files = self.n_files.clone();
        self.pool.execute(move || {
            perform(&mut item);
            n_files.fetch_sub(1, Ordering::Relaxed);
            tx.send(Task::Request(item))
                .expect("receiver should be listening");
        });
    }
}

impl<'a> Executor for Threaded<'a> {
    fn dispatch(&mut self, item: Item) -> Box<dyn Iterator<Item = Item> + '_> {
        // Yield any completed work before accepting new work - keep memory
        // pressure under control
        // - return an iterator that runs until we can submit and then submits
        //   as its last action
        Box::new(SubmitIterator {
            executor: self,
            item: Cell::new(Task::Request(item)),
        })
    }

    fn join(&mut self) -> Box<dyn Iterator<Item = Item> + '_> {
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
            handler(Notification::DownloadPushUnits("iops"));
            handler(Notification::DownloadContentLengthReceived(
                prev_files as u64,
            ));
        }
        if prev_files > 50 {
            println!("{} deferred IO operations", prev_files);
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
            handler(Notification::DownloadPopUnits);
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
            iter: self.rx.iter(),
            consume_sentinel: false,
        })
    }

    fn completed(&mut self) -> Box<dyn Iterator<Item = Item> + '_> {
        Box::new(JoinIterator {
            iter: self.rx.try_iter(),
            consume_sentinel: true,
        })
    }
}

impl<'a> Drop for Threaded<'a> {
    fn drop(&mut self) {
        // We are not permitted to fail - consume but do not handle the items.
        self.join().for_each(drop);
    }
}

struct JoinIterator<T: Iterator<Item = Task>> {
    iter: T,
    consume_sentinel: bool,
}

impl<T: Iterator<Item = Task>> Iterator for JoinIterator<T> {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        let task_o = self.iter.next();
        match task_o {
            None => None,
            Some(task) => match task {
                Task::Sentinel => {
                    if self.consume_sentinel {
                        self.next()
                    } else {
                        None
                    }
                }
                Task::Request(item) => Some(item),
            },
        }
    }
}

struct SubmitIterator<'a, 'b> {
    executor: &'a mut Threaded<'b>,
    item: Cell<Task>,
}

impl<'a, 'b> Iterator for SubmitIterator<'a, 'b> {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        // The number here is arbitrary; just a number to stop exhausting fd's on linux
        // and still allow rapid decompression to generate work to dispatch
        // This function could perhaps be tuned: e.g. it may wait in rx.iter()
        // unnecessarily blocking if many items complete at once but threads do
        // not pick up work quickly for some reason, until another thread
        // actually completes; however, results are presently ok.
        let threshold = 5;
        if self.executor.pool.queued_count() < threshold {
            if let Task::Request(item) = self.item.take() {
                self.executor.submit(item);
            };
            None
        } else {
            for task in self.executor.rx.iter() {
                if let Task::Request(item) = task {
                    return Some(item);
                }
                if self.executor.pool.queued_count() < threshold {
                    if let Task::Request(item) = self.item.take() {
                        self.executor.submit(item);
                    };
                    return None;
                }
            }
            unreachable!();
        }
    }
}
