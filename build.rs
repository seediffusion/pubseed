//! Gives the program file what Windows expects of it: the manifest wxWidgets
//! needs (version 6 common controls for themed, dark-mode capable widgets,
//! and per-monitor DPI awareness), and version details, which Explorer shows
//! and the installer reads. Also turns the help, written in Markdown, into the
//! page Help opens, next to the program.

use shipfitter::docs::{Page, convert};

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest::embed_manifest(embed_manifest::new_manifest("Pubseed"))
            .expect("embed manifest");
        let mut details = winresource::WindowsResource::new();
        details
            .set("ProductName", "Pubseed")
            .set("FileDescription", "Pubseed")
            .set("CompanyName", "Seediffusion")
            .set("LegalCopyright", "Copyright (c) 2026 Seediffusion and contributors");
        details.compile().expect("version details");
    }
    if let Some(dir) = shipfitter::build::target_profile_dir() {
        convert("docs/help.md".as_ref(), &dir.join("help.html"), &Page::default())
            .expect("help page");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=docs/help.md");
}
