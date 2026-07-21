use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let proto_root = manifest_dir.join("../../proto");
    let proto_file = proto_root.join("enterprise/telemetry/v1/events.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    // google/protobuf well-known types are bundled with prost-types; we only
    // need our own protos on the include path.
    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(rename_all = \"snake_case\")]")
        .btree_map(["."])
        .compile_protos(&[proto_file], &[proto_root])?;

    Ok(())
}
