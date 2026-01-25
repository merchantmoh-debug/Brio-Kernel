fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("PROTOC").is_err() {
        unsafe {
            std::env::set_var(
                "PROTOC",
                protoc_bin_vendored::protoc_bin_path()
                    .expect("protoc binary not found in vendored crate"),
            );
        }
    }
    tonic_prost_build::compile_protos("proto/mesh.proto")?;
    Ok(())
}
