#![allow(dead_code)]

use std::fs;

use tempfile::TempDir;

pub fn write_changeset(dir: &TempDir, filename: &str, package: &str, bump: &str, summary: &str) {
    let changeset_dir = dir.path().join(".changeset/changesets");
    fs::create_dir_all(&changeset_dir).expect("failed to create .changeset/changesets dir");
    let content = format!(
        r#"---
"{package}": {bump}
---

{summary}
"#
    );
    fs::write(changeset_dir.join(filename), content).expect("failed to write changeset");
}

pub fn write_multi_changeset(
    dir: &TempDir,
    filename: &str,
    entries: &[(&str, &str)],
    summary: &str,
) {
    let changeset_dir = dir.path().join(".changeset/changesets");
    fs::create_dir_all(&changeset_dir).expect("failed to create .changeset/changesets dir");
    let front_matter: String = entries
        .iter()
        .map(|(pkg, bump)| format!("\"{pkg}\": {bump}\n"))
        .collect();
    let content = format!("---\n{front_matter}---\n\n{summary}\n");
    fs::write(changeset_dir.join(filename), content)
        .expect("failed to write multi-package changeset");
}
