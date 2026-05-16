fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/broadcast.proto");
    tonic_prost_build::compile_protos("proto/broadcast.proto")?;
    Ok(())
}
