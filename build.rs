use git2::Repository;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("git_sha.rs");
    let mut f = File::create(&dest_path).unwrap();

    let git_sha = if let Ok(repo) = Repository::open(".") {
        if let Ok(head) = repo.head() {
            if let Ok(commit) = head.peel_to_commit() {
                commit.id().to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };

    writeln!(&mut f, "pub const GIT_SHA: &str = \"{}\";", git_sha).unwrap();
}
