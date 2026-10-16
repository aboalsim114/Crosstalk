//! Where the API key lives: the Windows Credential Manager (in memory
//! elsewhere). Implementations never log the key or return it to the UI.

use std::sync::{Arc, Mutex};

/// Longest key accepted (real keys are well under 200 characters).
const MAX_KEY_LEN: usize = 512;

/// Where the API key is kept. Implementations never log or return it to the UI.
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, key: &str) -> Result<(), String>;
    fn clear(&self) -> Result<(), String>;
    /// Shown in the status ("Gestionnaire d'identifiants Windows").
    fn label(&self) -> &'static str;
}

/// Kept for the session only (tests, and outside Windows).
#[derive(Default)]
pub struct MemoryKeyStore(Mutex<Option<String>>);

impl KeyStore for MemoryKeyStore {
    fn get(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    fn set(&self, key: &str) -> Result<(), String> {
        *self.0.lock().unwrap() = Some(key.to_string());
        Ok(())
    }
    fn clear(&self) -> Result<(), String> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
    fn label(&self) -> &'static str {
        "mémoire (cette session seulement)"
    }
}

/// The Windows Credential Manager: a generic credential, local to this
/// Windows account and machine ("Crosstalk/llm-api-key" in the app).
#[cfg(windows)]
pub struct CredentialStore {
    pub target: &'static str,
}

#[cfg(windows)]
impl CredentialStore {
    pub const TARGET: &'static str = "Crosstalk/llm-api-key";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
impl KeyStore for CredentialStore {
    fn get(&self) -> Option<String> {
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{
            CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };
        let target = Self::wide(self.target);
        let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `target` is a NUL-terminated UTF-16 string that outlives the
        // call; on success `cred` points to a CREDENTIALW allocated by the
        // system, read once below and released with CredFree.
        unsafe {
            CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None, &mut cred).ok()?;
            if cred.is_null() {
                return None;
            }
            let c = &*cred;
            let blob = if c.CredentialBlob.is_null() || c.CredentialBlobSize == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
            };
            CredFree(cred as *const core::ffi::c_void);
            String::from_utf8(blob).ok().filter(|k| !k.is_empty())
        }
    }

    fn set(&self, key: &str) -> Result<(), String> {
        use windows::core::PWSTR;
        use windows::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };
        let mut target = Self::wide(self.target);
        let mut user = Self::wide("crosstalk");
        let mut blob = key.as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: every pointer in `cred` refers to a buffer alive until the
        // end of this function; CredWriteW copies them.
        let r = unsafe { CredWriteW(&cred, 0) };
        blob.iter_mut().for_each(|b| *b = 0);
        r.map_err(|e| format!("Gestionnaire d'identifiants : {}", e.message()))
    }

    fn clear(&self) -> Result<(), String> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::ERROR_NOT_FOUND;
        use windows::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};
        let target = Self::wide(self.target);
        // SAFETY: `target` is NUL-terminated and alive during the call.
        match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(e) if e.code() == ERROR_NOT_FOUND.to_hresult() => Ok(()),
            Err(e) => Err(format!("Gestionnaire d'identifiants : {}", e.message())),
        }
    }

    fn label(&self) -> &'static str {
        "Gestionnaire d'identifiants Windows"
    }
}

/// The store used by the app: the Credential Manager on Windows.
pub fn default_key_store() -> Arc<dyn KeyStore> {
    #[cfg(windows)]
    {
        Arc::new(CredentialStore {
            target: CredentialStore::TARGET,
        })
    }
    #[cfg(not(windows))]
    {
        Arc::new(MemoryKeyStore::default())
    }
}

/// A key as typed (pasted keys often carry spaces or a line break).
pub fn clean_key(key: &str) -> Result<String, String> {
    let k = key.trim();
    if k.is_empty() {
        return Err("clé vide".into());
    }
    if k.len() > MAX_KEY_LEN || k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("clé invalide (un seul mot, sans espace)".into());
    }
    Ok(k.to_string())
}
