use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use uuid::Uuid;

/// Stable mod identity: uuid5 of the mod folder path, same across scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct ModId(pub Uuid);

impl ModId {
    pub fn from_path(path: &Path) -> Self {
        Self(Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            path.to_string_lossy().as_bytes(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_and_distinct() {
        let a = ModId::from_path(Path::new("C:/mods/a"));
        assert_eq!(a, ModId::from_path(Path::new("C:/mods/a")));
        assert_ne!(a, ModId::from_path(Path::new("C:/mods/b")));
    }
}
