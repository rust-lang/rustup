use std::collections::HashMap;

use super::{Executor, Item, Kind, get_executor};
use crate::{process::TestProcess, test::test_dir};

impl Item {
    /// The length of the file, for files (for stats)
    fn size(&self) -> Option<usize> {
        match &self.kind {
            Kind::File(buf) => Some(buf.len()),
            _ => None,
        }
    }
}

fn test_incremental_file(io_threads: &str) -> anyhow::Result<()> {
    let work_dir = test_dir()?;
    let mut vars = HashMap::new();
    vars.insert("RUSTUP_IO_THREADS".to_string(), io_threads.to_string());
    let tp = TestProcess::with_vars(vars);

    let mut written = 0;
    let mut file_finished = false;
    let mut io_executor: Box<dyn Executor> =
        get_executor(32 * 1024 * 1024, tp.process.io_thread_count()?);
    let (item, mut sender) = Item::write_file_segmented(
        work_dir.path().join("scratch"),
        0o666,
        io_executor.incremental_file_state(),
    )?;

    // The file should be open and incomplete, and no completed chunks
    assert!(io_executor.execute(item).collect::<Vec<_>>().is_empty());

    let mut chunk = io_executor.get_buffer(super::IO_CHUNK_SIZE);
    chunk.extend(b"0123456789");
    chunk = chunk.finished();
    sender.submit(chunk);
    let mut chunk = io_executor.get_buffer(super::IO_CHUNK_SIZE);
    chunk.extend(b"0123456789");
    chunk = chunk.finished();
    sender.submit(chunk);
    loop {
        for work in io_executor.completed().collect::<Vec<_>>() {
            match work {
                super::CompletedIo::Chunk(size) => written += size,
                super::CompletedIo::Item(item) => unreachable!("{:?}", item),
            }
        }
        if written == 20 {
            break;
        }
    }
    // sending a zero length chunk closes the file
    let mut chunk = io_executor.get_buffer(super::IO_CHUNK_SIZE);
    chunk = chunk.finished();
    sender.submit(chunk);
    loop {
        for work in io_executor.completed().collect::<Vec<_>>() {
            match work {
                super::CompletedIo::Chunk(_) => {}
                super::CompletedIo::Item(_) => {
                    file_finished = true;
                }
            }
        }
        if file_finished {
            break;
        }
    }

    // no more work should be outstanding
    assert!(file_finished);
    assert!(io_executor.join().collect::<Vec<_>>().is_empty());
    assert_eq!(io_executor.buffer_used(), 0);

    // We should be able to read back the file
    assert_eq!(
        std::fs::read_to_string(work_dir.path().join("scratch"))?,
        "01234567890123456789".to_string()
    );
    Ok(())
}

fn test_complete_file(io_threads: &str) -> anyhow::Result<()> {
    let work_dir = test_dir()?;
    let mut vars = HashMap::new();
    vars.insert("RUSTUP_IO_THREADS".to_string(), io_threads.to_string());
    let tp = TestProcess::with_vars(vars);

    let mut io_executor: Box<dyn Executor> =
        get_executor(32 * 1024 * 1024, tp.process.io_thread_count()?);
    let mut chunk = io_executor.get_buffer(10);
    chunk.extend(b"0123456789");
    assert_eq!(chunk.len(), 10);
    chunk = chunk.finished();
    let item = Item::write_file(work_dir.path().join("scratch"), 0o666, chunk);
    assert_eq!(item.size(), Some(10));
    let mut items = 0;
    let mut check_item = |item: Item| {
        assert_eq!(item.size(), Some(10));
        items += 1;
        assert_eq!(1, items);
    };
    let mut finished = false;
    for work in io_executor.execute(item).collect::<Vec<_>>() {
        // The file might complete immediately
        match work {
            super::CompletedIo::Chunk(size) => unreachable!("{:?}", size),
            super::CompletedIo::Item(item) => {
                check_item(item);
                finished = true;
            }
        }
    }
    if !finished {
        loop {
            for work in io_executor.completed().collect::<Vec<_>>() {
                match work {
                    super::CompletedIo::Chunk(size) => unreachable!("{:?}", size),
                    super::CompletedIo::Item(item) => {
                        check_item(item);
                        finished = true;
                    }
                }
            }
            if finished {
                break;
            }
        }
    }
    assert!(items > 0);
    // no more work should be outstanding
    assert!(io_executor.join().collect::<Vec<_>>().is_empty());

    // We should be able to read back the file with correct content
    assert_eq!(
        std::fs::read_to_string(work_dir.path().join("scratch"))?,
        "0123456789".to_string()
    );
    Ok(())
}

#[test]
fn test_incremental_file_immediate() {
    test_incremental_file("1").unwrap()
}

#[test]
fn test_incremental_file_threaded() {
    test_incremental_file("2").unwrap()
}

#[test]
fn test_complete_file_immediate() {
    test_complete_file("1").unwrap()
}

#[test]
fn test_complete_file_threaded() {
    test_complete_file("2").unwrap()
}

#[test]
fn test_effective_thread_count() {
    use super::{LOW_MEMORY_THRESHOLD, effective_thread_count};
    use crate::process::IoThreadCount::{Default, UserSpecified};

    // Already single-threaded: no change regardless of budget
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 16, Default(1)),
        1
    );
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 16, Default(0)),
        0
    );

    // Below threshold: forced to single-threaded
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 2, Default(8)),
        1
    );
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 2, Default(4)),
        1
    );

    // At or above threshold: thread count unchanged
    assert_eq!(effective_thread_count(LOW_MEMORY_THRESHOLD, Default(4)), 4);
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD * 2, Default(8)),
        8
    );

    // User-specified threads are always respected
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 16, UserSpecified(4)),
        4
    );
    assert_eq!(
        effective_thread_count(LOW_MEMORY_THRESHOLD / 2, UserSpecified(8)),
        8
    );
}

#[cfg(target_os = "linux")]
#[test]
fn test_linux_available_memory() -> anyhow::Result<()> {
    use super::linux_available_memory;

    let dir = test_dir()?;
    let path = dir.path().join("meminfo");
    // An absent file and an unreadable file (a directory) both retain fallback.
    assert_eq!(linux_available_memory(&path), None);
    assert_eq!(linux_available_memory(dir.path()), None);

    for (contents, expected) in [
        (
            "MemTotal: 928456 kB\nMemAvailable: 520192 kB\n",
            Some(508 * 1024 * 1024),
        ),
        ("MemAvailable:\t  1 kB\n", Some(1024)),
        ("MemAvailable: 0 kB\n", Some(0)),
        (
            "MemAvailable: 18014398509481983 kB\n",
            Some(u64::MAX - 1023),
        ),
        ("MemAvailable: 18014398509481984 kB\n", None),
        ("MemAvailable: 18446744073709551616 kB\n", None),
        ("MemAvailable: -1 kB\n", None),
        ("MemAvailable: invalid kB\n", None),
        ("MemAvailable: 512 MB\n", None),
        ("MemAvailable: 512\n", None),
        ("MemAvailable: 512 kB extra\n", None),
        ("MemAvailable:\n", None),
        ("MemTotal: 928456 kB\nMemFree: 100 kB\n", None),
        ("OtherMemAvailable: 512 kB\n", None),
        ("", None),
    ] {
        std::fs::write(&path, contents)?;
        assert_eq!(linux_available_memory(&path), expected, "{contents:?}");
    }
    std::fs::write(&path, [0xff])?;
    assert_eq!(linux_available_memory(&path), None);
    Ok(())
}
