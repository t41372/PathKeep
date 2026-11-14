//! Responsibilities: prove bounded git reads reject special files and report unreadable text.
//! Not responsible for: invoking git or replacing source-filesystem operations with mocks.
//! Failure mode: invalid UTF-8 silently loses git facts.
use super::git;

#[test]
fn invalid_git_text_is_reported_as_unreadable() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join(".git")).unwrap();
    std::fs::write(temp.path().join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();
    std::fs::write(temp.path().join(".git/config"), [0xff, 0xfe]).unwrap();
    let (facts, issue) = git::observe(temp.path());
    assert!(facts.is_none());
    assert_eq!(issue.unwrap().code, "git_unreadable");
}
