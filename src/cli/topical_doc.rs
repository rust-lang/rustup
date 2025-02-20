use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

struct DocData<'a> {
    topic: &'a str,
    subtopic: &'a str,
    root: &'a Path,
}

fn index_html(doc: &DocData<'_>, wpath: &Path) -> Option<PathBuf> {
    let indexhtml = wpath.join("index.html");
    match &doc.root.join(&indexhtml).exists() {
        true => Some(indexhtml),
        false => None,
    }
}

fn dir_into_vec(dir: &Path) -> Result<Vec<OsString>> {
    fs::read_dir(dir)
        .with_context(|| format!("Failed to read_dir {dir:?}"))?
        .map(|f| Ok(f?.file_name()))
        .collect()
}

fn search_path(doc: &DocData<'_>, wpath: &Path, keywords: &[&str]) -> Result<PathBuf> {
    let dir = &doc.root.join(wpath);
    if dir.is_dir() {
        let entries = dir_into_vec(dir)?;
        for k in keywords {
            let filename = &format!("{}.{}.html", k, doc.subtopic);
            if entries.contains(&OsString::from(filename)) {
                return Ok(dir.join(filename));
            }
        }
    }
    Err(anyhow!(format!("No document for '{}'", doc.topic)))
}

pub(crate) fn local_path(root: &Path, topic: &str) -> Result<PathBuf> {
    // The ORDER of keywords_top is used for the default search and should not
    // be changed.
    // https://github.com/rust-lang/rustup/issues/2076#issuecomment-546613036
    let keywords_top = vec!["keyword", "primitive", "macro"];
    let keywords_mod = ["fn", "struct", "trait", "enum", "type", "constant", "union"];

    let topic_vec: Vec<&str> = topic.split("::").collect();
    let work_path = topic_vec.iter().fold(PathBuf::new(), |acc, e| acc.join(e));
    let mut subtopic = topic_vec[topic_vec.len() - 1];
    let mut forced_keyword = None;

    if topic_vec.len() == 1 {
        let split: Vec<&str> = topic.splitn(2, ':').collect();
        if split.len() == 2 {
            forced_keyword = Some(vec![split[0]]);
            subtopic = split[1];
        }
    }

    let doc = DocData {
        topic,
        subtopic,
        root,
    };

    /**************************
     * Please ensure tests/mock/topical_doc_data.rs is UPDATED to reflect
     * any change in functionality.

    Argument      File                    directory

    # len() == 1 Return index.html
    std           std/index.html          root/std
    core          core/index.html         root/core
    alloc         alloc/index.html        root/core
    KKK           std/keyword.KKK.html    root/std
    PPP           std/primitive.PPP.html  root/std
    MMM           std/macro.MMM.html      root/std


    # len() == 2 not ending in ::
    MMM           std/macro.MMM.html      root/std
    KKK           std/keyword.KKK.html    root/std
    PPP           std/primitive.PPP.html  root/std
    MMM           core/macro.MMM.html     root/core
    MMM           alloc/macro.MMM.html    root/alloc
    # If above fail, try module
    std::module   std/module/index.html   root/std/module
    core::module  core/module/index.html  root/core/module
    alloc::module alloc/module/index.html alloc/core/module

    # len() == 2, ending with ::
    std::module   std/module/index.html   root/std/module
    core::module  core/module/index.html  root/core/module
    alloc::module alloc/module/index.html alloc/core/module

    # len() > 2
    # search for index.html in rel_path
    std::AAA::MMM std/AAA/MMM/index.html     root/std/AAA/MMM

    # OR check if parent() dir exists and search for fn/struct/etc
    std::AAA::FFF std/AAA/fn.FFF9.html       root/std/AAA
    std::AAA::SSS std/AAA/struct.SSS.html    root/std/AAA
    core:AAA::SSS std/AAA/struct.SSS.html    root/coreAAA
    alloc:AAA::SSS std/AAA/struct.SSS.html   root/coreAAA
    std::AAA::TTT std/2222/trait.TTT.html    root/std/AAA
    std::AAA::EEE std/2222/enum.EEE.html     root/std/AAA
    std::AAA::TTT std/2222/type.TTT.html     root/std/AAA
    std::AAA::CCC std/2222/constant.CCC.html root/std/AAA

    **************************/

    // topic.split.count cannot be 0
    let subpath_os_path = match topic_vec.len() {
        1 => match topic {
            "std" | "core" | "alloc" => {
                index_html(&doc, &work_path).context(anyhow!("No document for '{}'", doc.topic))?
            }
            _ => search_path(
                &doc,
                Path::new("std"),
                &forced_keyword.unwrap_or(keywords_top),
            )?,
        },
        2 => match index_html(&doc, &work_path) {
            Some(f) => f,
            None => {
                let parent = work_path.parent().unwrap();
                search_path(&doc, parent, &keywords_top)?
            }
        },
        _ => match index_html(&doc, &work_path) {
            Some(f) => f,
            None => {
                // len > 2, guaranteed to have a parent, safe to unwrap
                let parent = work_path.parent().unwrap();
                search_path(&doc, parent, &keywords_mod)?
            }
        },
    };
    // The path and filename were validated to be existing on the filesystem.
    // It should be safe to unwrap, or worth panicking.
    Ok(subpath_os_path)
}
