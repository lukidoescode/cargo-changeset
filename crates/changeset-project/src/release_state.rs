use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Active prerelease configuration.
/// File: `.changeset/pre-release.toml`
/// Format:
/// ```toml
/// crate-a = "alpha"
/// crate-b = "beta"
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrereleaseState {
    #[serde(flatten)]
    packages: HashMap<String, String>,
}

impl PrereleaseState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, crate_name: &str) -> Option<&str> {
        self.packages.get(crate_name).map(String::as_str)
    }

    pub fn insert(&mut self, crate_name: String, tag: String) {
        self.packages.insert(crate_name, tag);
    }

    #[must_use]
    pub fn remove(&mut self, crate_name: &str) -> Option<String> {
        self.packages.remove(crate_name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.packages.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    #[must_use]
    pub fn contains(&self, crate_name: &str) -> bool {
        self.packages.contains_key(crate_name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.packages.len()
    }
}

/// Graduation queue for 0.x packages.
/// File: `.changeset/graduation.toml`
/// Format:
/// ```toml
/// graduation = ["crate-a", "crate-b"]
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraduationState {
    #[serde(default)]
    graduation: Vec<String>,
}

impl GraduationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, crate_name: String) {
        if !self.graduation.contains(&crate_name) {
            self.graduation.push(crate_name);
        }
    }

    #[must_use]
    pub fn remove(&mut self, crate_name: &str) -> bool {
        let len_before = self.graduation.len();
        self.graduation.retain(|x| x != crate_name);
        self.graduation.len() != len_before
    }

    #[must_use]
    pub fn contains(&self, crate_name: &str) -> bool {
        self.graduation.iter().any(|x| x == crate_name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.graduation.iter().map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.graduation.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.graduation.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod prerelease_state {
        use super::*;

        #[test]
        fn new_creates_empty_state() {
            let state = PrereleaseState::new();

            assert!(state.is_empty());
            assert_eq!(state.len(), 0);
        }

        #[test]
        fn insert_and_get() {
            let mut state = PrereleaseState::new();

            state.insert("my-crate".to_string(), "alpha".to_string());

            assert_eq!(state.get("my-crate"), Some("alpha"));
            assert_eq!(state.len(), 1);
            assert!(!state.is_empty());
        }

        #[test]
        fn insert_overwrites_existing() {
            let mut state = PrereleaseState::new();
            state.insert("my-crate".to_string(), "alpha".to_string());

            state.insert("my-crate".to_string(), "beta".to_string());

            assert_eq!(state.get("my-crate"), Some("beta"));
            assert_eq!(state.len(), 1);
        }

        #[test]
        fn get_nonexistent_returns_none() {
            let state = PrereleaseState::new();

            assert_eq!(state.get("nonexistent"), None);
        }

        #[test]
        fn remove_existing() {
            let mut state = PrereleaseState::new();
            state.insert("my-crate".to_string(), "alpha".to_string());

            let removed = state.remove("my-crate");

            assert_eq!(removed, Some("alpha".to_string()));
            assert!(state.is_empty());
        }

        #[test]
        fn remove_nonexistent_returns_none() {
            let mut state = PrereleaseState::new();

            let removed = state.remove("nonexistent");

            assert_eq!(removed, None);
        }

        #[test]
        fn contains() {
            let mut state = PrereleaseState::new();
            state.insert("my-crate".to_string(), "alpha".to_string());

            assert!(state.contains("my-crate"));
            assert!(!state.contains("other-crate"));
        }

        #[test]
        fn iter() {
            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());
            state.insert("crate-b".to_string(), "beta".to_string());

            let items: Vec<_> = state.iter().collect();

            assert_eq!(items.len(), 2);
            assert!(items.contains(&("crate-a", "alpha")));
            assert!(items.contains(&("crate-b", "beta")));
        }

        #[test]
        fn serialize_deserialize_roundtrip() {
            let mut state = PrereleaseState::new();
            state.insert("crate-a".to_string(), "alpha".to_string());
            state.insert("crate-b".to_string(), "beta".to_string());

            let serialized = toml::to_string(&state).expect("serialization should succeed");
            let deserialized: PrereleaseState =
                toml::from_str(&serialized).expect("deserialization should succeed");

            assert_eq!(state, deserialized);
        }

        #[test]
        fn deserialize_from_toml() {
            let toml_content = r#"
crate-a = "alpha"
crate-b = "beta"
"#;

            let state: PrereleaseState =
                toml::from_str(toml_content).expect("deserialization should succeed");

            assert_eq!(state.get("crate-a"), Some("alpha"));
            assert_eq!(state.get("crate-b"), Some("beta"));
            assert_eq!(state.len(), 2);
        }

        #[test]
        fn deserialize_empty() {
            let toml_content = "";

            let state: PrereleaseState =
                toml::from_str(toml_content).expect("deserialization should succeed");

            assert!(state.is_empty());
        }
    }

    mod graduation_state {
        use super::*;

        #[test]
        fn new_creates_empty_state() {
            let state = GraduationState::new();

            assert!(state.is_empty());
            assert_eq!(state.len(), 0);
        }

        #[test]
        fn add_single() {
            let mut state = GraduationState::new();

            state.add("my-crate".to_string());

            assert!(state.contains("my-crate"));
            assert_eq!(state.len(), 1);
        }

        #[test]
        fn add_duplicate_is_ignored() {
            let mut state = GraduationState::new();
            state.add("my-crate".to_string());

            state.add("my-crate".to_string());

            assert_eq!(state.len(), 1);
        }

        #[test]
        fn add_multiple() {
            let mut state = GraduationState::new();

            state.add("crate-a".to_string());
            state.add("crate-b".to_string());

            assert_eq!(state.len(), 2);
            assert!(state.contains("crate-a"));
            assert!(state.contains("crate-b"));
        }

        #[test]
        fn remove_existing() {
            let mut state = GraduationState::new();
            state.add("my-crate".to_string());

            let removed = state.remove("my-crate");

            assert!(removed);
            assert!(state.is_empty());
        }

        #[test]
        fn remove_nonexistent_returns_false() {
            let mut state = GraduationState::new();

            let removed = state.remove("nonexistent");

            assert!(!removed);
        }

        #[test]
        fn contains() {
            let mut state = GraduationState::new();
            state.add("my-crate".to_string());

            assert!(state.contains("my-crate"));
            assert!(!state.contains("other-crate"));
        }

        #[test]
        fn iter() {
            let mut state = GraduationState::new();
            state.add("crate-a".to_string());
            state.add("crate-b".to_string());

            let items: Vec<_> = state.iter().collect();

            assert_eq!(items.len(), 2);
            assert!(items.contains(&"crate-a"));
            assert!(items.contains(&"crate-b"));
        }

        #[test]
        fn serialize_deserialize_roundtrip() {
            let mut state = GraduationState::new();
            state.add("crate-a".to_string());
            state.add("crate-b".to_string());

            let serialized = toml::to_string(&state).expect("serialization should succeed");
            let deserialized: GraduationState =
                toml::from_str(&serialized).expect("deserialization should succeed");

            assert_eq!(state, deserialized);
        }

        #[test]
        fn deserialize_from_toml() {
            let toml_content = r#"
graduation = ["crate-a", "crate-b"]
"#;

            let state: GraduationState =
                toml::from_str(toml_content).expect("deserialization should succeed");

            assert!(state.contains("crate-a"));
            assert!(state.contains("crate-b"));
            assert_eq!(state.len(), 2);
        }

        #[test]
        fn deserialize_empty() {
            let toml_content = "";

            let state: GraduationState =
                toml::from_str(toml_content).expect("deserialization should succeed");

            assert!(state.is_empty());
        }

        #[test]
        fn deserialize_empty_array() {
            let toml_content = "graduation = []";

            let state: GraduationState =
                toml::from_str(toml_content).expect("deserialization should succeed");

            assert!(state.is_empty());
        }
    }
}
