//! `rustup doc` and `rustup man`: opening toolchain documentation and man pages.

use std::{
    borrow::Cow,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use clap::Args;
use tracing::info;

use super::topical_doc;
use crate::{
    config::{ActiveSource, Cfg},
    dist::{PartialToolchainDesc, manifest::ComponentStatus},
    toolchain::DistributableToolchain,
    utils::{self, ExitCode},
};

macro_rules! docs_data {
    (
        $(
            $( #[$meta:meta] )*
            ($ident:ident, $help:expr, $path:expr $(,)?)
        ),+ $(,)?
    ) => {
        #[derive(Debug, Args)]
        pub(crate) struct DocPage {
            $(
                #[doc = $help]
                #[arg(long, group = "page")]
                $( #[$meta] )*
                $ident: bool,
            )+
        }

        impl DocPage {
            fn path_str(&self) -> Option<&'static str> {
                $( if self.$ident { return Some($path); } )+
                None
            }
        }
    };
}

docs_data![
    // flags can be used to open specific documents, e.g. `rustup doc --nomicon`
    // tuple elements: document name used as flag, help message, document index path
    (
        alloc,
        "The Rust core allocation and collections library",
        "alloc/index.html"
    ),
    (
        book,
        "The Rust Programming Language book",
        "book/index.html"
    ),
    (cargo, "The Cargo Book", "cargo/index.html"),
    (clippy, "The Clippy Documentation", "clippy/index.html"),
    (core, "The Rust Core Library", "core/index.html"),
    (
        edition_guide,
        "The Rust Edition Guide",
        "edition-guide/index.html"
    ),
    (
        embedded_book,
        "The Embedded Rust Book",
        "embedded-book/index.html"
    ),
    (
        error_codes,
        "The Rust Error Codes Index",
        "error_codes/index.html"
    ),
    (
        nomicon,
        "The Dark Arts of Advanced and Unsafe Rust Programming",
        "nomicon/index.html"
    ),
    #[arg(long = "proc_macro")]
    (
        proc_macro,
        "A support library for macro authors when defining new macros",
        "proc_macro/index.html"
    ),
    (reference, "The Rust Reference", "reference/index.html"),
    (releases, "Rust Release Notes", "releases.html"),
    (
        rust_by_example,
        "A collection of runnable examples that illustrate various Rust concepts and standard libraries",
        "rust-by-example/index.html"
    ),
    (
        rustc,
        "The compiler for the Rust programming language",
        "rustc/index.html"
    ),
    (
        rustc_docs,
        "The API documentation for the Rust compiler and other toolchain components",
        "rustc-docs/index.html"
    ),
    (
        rustdoc,
        "Documentation generator for Rust projects",
        "rustdoc/index.html"
    ),
    (std, "Standard library API documentation", "std/index.html"),
    (
        style_guide,
        "The Rust Style Guide",
        "style-guide/index.html"
    ),
    (
        test,
        "Support code for rustc's built in unit-test and micro-benchmarking framework",
        "test/index.html"
    ),
    (
        unstable_book,
        "The Unstable Book",
        "unstable-book/index.html"
    ),
];

impl DocPage {
    fn path(&self) -> Option<&'static Path> {
        self.path_str().map(Path::new)
    }

    fn name(&self) -> Option<&'static str> {
        Some(self.path_str()?.rsplit_once('/')?.0)
    }

    fn resolve<'t>(&self, root: &Path, topic: &'t str) -> Option<(PathBuf, Option<&'t str>)> {
        // Use `.parent()` to chop off the default top-level `index.html`.
        let mut base = root.join(Path::new(self.path()?).parent()?);
        base.extend(topic.split("::"));
        let base_index_html = base.join("index.html");

        if base_index_html.is_file() {
            return Some((base_index_html, None));
        }

        let base_html = base.with_extension("html");
        if base_html.is_file() {
            return Some((base_html, None));
        }

        let parent_html = base.parent()?.with_extension("html");
        if parent_html.is_file() {
            return Some((parent_html, topic.rsplit_once("::").map(|(_, s)| s)));
        }

        None
    }
}

pub(crate) async fn doc(
    cfg: &Cfg<'_>,
    path_only: bool,
    toolchain: Option<PartialToolchainDesc>,
    mut topic: Option<&str>,
    doc_page: &DocPage,
) -> Result<ExitCode> {
    let toolchain = toolchain.map(|desc| (desc, ActiveSource::CommandLine));
    let toolchain = cfg.toolchain_from_partial(toolchain).await?.0;

    if let Ok(distributable) = DistributableToolchain::try_from(&toolchain)
        && let [_] = distributable
            .components()?
            .into_iter()
            .filter(|cstatus| cstatus.component.short_name() == "rust-docs" && !cstatus.installed)
            .take(1)
            .collect::<Vec<ComponentStatus>>()
            .as_slice()
    {
        info!(
            "`rust-docs` not installed in toolchain `{}`\nhelp: run `rustup component add --toolchain {} rust-docs` to install it",
            distributable.desc(),
            distributable.desc()
        );
        return Err(anyhow!(
            "unable to view documentation which is not installed"
        ));
    };

    let (doc_path, fragment) = match (topic, doc_page.name()) {
        (Some(topic), Some(name)) => {
            let (doc_path, fragment) = doc_page
                .resolve(&toolchain.doc_path("")?, topic)
                .context(format!("no document for {name} on {topic}"))?;
            (Cow::Owned(doc_path), fragment)
        }
        (Some(topic), None) => {
            let doc_path = topical_doc::local_path(&toolchain.doc_path("").unwrap(), topic)?;
            (Cow::Owned(doc_path), None)
        }
        (None, name) => {
            topic = name;
            let doc_path = doc_page.path().unwrap_or_else(|| Path::new("index.html"));
            (Cow::Borrowed(doc_path), None)
        }
    };

    if path_only {
        let doc_path = toolchain.doc_path(&doc_path)?;
        writeln!(cfg.process.stdout().lock(), "{}", doc_path.display())?;
        return Ok(ExitCode::SUCCESS);
    }

    if let Some(name) = topic {
        writeln!(
            cfg.process.stderr().lock(),
            "Opening docs named `{name}` in your browser"
        )?;
    } else {
        writeln!(cfg.process.stderr().lock(), "Opening docs in your browser")?;
    }
    toolchain.open_docs(&doc_path, fragment)?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(windows))]
pub(crate) async fn man(
    cfg: &Cfg<'_>,
    command: &str,
    toolchain: Option<PartialToolchainDesc>,
) -> Result<ExitCode> {
    let toolchain = toolchain.map(|desc| (desc, ActiveSource::CommandLine));
    let toolchain = cfg.toolchain_from_partial(toolchain).await?.0;
    let path = toolchain.man_path();
    utils::assert_is_directory(&path)?;

    let mut manpaths = std::ffi::OsString::from(path);
    manpaths.push(":"); // prepend to the default MANPATH list
    if let Some(path) = cfg.process.var_os("MANPATH") {
        manpaths.push(path);
    }
    std::process::Command::new("man")
        .env("MANPATH", manpaths)
        .arg(command)
        .status()
        .expect("failed to open man page");
    Ok(ExitCode::SUCCESS)
}
