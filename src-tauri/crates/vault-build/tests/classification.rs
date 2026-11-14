//! Responsibilities: pin classification, override precedence and released identity literals.
//! Not responsible for: opening platform folders or running native integrations.
use std::{ffi::OsString, path::PathBuf};
use vault_build::{BuildClass, CURRENT, classify, resolve_root};

#[test]
fn distribution_requires_designation_and_disabled_debug_assertions() {
    assert_eq!(classify(false, true), BuildClass::Development);
    assert_eq!(classify(false, false), BuildClass::Development);
    assert_eq!(classify(true, true), BuildClass::Development);
    assert_eq!(classify(true, false), BuildClass::Distribution);
    assert_eq!(CURRENT, BuildClass::Development);
}

#[test]
fn explicit_root_is_unchanged_and_empty_is_never_a_fallback() {
    for class in [BuildClass::Development, BuildClass::Distribution] {
        let explicit = OsString::from("synthetic/archive with spaces/../chosen");
        assert_eq!(
            resolve_root(class, Some(explicit.clone()), |_| panic!(
                "override must not resolve defaults"
            ))
            .unwrap(),
            PathBuf::from(explicit)
        );
        assert!(
            resolve_root(class, Some(OsString::new()), |_| panic!(
                "empty override must not resolve defaults"
            ))
            .unwrap_err()
            .to_string()
            .contains("empty-project-root")
        );
        assert_eq!(
            resolve_root(class, Some(" ".into()), |_| panic!("nonempty bytes are preserved"))
                .unwrap(),
            PathBuf::from(" ")
        );
    }
}

#[test]
fn each_default_uses_its_own_project_directory_name_without_io() {
    let synthetic = PathBuf::from("synthetic/defaults");
    for (class, application) in
        [(BuildClass::Development, "pathkeep-dev"), (BuildClass::Distribution, "pathkeep")]
    {
        let result = resolve_root(class, None, |identity| {
            assert_eq!(
                (identity.qualifier, identity.organization, identity.application),
                ("com", "yi-ting", application)
            );
            Some(synthetic.join(identity.application))
        })
        .unwrap();
        assert_eq!(result, synthetic.join(application));
    }
}

#[test]
fn released_names_remain_literal_and_development_names_do_not_alias_them() {
    assert_eq!(vault_build::ARCHIVE_KEY_ACCOUNT, "database-key");
    assert_eq!(vault_build::PROVIDER_KEY_ACCOUNT_PREFIX, "ai-provider::");
    let release = BuildClass::Distribution.identity();
    assert_eq!(release.application, "pathkeep");
    assert_eq!(release.bundle_identifier, "com.yi-ting.pathkeep");
    assert_eq!(release.keyring_service, "com.yi-ting.pathkeep");
    assert_eq!(release.schedule_label, "com.yi-ting.pathkeep.backup");
    assert_eq!(release.login_item_label, "com.yi-ting.pathkeep.login");
    assert_eq!(release.autostart_name, "PathKeep");
    assert_eq!(release.windows_upgrade_code, "818daeb2-ee49-5696-a1db-bee51050939c");
    assert_eq!(
        release.legacy_schedule_labels,
        ["dev.codex.pathkeep.backup", "dev.codex.browser-history-backup.backup"]
    );
    let dev = BuildClass::Development.identity();
    assert_eq!(dev.application, "pathkeep-dev");
    assert_eq!(dev.keyring_service, "com.yi-ting.pathkeep.dev");
    assert_eq!(dev.schedule_label, "com.yi-ting.pathkeep.dev.backup");
    assert_eq!(dev.login_item_label, "com.yi-ting.pathkeep.dev.login");
    assert_eq!(dev.autostart_name, "PathKeep Dev");
    assert_ne!(dev.bundle_identifier, release.bundle_identifier);
    assert_ne!(dev.windows_upgrade_code, release.windows_upgrade_code);
    assert!(dev.legacy_schedule_labels.is_empty());
}
