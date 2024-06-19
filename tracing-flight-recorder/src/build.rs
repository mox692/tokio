use std::{env, io::Result};
fn main() -> Result<()> {
    prost_build::compile_protos(&["src/proto/items.proto"], &["src/"])?;
    Ok(())
}
