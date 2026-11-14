//! Responsibilities: pin the folder and git observations a run records (contract §4):
//! `folders`, git facts read from `.git` files only, userinfo stripped, symlinks and
//! oversized files not read, and `--no-git`.
//! Not responsible for: seen/missing sources or which record field names a folder
//! (run_observations.rs).
//! Failure modes: an unchanged folder is stored again or changes are not newest first;
//! a worktree reports its own git dir; a remote URL keeps its token; a symlinked `.git`,
//! `HEAD` or `config` is followed; a git file over 1 MiB is read; `--no-git`
//! reads git files; `git` is executed.
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::fixtures::pi;
#[cfg(unix)]
use support::harness::fake_git;
use support::harness::{canonical_root as base, folders, same_place};
use support::native::*;
// `args` and `write` serve only the unix-only tests.
#[cfg_attr(not(unix), allow(unused_imports))]
use support::{Case, archive_contains, args, write};

fn source_arg(harness: &str, root: &Path) -> Vec<String> {
    vec!["--source".into(), format!("{harness}={}", root.display())]
}

fn backup(case: &mut Case, sources: &[String]) -> Value {
    let mut arguments = sources.to_vec();
    arguments.push("--execute".into());
    case.call("backup", arguments)
}

#[cfg(unix)]
#[test]
fn folders_record_the_working_directory_and_its_git_facts_once_per_change() {
    let mut case = Case::new("contract-folder-observations");
    let repository = base(&case).join("projects/observatory");
    let cwd = repository.join("app");
    fs::create_dir_all(&cwd).unwrap();
    let git = git_repository(
        &repository,
        "main",
        &[("origin", "https://example.invalid/synthetic/observatory.git")],
    );
    let plain = base(&case).join("projects/plain-folder");
    fs::create_dir_all(&plain).unwrap();
    let root = base(&case).join("pi");
    pi(&root, cwd.to_str().unwrap(), "pi-in-repository", "repositoryneedle");
    pi(&root, plain.to_str().unwrap(), "pi-in-plain-folder", "plainfolderneedle");
    let (path, sentinel) = fake_git(&case);
    case.fixture(json!({"seed":"contract-folder-observations-v1","private_data":false}));
    case.init();
    let run = |case: &mut Case| {
        let mut arguments = source_arg("pi", &root);
        arguments.push("--execute".into());
        case.call_with_env("backup", arguments, &[("PATH", path.as_str())])
    };
    let report = run(&mut case);
    case.equal("backup captures both files", report["captured"].clone(), json!(2));

    let first = folders(&mut case, Some(&cwd));
    case.equal("one observation of the folder", json!(first.len()), json!(1));
    let entry = &first[0];
    case.equal(
        "path is the structured working directory",
        entry["path"].clone(),
        json!(cwd.to_str().unwrap()),
    );
    case.equal("the folder exists", entry["exists"].clone(), json!(true));
    case.truth("observed_at is recorded", !entry["observed_at"].is_null());
    case.truth("top_level is the repository", same_place(&entry["git"]["top_level"], &repository));
    case.truth("common_dir is its .git", same_place(&entry["git"]["common_dir"], &git));
    case.equal("branch from HEAD", entry["git"]["branch"].clone(), json!("main"));
    case.equal(
        "remotes from config",
        entry["git"]["remotes"].clone(),
        json!(["https://example.invalid/synthetic/observatory.git"]),
    );
    let other = folders(&mut case, Some(&plain));
    case.equal("--path filters to one folder", json!(other.len()), json!(1));
    case.equal("a folder outside git has git: null", other[0]["git"].clone(), Value::Null);

    run(&mut case);
    let unchanged = folders(&mut case, Some(&cwd)).len();
    case.equal("an unchanged folder is not stored again", json!(unchanged), json!(1));
    write(&git.join("HEAD"), b"ref: refs/heads/feature-telescope\n");
    run(&mut case);
    let changed = folders(&mut case, Some(&cwd));
    case.equal("a change adds an observation", json!(changed.len()), json!(2));
    case.equal("newest first", changed[0]["git"]["branch"].clone(), json!("feature-telescope"));
    fs::remove_dir_all(&repository).unwrap();
    run(&mut case);
    let deleted = folders(&mut case, Some(&cwd));
    case.equal("a deleted folder is a change", json!(deleted.len()), json!(3));
    case.equal(
        "newest says it is gone",
        json!([deleted[0]["exists"], deleted[0]["git"]]),
        json!([false, null]),
    );
    case.truth(
        "observations are newest first",
        deleted
            .windows(2)
            .all(|pair| pair[0]["observed_at"].as_i64() >= pair[1]["observed_at"].as_i64()),
    );
    let everything = folders(&mut case, None).len();
    case.equal("all folders are listed without --path", json!(everything), json!(4));
    case.truth("git is never run", !sentinel.exists());
}

