use std::collections::HashMap;

use crate::errors::Result;
use crate::test::test_dir;

use super::{get_executor, Executor, Item};
use crate::currentprocess;

fn test_incremental_file(io_threads: &str) -> Result<()> {
    let work_dir = test_dir()?;
    let mut vars = HashMap::new();
    vars.insert("RUSTUP_IO_THREADS".to_string(), io_threads.to_string());
    let tp = Box::new(currentprocess::TestProcess {
        vars,
        ..Default::default()
    });
    currentprocess::with(tp, || -> Result<()> {
        let mut written = 0;
        let mut file_finished = false;
        let mut io_executor: Box<dyn Executor> = get_executor(None)?;
        let (item, mut sender) = Item::write_file_segmented(
            work_dir.path().join("scratch"),
            0o666,
            io_executor.incremental_file_state(),
        )?;
        for _ in io_executor.execute(item).collect::<Vec<_>>() {
            // The file should be open and incomplete, and no completed chunks
            unreachable!();
        }
        let mut chunk: Vec<u8> = vec![];
        chunk.extend(b"0123456789");
        // We should be able to submit more than one chunk
        sender(chunk.clone());
        sender(chunk);
        loop {
            for work in io_executor.completed().collect::<Vec<_>>() {
                match work {
                    super::CompletedIo::Chunk(size) => written += size,
                    super::CompletedIo::Item(item) => unreachable!(format!("{:?}", item)),
                }
            }
            if written == 20 {
                break;
            }
        }
        // sending a zero length chunk closes the file
        sender(vec![]);
        loop {
            for work in io_executor.completed().collect::<Vec<_>>() {
                match work {
                    super::CompletedIo::Chunk(_) => unreachable!(),
                    super::CompletedIo::Item(_) => {
                        file_finished = true;
                    }
                }
            }
            if file_finished {
                break;
            }
        }
        assert_eq!(true, file_finished);
        for _ in io_executor.join().collect::<Vec<_>>() {
            // no more work should be outstanding
            unreachable!();
        }
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_incremental_file_immediate() -> Result<()> {
    test_incremental_file("1")
}

#[test]
fn test_incremental_file_threaded() -> Result<()> {
    test_incremental_file("2")
}
