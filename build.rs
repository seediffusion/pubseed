//! Embeds the Windows manifest wxWidgets needs: version 6 common controls
//! (themed, dark-mode capable widgets) and per-monitor DPI awareness.

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest::embed_manifest(embed_manifest::new_manifest("Pubseed"))
            .expect("embed manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
