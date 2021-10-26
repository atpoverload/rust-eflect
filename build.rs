extern crate protoc_rust;

use std::fs;
use std::path::Path;

// builds the sample protos and a mod.rs to make them importable
fn build_protos() -> std::io::Result<()> {
    let proto_path = "src/protos";
    let protos = Path::new(proto_path);
    if protos.exists() {
        fs::remove_dir_all(protos)?;
    }
    fs::create_dir(protos)?;
    protoc_rust::Codegen::new()
        .out_dir(protos)
        .inputs(&[
            "protos/sample/jiffies.proto",
            "protos/sample/rapl.proto",
            "protos/sample/sample.proto",
        ])
        .include(".")
        .run()
        .expect("protoc");
    let mod_path = proto_path.to_owned() + "/mod.rs";
    fs::write(mod_path, "pub mod sample;\npub mod jiffies;\npub mod rapl;\n")?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    build_protos()
}
