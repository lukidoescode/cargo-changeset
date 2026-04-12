use std::process::Command;

use tempfile::TempDir;

#[allow(dead_code)]
pub fn init_git_repo(dir: &TempDir) {
    Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir.path())
        .output()
        .expect("failed to init git repo");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(dir.path())
        .output()
        .expect("failed to configure git email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .output()
        .expect("failed to configure git name");
}

#[allow(dead_code)]
pub fn git_add_and_commit(dir: &TempDir, message: &str) {
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir.path())
        .output()
        .expect("failed to git add");
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir.path())
        .output()
        .expect("failed to git commit");
}

#[allow(dead_code)]
pub fn create_branch(dir: &TempDir, name: &str) {
    Command::new("git")
        .args(["checkout", "-b", name])
        .current_dir(dir.path())
        .output()
        .expect("failed to create branch");
}

#[allow(dead_code)]
pub fn create_tag(dir: &TempDir, tag_name: &str, message: &str) {
    Command::new("git")
        .args(["tag", "-a", tag_name, "-m", message])
        .current_dir(dir.path())
        .output()
        .expect("failed to create tag");
}
