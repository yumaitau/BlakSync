//! Small Windows-only helpers: owner-only ACLs.

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED, ERROR_SUCCESS, HANDLE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::{Error, Result};

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

fn os_error(code: u32) -> Error {
    Error::Io(io::Error::from_raw_os_error(code as i32))
}

/// Replace the DACL with full control for the current user, SYSTEM, and
/// Administrators only, and stop inheriting from the parent. This is the
/// Windows counterpart of `chmod 0700` / `0600`. Directories pass the same
/// entries on to new children.
pub fn restrict_to_owner(path: &Path) -> Result<()> {
    let user = current_user_sid()?;
    let inherit = if path.is_dir() { "OICI" } else { "" };
    let sddl = format!("D:P(A;{inherit};FA;;;{user})(A;{inherit};FA;;;SY)(A;{inherit};FA;;;BA)");
    let sddl = wide(sddl);
    let target = wide(path);
    // SAFETY: every pointer passed is valid for the duration of each call, and
    // the descriptor allocated by Windows is released with LocalFree.
    unsafe {
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        ) == 0
        {
            return Err(Error::Io(io::Error::last_os_error()));
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl: *mut ACL = null_mut();
        let result = if GetSecurityDescriptorDacl(
            descriptor,
            &mut present,
            &mut dacl,
            &mut defaulted,
        ) == 0
        {
            Err(Error::Io(io::Error::last_os_error()))
        } else {
            match SetNamedSecurityInfoW(
                target.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                dacl,
                null(),
            ) {
                ERROR_SUCCESS => Ok(()),
                // FAT/exFAT USB drives have no ACLs, as chmod is moot there too.
                ERROR_NOT_SUPPORTED | ERROR_INVALID_FUNCTION => Ok(()),
                code => Err(os_error(code)),
            }
        };
        LocalFree(descriptor);
        result
    }
}

fn current_user_sid() -> Result<String> {
    // SAFETY: the token handle is closed before returning; the buffer outlives
    // every use of the SID pointer inside it; the SID string is freed with
    // LocalFree after it is copied.
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(Error::Io(io::Error::last_os_error()));
        }
        let mut length = 0u32;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut length);
        // u64 storage keeps TOKEN_USER suitably aligned.
        let mut buffer = vec![0u64; (length as usize).div_ceil(8).max(1)];
        let ok = GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        );
        let error = io::Error::last_os_error();
        CloseHandle(token);
        if ok == 0 {
            return Err(Error::Io(error));
        }
        let user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let mut text: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
            return Err(Error::Io(io::Error::last_os_error()));
        }
        let mut end = 0;
        while *text.add(end) != 0 {
            end += 1;
        }
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, end));
        LocalFree(text.cast());
        Ok(sid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_user_sid_is_a_user_sid() {
        assert!(current_user_sid().unwrap().starts_with("S-1-5-"));
    }

    #[test]
    fn restricts_a_file_and_directory() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("secret.pem");
        std::fs::write(&file, "x").unwrap();
        restrict_to_owner(temporary.path()).unwrap();
        restrict_to_owner(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "x");
    }
}
