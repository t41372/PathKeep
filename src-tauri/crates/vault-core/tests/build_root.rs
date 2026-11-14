//! Responsibilities: pin released path spelling and the real development root entry point.
//! Not responsible for: reading or creating the returned OS directories. Only path values are used.
use directories::ProjectDirs;
use std::path::Path;
use std::process::Command;

// Each environment case runs in its own child so parallel tests cannot change its root.
#[test]
fn development_entrypoint_uses_the_development_project_directory() {
    if std::env::var_os("PATHKEEP_ROOT_RESOLUTION_CHILD").is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "development_entrypoint_uses_the_development_project_directory"])
            .env("PATHKEEP_ROOT_RESOLUTION_CHILD", "1")
            .env_remove("CHB_PROJECT_ROOT")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let root = vault_core::project_paths().unwrap().app_root;
    #[cfg(target_os = "macos")]
    {
        assert!(root.ends_with("Library/Application Support/com.yi-ting.pathkeep-dev"), "{root:?}");
        assert!(!root.ends_with("Library/Application Support/com.yi-ting.pathkeep"));
    }
    #[cfg(target_os = "windows")]
    {
        assert!(root.ends_with("yi-ting/pathkeep-dev/data"), "{root:?}");
        assert!(!root.ends_with("yi-ting/pathkeep/data"));
    }
    #[cfg(target_os = "linux")]
    {
        assert!(root.ends_with("pathkeep-dev"), "{root:?}");
        assert!(!root.ends_with("pathkeep"));
    }
}

#[test]
fn empty_root_override_is_rejected_by_the_entrypoint() {
    if std::env::var_os("PATHKEEP_ROOT_RESOLUTION_CHILD").is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "empty_root_override_is_rejected_by_the_entrypoint"])
            .env("PATHKEEP_ROOT_RESOLUTION_CHILD", "1")
            .env("CHB_PROJECT_ROOT", "")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let error = vault_core::project_paths().unwrap_err();
    assert!(error.to_string().contains("empty-project-root"), "{error}");
}

#[test]
fn explicit_root_bytes_reach_the_real_entrypoint_unchanged() {
    let chosen = "synthetic/archive with spaces/../chosen";
    if std::env::var_os("PATHKEEP_ROOT_RESOLUTION_CHILD").is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "explicit_root_bytes_reach_the_real_entrypoint_unchanged"])
            .env("PATHKEEP_ROOT_RESOLUTION_CHILD", "1")
            .env("CHB_PROJECT_ROOT", chosen)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    assert_eq!(vault_core::project_paths().unwrap().app_root, Path::new(chosen));
}

#[test]
fn distribution_project_directories_keep_the_released_relative_path_bytes() {
    let identity = vault_build::BuildClass::Distribution.identity();
    let dirs =
        ProjectDirs::from(identity.qualifier, identity.organization, identity.application).unwrap();
    #[cfg(target_os = "macos")]
    {
        assert_eq!(dirs.project_path(), Path::new("com.yi-ting.pathkeep"));
        assert!(
            dirs.data_local_dir().ends_with("Library/Application Support/com.yi-ting.pathkeep")
        );
    }
    #[cfg(target_os = "windows")]
    {
        assert_eq!(dirs.project_path(), Path::new("yi-ting/pathkeep"));
        assert!(dirs.data_local_dir().ends_with("yi-ting/pathkeep/data"));
    }
    #[cfg(target_os = "linux")]
    {
        assert_eq!(dirs.project_path(), Path::new("pathkeep"));
        assert!(dirs.data_local_dir().ends_with("pathkeep"));
    }
}