#[test]
fn a_linked_worktree_reports_the_main_repository_as_its_common_dir() {
    let mut case = Case::new("contract-worktree-observation");
    let main = base(&case).join("repositories/main");
    let worktree = base(&case).join("repositories/main-feature");
    fs::create_dir_all(&worktree).unwrap();
    let git =
        git_repository(&main, "main", &[("origin", "https://example.invalid/synthetic/main.git")]);
    git_worktree(&git, "main-feature", &worktree, "feature-worktree");
    let root = base(&case).join("pi");
    pi(&root, worktree.to_str().unwrap(), "pi-in-worktree", "worktreeneedle");
    case.fixture(json!({"seed":"contract-worktree-observation-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal("backup captures the file", report["captured"].clone(), json!(1));
    let entries = folders(&mut case, Some(&worktree));
    case.equal("one observation", json!(entries.len()), json!(1));
    let observed = &entries[0]["git"];
    case.truth("top_level is the worktree", same_place(&observed["top_level"], &worktree));
    case.truth(
        "common_dir is the main repository's git dir",
        same_place(&observed["common_dir"], &git),
    );
    case.equal("the worktree's own branch", observed["branch"].clone(), json!("feature-worktree"));
    case.equal(
        "remotes of the main repository",
        observed["remotes"].clone(),
        json!(["https://example.invalid/synthetic/main.git"]),
    );
}

#[test]
fn remote_urls_are_stored_without_userinfo() {
    let mut case = Case::unencrypted("contract-remote-userinfo");
    let repository = base(&case).join("projects/credentials");
    fs::create_dir_all(&repository).unwrap();
    git_repository(
        &repository,
        "main",
        &[
            ("origin", "https://synthetic-user:tokenuserinfoneedle@example.invalid/a/b.git"),
            ("mirror", "http://mirroruserinfoneedle@example.invalid/c.git"),
        ],
    );
    let root = base(&case).join("pi");
    pi(&root, repository.to_str().unwrap(), "pi-with-token-remote", "remoteneedle");
    case.fixture(json!({"seed":"contract-remote-userinfo-v1","private_data":false}));
    let control = case.control(source_arg("pi", &root));
    case.equal("control captures the file", control.backup["captured"].clone(), json!(1));
    case.init();
    backup(&mut case, &source_arg("pi", &root));
    let entries = folders(&mut case, Some(&repository));
    let mut remotes = entries[0]["git"]["remotes"].as_array().cloned().unwrap_or_default();
    remotes.sort_by_key(Value::to_string);
    case.equal(
        "userinfo is removed",
        json!(remotes),
        json!(["http://example.invalid/c.git", "https://example.invalid/a/b.git"]),
    );
    case.truth(
        "the stored remote is findable, so the scan can fail",
        archive_contains(&case.archive, b"example.invalid/a/b.git"),
    );
    for secret in ["tokenuserinfoneedle", "mirroruserinfoneedle", "synthetic-user"] {
        case.truth(
            "userinfo appears in no archive file",
            !archive_contains(&case.archive, secret.as_bytes()),
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinked_or_oversized_git_files_are_not_read() {
    use std::os::unix::fs::symlink;
    let mut case = Case::unencrypted("contract-git-symlinks");
    let projects = base(&case).join("projects");
    let outside = base(&case).join("outside");
    let real = git_repository(
        &outside.join("real"),
        "main",
        &[("origin", "https://example.invalid/symlinkedgitdirneedle.git")],
    );
    let linked_git = projects.join("linked-git");
    fs::create_dir_all(&linked_git).unwrap();
    symlink(&real, linked_git.join(".git")).unwrap();
    let linked_head = projects.join("linked-head");
    let git = git_repository(&linked_head, "main", &[]);
    write(&outside.join("HEAD"), b"ref: refs/heads/symlinkedheadneedle\n");
    fs::remove_file(git.join("HEAD")).unwrap();
    symlink(outside.join("HEAD"), git.join("HEAD")).unwrap();
    let linked_config = projects.join("linked-config");
    let git = git_repository(&linked_config, "main", &[]);
    write(
        &outside.join("config"),
        git_config(&[("origin", "https://example.invalid/symlinkedconfigneedle.git")]).as_bytes(),
    );
    fs::remove_file(git.join("config")).unwrap();
    symlink(outside.join("config"), git.join("config")).unwrap();
    let oversized = projects.join("oversized-config");
    let git = git_repository(&oversized, "main", &[]);
    let mut config = git_config(&[]);
    while config.len() <= 1024 * 1024 {
        config.push_str("# synthetic padding line that keeps the remote past the cap\n");
    }
    config.push_str("[remote \"late\"]\n\turl = https://example.invalid/beyondcapneedle.git\n");
    write(&git.join("config"), config.as_bytes());
    let ordinary = projects.join("ordinary");
    git_repository(&ordinary, "main", &[("origin", "https://example.invalid/ordinary.git")]);
    // Symlinked folders get a root of their own, so their run's issues can be told
    // apart from the oversized config's.
    let linked_root = base(&case).join("pi-symlinked");
    let other_root = base(&case).join("pi-others");
    for (root, folder) in [
        (&linked_root, &linked_git),
        (&linked_root, &linked_head),
        (&linked_root, &linked_config),
        (&other_root, &oversized),
        (&other_root, &ordinary),
    ] {
        let name = folder.file_name().unwrap().to_str().unwrap();
        pi(root, folder.to_str().unwrap(), &format!("pi-{name}"), "symlinkcaseneedle");
    }
    case.fixture(json!({"seed":"contract-git-symlinks-v1","private_data":false}));
    let mut both = source_arg("pi", &linked_root);
    both.extend(source_arg("pi", &other_root));
    let control = case.control(both.clone());
    case.equal("control captures five files", control.backup["captured"].clone(), json!(5));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &linked_root));
    let has = |report: &Value, code: &str| {
        report["issues"].as_array().unwrap().iter().any(|issue| issue["code"] == json!(code))
    };
    case.truth(
        "a skipped symlink is the run issue git_symlink_skipped",
        has(&report, "git_symlink_skipped"),
    );
    case.truth("the symlink-only run has no other git issue", !has(&report, "git_file_too_large"));
    let report = backup(&mut case, &both);
    case.truth(
        "an oversized git file is the run issue git_file_too_large",
        has(&report, "git_file_too_large"),
    );
    let ordinary_git = folders(&mut case, Some(&ordinary))[0]["git"].clone();
    case.equal(
        "an ordinary repository is read",
        ordinary_git["remotes"].clone(),
        json!(["https://example.invalid/ordinary.git"]),
    );
    for folder in [&linked_git, &linked_head, &linked_config] {
        let entries = folders(&mut case, Some(folder));
        case.equal("a symlinked git file yields git: null", entries[0]["git"].clone(), Value::Null);
    }
    let capped = folders(&mut case, Some(&oversized))[0]["git"].clone();
    case.equal("a git file over 1 MiB yields git: null", capped, Value::Null);
    for needle in
        ["symlinkedgitdirneedle", "symlinkedheadneedle", "symlinkedconfigneedle", "beyondcapneedle"]
    {
        case.truth(
            "an unread file's content is nowhere in the archive",
            !archive_contains(&case.archive, needle.as_bytes()),
        );
    }
}

#[cfg(unix)]
#[test]
fn backup_with_no_git_reads_no_git_file() {
    let mut case = Case::new("contract-no-git");
    let repository = base(&case).join("projects/no-git");
    git_repository(&repository, "main", &[("origin", "https://example.invalid/no-git.git")]);
    let root = base(&case).join("pi");
    pi(&root, repository.to_str().unwrap(), "pi-no-git", "nogitneedle");
    let (path, sentinel) = fake_git(&case);
    case.fixture(json!({"seed":"contract-no-git-v1","private_data":false}));
    case.init();
    let mut arguments = source_arg("pi", &root);
    arguments.extend(args(&["--no-git", "--execute"]));
    case.call_with_env("backup", arguments, &[("PATH", path.as_str())]);
    let first = folders(&mut case, Some(&repository));
    case.equal(
        "--no-git records the folder with git: null",
        json!([first.len(), first[0]["exists"], first[0]["git"]]),
        json!([1, true, null]),
    );

    backup(&mut case, &source_arg("pi", &root));
    let second = folders(&mut case, Some(&repository));
    case.equal(
        "without --no-git the same folder has git facts",
        second[0]["git"]["branch"].clone(),
        json!("main"),
    );
    case.truth("git is never run", !sentinel.exists());
}

#[cfg(unix)]
#[test]
fn a_folder_reached_through_a_symlinked_parent_still_gets_its_git_facts() {
    use std::os::unix::fs::symlink;
    let mut case = Case::new("contract-git-symlinked-parent");
    // The way `/var` or a linked `~/code` looks: a link above the folder, none inside `.git`.
    let real = base(&case).join("volumes/disk/code");
    let repository = real.join("orbit");
    fs::create_dir_all(repository.join("src")).unwrap();
    let git = git_repository(
        &repository,
        "main",
        &[("origin", "https://example.invalid/synthetic/orbit.git")],
    );
    let link = base(&case).join("code");
    symlink(&real, &link).unwrap();
    let cwd = link.join("orbit/src");
    let root = base(&case).join("pi");
    pi(&root, cwd.to_str().unwrap(), "pi-through-a-link", "linkedparentneedle");
    case.fixture(json!({"seed":"contract-git-symlinked-parent-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    case.equal(
        "a link above the folder is not a skipped git symlink",
        json!(
            report["issues"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|issue| issue["code"].as_str().is_some_and(|code| code.starts_with("git_")))
                .count()
        ),
        json!(0),
    );
    let entries = folders(&mut case, Some(&cwd));
    case.equal("one observation", json!(entries.len()), json!(1));
    let entry = &entries[0];
    case.equal("path is as the session recorded it", entry["path"].clone(), json!(cwd.to_str()));
    case.truth("top_level is the repository", same_place(&entry["git"]["top_level"], &repository));
    case.truth("common_dir is its .git", same_place(&entry["git"]["common_dir"], &git));
    case.equal("branch from HEAD", entry["git"]["branch"].clone(), json!("main"));
    case.equal(
        "remotes from config",
        entry["git"]["remotes"].clone(),
        json!(["https://example.invalid/synthetic/orbit.git"]),
    );
}

#[cfg(unix)]
#[test]
fn a_git_file_that_cannot_be_read_is_reported_not_passed_over() {
    use std::os::unix::fs::PermissionsExt;
    let mut case = Case::new("contract-git-unreadable");
    let repository = base(&case).join("projects/locked");
    let git = git_repository(
        &repository,
        "main",
        &[("origin", "https://example.invalid/synthetic/locked.git")],
    );
    fs::set_permissions(git.join("config"), fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(git.join("config")).is_ok() {
        // Running as a user no permission bit stops: there is nothing to observe here.
        return;
    }
    let root = base(&case).join("pi");
    pi(&root, repository.to_str().unwrap(), "pi-in-locked-repository", "lockedconfigneedle");
    case.fixture(json!({"seed":"contract-git-unreadable-v1","private_data":false}));
    case.init();
    let report = backup(&mut case, &source_arg("pi", &root));
    let issues = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|issue| issue["code"] == json!("git_unreadable"))
        .collect::<Vec<_>>();
    case.equal("the run says it could not read the git facts", json!(issues.len()), json!(1));
    case.equal(
        "and names the folder",
        issues[0]["path"].clone(),
        json!(repository.to_str().unwrap()),
    );
    let entries = folders(&mut case, Some(&repository));
    case.equal("the folder is recorded without git facts", entries[0]["git"].clone(), Value::Null);
    fs::set_permissions(git.join("config"), fs::Permissions::from_mode(0o644)).unwrap();
}
