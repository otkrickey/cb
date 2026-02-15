use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from("../../CB/Generated");

    let bridges = vec!["src/lib.rs"];

    swift_bridge_build::parse_bridges(bridges)
        .write_all_concatenated(out_dir, env!("CARGO_PKG_NAME"));
}
