use std::process::Command;

use chrono::Utc;

fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/");

    let version = env!("CARGO_PKG_VERSION");
    let git_hash = git_short_hash().unwrap_or_else(|| "unknown".to_owned());
    let build_date = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let is_release = is_release_commit(version, &git_hash);

    let version_string = if is_release {
        version.to_owned()
    } else {
        format!("{version}+{git_hash}.{build_date}")
    };

    println!("cargo:rustc-env=CARGO_CHANGESET_VERSION={version_string}");
}

fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
    } else {
        None
    }
}

fn is_release_commit(version: &str, git_hash: &str) -> bool {
    if git_hash == "unknown" {
        return false;
    }

    let expected_tag = format!("cargo-changeset@v{version}");

    let output = Command::new("git")
        .args(["tag", "--points-at", "HEAD"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let tags = String::from_utf8(out.stdout).unwrap_or_default();
            tags.lines().any(|line| line.trim() == expected_tag)
        }
        _ => false,
    }
}
