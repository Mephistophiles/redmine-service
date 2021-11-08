fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/redmine_api.proto")?;
    println!("cargo:rerun-if-changed=proto/redmine_api.proto");
    Ok(())
}
