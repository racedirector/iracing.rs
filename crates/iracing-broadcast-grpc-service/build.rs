fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/broadcast.proto");

    let descriptor_path =
        std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("iracing.broadcast.bin");
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);

    tonic_prost_build::configure()
        .file_descriptor_set_path(descriptor_path)
        .compile_with_config(config, &["proto/broadcast.proto"], &["proto"])?;
    Ok(())
}
