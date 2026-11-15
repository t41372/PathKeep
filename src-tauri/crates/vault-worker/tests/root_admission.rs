//! Responsibilities: pin root-entry refusal and absence of effects through real backend APIs.
//! Not responsible for: implementing admission, publishing stamps, or proving native OS services.
#[path = "root_admission/control.rs"]
mod control;
#[path = "root_admission/operations.rs"]
mod operations;
#[path = "root_admission/registration.rs"]
mod registration;
#[path = "root_admission/support.rs"]
mod support;
