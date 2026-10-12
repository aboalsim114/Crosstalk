use super::TranslationPlan;
use zeroize::Zeroizing;
#[derive(Debug)]
pub struct CredentialError;
pub trait CredentialStore: Send + Sync {
    fn supported(&self) -> bool;
    fn read(&self, plan: TranslationPlan) -> Result<Option<Zeroizing<String>>, CredentialError>;
    fn write(&self, plan: TranslationPlan, key: &str) -> Result<(), CredentialError>;
    fn delete(&self, plan: TranslationPlan) -> Result<(), CredentialError>;
}
pub struct WindowsCredentials;
#[cfg(windows)]
mod native {
    use super::*;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::*;
    use zeroize::Zeroize;
    fn target(plan: TranslationPlan) -> &'static str {
        match plan {
            TranslationPlan::Free => "Crosstalk/DeepL/Free",
            TranslationPlan::Pro => "Crosstalk/DeepL/Pro",
        }
    }
    fn wide(name: &str) -> Vec<u16> {
        name.encode_utf16().chain(Some(0)).collect()
    }
    struct OwnedCredential(*mut CREDENTIALW);
    impl Drop for OwnedCredential {
        fn drop(&mut self) {
            unsafe {
                let value = &mut *self.0;
                if value.CredentialBlobSize != 0 && !value.CredentialBlob.is_null() {
                    std::slice::from_raw_parts_mut(
                        value.CredentialBlob,
                        value.CredentialBlobSize as usize,
                    )
                    .zeroize();
                }
                CredFree(self.0.cast());
            }
        }
    }
    fn read_target(name: &str) -> Result<Option<Zeroizing<String>>, CredentialError> {
        let target = wide(name);
        let mut ptr = std::ptr::null_mut();
        match unsafe { CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut ptr) } {
            Ok(()) => {}
            Err(e) if e.code().0 as u32 == 0x80070490 => return Ok(None),
            Err(_) => return Err(CredentialError),
        }
        let owned = OwnedCredential(ptr);
        let value = unsafe { &*owned.0 };
        if value.CredentialBlobSize == 0
            || value.CredentialBlobSize > 4096
            || value.CredentialBlob.is_null()
        {
            return Err(CredentialError);
        }
        let bytes = Zeroizing::new(
            unsafe {
                std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize)
            }
            .to_vec(),
        );
        let result = std::str::from_utf8(&bytes)
            .map_err(|_| CredentialError)?
            .to_owned();
        Ok(Some(Zeroizing::new(result)))
    }
    fn write_target(name: &str, key: &str) -> Result<(), CredentialError> {
        let mut target = wide(name);
        let mut blob = Zeroizing::new(key.as_bytes().to_vec());
        let credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            ..Default::default()
        };
        unsafe { CredWriteW(&credential, 0) }.map_err(|_| CredentialError)
    }
    fn delete_target(name: &str) -> Result<(), CredentialError> {
        let target = wide(name);
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code().0 as u32 == 0x80070490 => Ok(()),
            Err(_) => Err(CredentialError),
        }
    }
    impl CredentialStore for WindowsCredentials {
        fn supported(&self) -> bool {
            true
        }
        fn read(
            &self,
            plan: TranslationPlan,
        ) -> Result<Option<Zeroizing<String>>, CredentialError> {
            read_target(target(plan))
        }
        fn write(&self, plan: TranslationPlan, key: &str) -> Result<(), CredentialError> {
            write_target(target(plan), key)
        }
        fn delete(&self, plan: TranslationPlan) -> Result<(), CredentialError> {
            delete_target(target(plan))
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = delete_target(&self.0);
            }
        }
        #[test]
        fn isolated_windows_credential_roundtrip() {
            let suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let name = format!("Crosstalk/TestOnly/{}/{suffix}", std::process::id());
            let cleanup = Cleanup(name);
            assert!(read_target(&cleanup.0).unwrap().is_none());
            write_target(&cleanup.0, "synthetic-local-test-key").unwrap();
            // Boolean assert avoids leaking even synthetic credentials on test failure.
            assert!(read_target(&cleanup.0)
                .unwrap()
                .is_some_and(|v| v.as_str() == "synthetic-local-test-key"));
            delete_target(&cleanup.0).unwrap();
            assert!(read_target(&cleanup.0).unwrap().is_none());
        }
    }
}
#[cfg(not(windows))]
impl CredentialStore for WindowsCredentials {
    fn supported(&self) -> bool {
        false
    }
    fn read(&self, _: TranslationPlan) -> Result<Option<Zeroizing<String>>, CredentialError> {
        Err(CredentialError)
    }
    fn write(&self, _: TranslationPlan, _: &str) -> Result<(), CredentialError> {
        Err(CredentialError)
    }
    fn delete(&self, _: TranslationPlan) -> Result<(), CredentialError> {
        Err(CredentialError)
    }
}
