use std::collections::HashMap;

use anyhow::Result;

use super::{get_executor, Executor, Item, Kind};
use crate::process::TestProcess;
use crate::test::test_dir;

impl Item {
    /// The length of the file, for files (for stats)
    fn size(&self) -> Option<usize> {
        match &self.kind {
            Kind::File(buf) => Some(buf.len()),
            _ => None,
        }
    }
}

fn test_incremental_file(io_threads: &str) -> Result<()> {
    let work_dir = test_dir()?;
    let mut vars = HashMap::new();
    vars.insert("RUSTUP_IO_THREADS".to_string(), io_threads.to_string());
    let tp = TestProcess::with_vars(vars);

    let mut written = 0;
    let mut file_finished = false;
    let mut io_executor: Box<dyn Executor> = get_executor(None, 32 * 1024 * 1024, &tp.process)?;
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
    sender(chunk);
    let mut chunk = io_executor.get_buffer(super::IO_CHUNK_SIZE);
    chunk.extend(b"0123456789");
    chunk = chunk.finished();
    sender(chunk);
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
    sender(chunk);
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

fn test_complete_file(io_threads: &str) -> Result<()> {
    let work_dir = test_dir()?;
    let mut vars = HashMap::new();
    vars.insert("RUSTUP_IO_THREADS".to_string(), io_threads.to_string());
    let tp = TestProcess::with_vars(vars);

    let mut io_executor: Box<dyn Executor> = get_executor(None, 32 * 1024 * 1024, &tp.process)?;
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
