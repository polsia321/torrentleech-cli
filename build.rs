use std::process::Command;

fn main() -> anyhow::Result<()> {
    println!("cargo::rerun-if-env-changed=GITHUB_SHA");
    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/index");

    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .or_else(git_commit)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo::rustc-env=TL_COMMIT_SHA={commit}");
    Ok(())
}

fn git_commit() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let commit = String::from_utf8(output.stdout).ok()?;
    let commit = commit.trim();
    if commit.is_empty() {
        None
    } else {
        Some(commit.to_string())
    }
}
