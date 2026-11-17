//! Gives the program file what Windows expects of it: the manifest wxWidgets
//! needs (version 6 common controls for themed, dark-mode capable widgets,
//! and per-monitor DPI awareness), and version details, which Explorer shows
//! and the installer reads.

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
    println!("cargo:rerun-if-changed=build.rs");
}
