fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["protos/perfetto_trace.proto"], &["protos"])?;
    Ok(())
}
