//! Responsibilities: per-root named pipes with a protected current-user DACL.
//! Not responsible for: database ownership, elevation or remote access.
//! Failure modes: default DACL admits other users; pipe is accessible remotely;
//! busy pipe looks like a missing owner; first-instance collision replaces owner.
//! Tests: current_user_is_the_only_dacl_trustee; transport.rs on Windows.
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn current_user_is_the_only_dacl_trustee() {
        let sid = super::current_sid().unwrap();
        let sddl = super::sddl(&sid);
        assert_eq!(sddl, format!("D:P(A;;GA;;;{sid})"));
        assert!(sid.starts_with("S-1-"));
        super::create_pipe(std::path::Path::new("C:/PathKeep-test"), true).unwrap();
    }
}
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
    ptr,
};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        },
        GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

fn current_sid() -> io::Result<String> {
    unsafe {
        let mut token = ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut length = 0;
        GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut length);
        // TOKEN_USER needs pointer alignment, not a byte Vec's alignment.
        let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
        let ok =
            GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), length, &mut length);
        let error = io::Error::last_os_error();
        CloseHandle(token);
        if ok == 0 {
            return Err(error);
        }
        let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let mut text = ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut count = 0;
        while *text.add(count) != 0 {
            count += 1;
        }
        let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, count));
        LocalFree(text.cast());
        Ok(result)
    }
}
fn sddl(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})")
}
fn pipe_name(root: &Path, sid: &str) -> String {
    let hash = Sha256::digest(format!("{sid}\0{}", root.display()).as_bytes());
    format!(r"\\.\pipe\PathKeep-{:x}", hash)
}
fn create_pipe(root: &Path, first: bool) -> io::Result<NamedPipeServer> {
    let sid = current_sid()?;
    let sddl: Vec<u16> = sddl(&sid).encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut descriptor = ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                pipe_name(root, &sid),
                (&mut attributes as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        result
    }
}
pub struct Listener {
    pending: NamedPipeServer,
    root: PathBuf,
}
impl Listener {
    pub async fn bind(root: &Path) -> io::Result<Self> {
        Ok(Self { pending: create_pipe(root, true)?, root: root.into() })
    }
    pub async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        self.pending.connect().await?;
        // Create the next instance before moving the connected one to the handler.
        let next = create_pipe(&self.root, false)?;
        Ok(std::mem::replace(&mut self.pending, next))
    }
}
pub async fn connect(root: &Path) -> io::Result<NamedPipeClient> {
    let name = pipe_name(root, &current_sid()?);
    // ERROR_PIPE_BUSY means an owner exists; wait briefly without launching another app.
    for _ in 0..100 {
        match ClientOptions::new().open(&name) {
            Err(error) if error.raw_os_error() == Some(231) => {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await
            }
            result => return result,
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "PathKeep owner pipe remained busy"))
}
pub fn not_running(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}
