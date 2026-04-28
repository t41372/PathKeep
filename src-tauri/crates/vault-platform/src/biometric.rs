//! App Lock biometric adapter.
//!
//! Biometric unlock is intentionally scoped to the desktop session boundary.
//! It can unlock the current App Lock session when the host supports it, but it
//! does not replace archive encryption or native keyring guarantees.

use vault_core::AppLockBiometricState;

/// Reports whether biometric unlock is available in the current desktop build.
pub fn app_lock_biometric_state() -> AppLockBiometricState {
    platform::app_lock_biometric_state()
}

/// Attempts to authenticate the current user with the host biometric prompt.
pub fn authenticate_app_lock_biometric() -> Result<(), String> {
    platform::authenticate_app_lock_biometric()
}

/// Translates platform Touch ID errors into user-facing App Lock messages.
#[cfg_attr(coverage, allow(dead_code))]
fn map_touch_id_error(code: Option<isize>, description: Option<String>) -> String {
    match code {
        Some(-1) => "Touch ID could not verify your identity. Try again or use the app lock passcode."
            .to_string(),
        Some(-2) => "Touch ID unlock was canceled.".to_string(),
        Some(-3) => "Touch ID was skipped. Use the app lock passcode instead.".to_string(),
        Some(-4) => {
            "Touch ID unlock was interrupted by macOS. Try again or use the app lock passcode."
                .to_string()
        }
        Some(-5) | Some(-6) | Some(-12) | Some(-13) => {
            "Touch ID is unavailable on this Mac right now. Use the app lock passcode instead."
                .to_string()
        }
        Some(-7) => {
            "Touch ID is available on this Mac, but no fingerprints are enrolled. Use the app lock passcode instead."
                .to_string()
        }
        Some(-8) => {
            "Touch ID is locked out on this Mac right now. Unlock it in macOS or use the app lock passcode instead."
                .to_string()
        }
        Some(-9) => "Touch ID unlock was canceled by PathKeep.".to_string(),
        Some(-10) => "Touch ID unlock is no longer valid. Try again.".to_string(),
        _ => description.unwrap_or_else(|| {
            "Touch ID unlock failed. Use the app lock passcode instead.".to_string()
        }),
    }
}

#[cfg(all(target_os = "macos", not(coverage)))]
mod platform {
    use super::map_touch_id_error;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};
    use std::{sync::mpsc, time::Duration};
    use vault_core::AppLockBiometricState;

    const TOUCH_ID_PROMPT_TIMEOUT: Duration = Duration::from_secs(90);

    /// Returns Touch ID availability for the running macOS session.
    pub fn app_lock_biometric_state() -> AppLockBiometricState {
        let context = unsafe { LAContext::new() };
        match unsafe {
            context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
        } {
            Ok(()) => AppLockBiometricState::TouchIdAvailable,
            Err(_) => AppLockBiometricState::TouchIdUnavailable,
        }
    }

    /// Presents the Touch ID prompt and maps platform failures into product copy.
    pub fn authenticate_app_lock_biometric() -> Result<(), String> {
        let context = unsafe { LAContext::new() };
        match unsafe {
            context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
        } {
            Ok(()) => {}
            Err(error) => {
                return Err(map_touch_id_error(
                    Some(error.code()),
                    Some(error.localizedDescription().to_string()),
                ));
            }
        }

        let (sender, receiver) = mpsc::channel();
        let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
            let code = if error.is_null() { None } else { Some(unsafe { &*error }.code()) };
            let description = if error.is_null() {
                None
            } else {
                Some(unsafe { &*error }.localizedDescription().to_string())
            };
            let _ = sender.send((success.as_bool(), code, description));
        });
        let reason = NSString::from_str("unlock the current PathKeep session with Touch ID");

        unsafe {
            context.evaluatePolicy_localizedReason_reply(
                LAPolicy::DeviceOwnerAuthenticationWithBiometrics,
                &reason,
                &reply,
            );
        }

        match receiver.recv_timeout(TOUCH_ID_PROMPT_TIMEOUT) {
            Ok((true, _, _)) => Ok(()),
            Ok((false, code, description)) => Err(map_touch_id_error(code, description)),
            Err(_) => Err(
                "Touch ID did not finish before PathKeep timed out. Try again or use the app lock passcode."
                    .to_string(),
            ),
        }
    }
}

#[cfg(any(coverage, not(target_os = "macos")))]
mod platform {
    use vault_core::AppLockBiometricState;

    /// Reports that biometric unlock is unsupported on this build target or in
    /// deterministic coverage runs.
    pub fn app_lock_biometric_state() -> AppLockBiometricState {
        AppLockBiometricState::Unsupported
    }

    /// Returns an explicit unsupported error on non-macOS builds.
    pub fn authenticate_app_lock_biometric() -> Result<(), String> {
        Err("Biometric unlock is not available in the current desktop build.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::map_touch_id_error;
    #[cfg(coverage)]
    use super::{app_lock_biometric_state, authenticate_app_lock_biometric};
    #[cfg(coverage)]
    use vault_core::AppLockBiometricState;

    #[test]
    fn maps_touch_id_errors_to_truthful_messages() {
        for code in [-1, -2, -3, -4, -5, -6, -7, -8, -9, -10, -12, -13] {
            assert!(
                !map_touch_id_error(Some(code), None).is_empty(),
                "code {code} should map to copy"
            );
        }
        assert!(map_touch_id_error(Some(-2), None).contains("canceled"));
        assert!(map_touch_id_error(Some(-7), None).contains("no fingerprints"));
        assert!(map_touch_id_error(Some(-8), None).contains("locked out"));
        assert_eq!(map_touch_id_error(None, Some("Custom failure".to_string())), "Custom failure");
        assert_eq!(
            map_touch_id_error(Some(999), None),
            "Touch ID unlock failed. Use the app lock passcode instead."
        );
    }

    #[cfg(coverage)]
    #[test]
    fn coverage_biometric_adapter_uses_non_prompting_contract() {
        assert_eq!(app_lock_biometric_state(), AppLockBiometricState::Unsupported);
        let error =
            authenticate_app_lock_biometric().expect_err("coverage adapter should not prompt");
        assert!(error.contains("not available"));
    }
}
