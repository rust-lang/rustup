use std::collections::HashSet;
use std::path::PathBuf;

// Paths are written as a string in the UNIX format to make it easy
// to maintain.
static TEST_CASES: &[(&[&str], &str)] = &[
    (&["core"], "core/index.html"),
    (&["core::arch"], "core/arch/index.html"),
    (&["fn"], "std/keyword.fn.html"),
    (&["keyword:fn"], "std/keyword.fn.html"),
    (&["primitive:fn"], "std/primitive.fn.html"),
    (&["macro:file!"], "std/macro.file!.html"),
    (&["macro:file"], "std/macro.file.html"),
    (&["std::fs"], "std/fs/index.html"),
    (&["std::fs::read_dir"], "std/fs/fn.read_dir.html"),
    (&["std::io::Bytes"], "std/io/struct.Bytes.html"),
    (&["std::iter::Sum"], "std/iter/trait.Sum.html"),
    (&["std::io::error::Result"], "std/io/error/type.Result.html"),
    (&["usize"], "std/primitive.usize.html"),
    (&["eprintln"], "std/macro.eprintln.html"),
    (&["alloc::format"], "alloc/macro.format.html"),
    (&["std::mem::MaybeUninit"], "std/mem/union.MaybeUninit.html"),
    (&["--rustc", "lints"], "rustc/lints/index.html"),
    (&["--rustdoc", "lints"], "rustdoc/lints.html"),
    (
        &["lints::broken_intra_doc_links", "--rustdoc"],
        "rustdoc/lints.html",
    ),
];

fn repath(origin: &str) -> String {
    // Add doc prefix and rewrite string paths for the current platform
    let with_prefix = "share/doc/rust/html/".to_owned() + origin;
    let splitted = with_prefix.split('/');
    let repathed = splitted.fold(PathBuf::new(), |acc, e| acc.join(e));
    repathed.into_os_string().into_string().unwrap()
}

pub fn test_cases<'a>() -> impl Iterator<Item = (&'a [&'a str], String)> {
    TEST_CASES.iter().map(|(args, path)| (*args, repath(path)))
}

pub fn unique_paths() -> impl Iterator<Item = String> {
    // Hashset used to test uniqueness of values through insert method.
    let mut unique_paths = HashSet::new();
    TEST_CASES
        .iter()
        .filter(move |(_, p)| unique_paths.insert(p))
        .map(|(_, p)| repath(p))
}
