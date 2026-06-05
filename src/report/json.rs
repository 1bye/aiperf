use serde::Serialize;
use std::path::Path;

pub fn write_json<T: Serialize>(value: &T, path: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)
        .map_err(|e| anyhow::anyhow!("create {}: {e}", path.display()))?;
    serde_json::to_writer_pretty(file, value)
        .map_err(|e| anyhow::anyhow!("write JSON {}: {e}", path.display()))
}
