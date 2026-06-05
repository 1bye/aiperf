use std::path::Path;

pub fn source_maps_available(project_root: &Path) -> bool {
    ["dist", ".next", "build"]
        .iter()
        .any(|dir| project_root.join(dir).exists())
}
