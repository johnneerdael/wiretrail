use crate::raw::RawDoc;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

/// Load a HAR file via mmap and a single-pass typed deserialization.
pub fn load(path: &Path) -> Result<RawDoc, LoadError> {
    let file = File::open(path).map_err(LoadError::Io)?;
    // SAFETY: the file is opened read-only and not mutated while mapped.
    let mmap = unsafe { Mmap::map(&file).map_err(LoadError::Io)? };
    let doc: RawDoc = serde_json::from_slice(&mmap).map_err(LoadError::Json)?;
    Ok(doc)
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("failed to read HAR file")]
    Io(#[source] std::io::Error),
    #[error("failed to parse HAR JSON")]
    Json(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn loads_v1_2_fixture() {
        let doc = load(&fixture("someapi123.har")).expect("should load");
        assert_eq!(doc.log.version, "1.2");
        assert!(!doc.log.entries.is_empty());
    }

    #[test]
    fn loads_v1_3_fixture() {
        let doc = load(&fixture("someapi13.har")).expect("should load");
        assert_eq!(doc.log.version, "1.3");
        assert!(!doc.log.entries.is_empty());
    }
}
