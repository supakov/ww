//! Where a plugin comes from and whether it loads: the SHA-256 of its DLL and its Authenticode
//! signature embedded in the file (`origin_of`), and the setting that loads only the plugins it
//! trusts (`Trust`): the ones whose SHA-256 it lists and, when it says so, the ones whose
//! signature Windows trusts. The signature is checked without the network (no revocation); a
//! signature in a catalog of Windows (its own DLLs) is not looked at.

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The plugins a program loads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trust {
    /// Only the plugins trusted load.
    pub only_trusted: bool,
    /// A plugin whose embedded signature Windows trusts is trusted too.
    pub signed: bool,
    /// The SHA-256 of the DLLs trusted, in hexadecimal.
    pub sha256: Vec<String>,
}

/// The Authenticode signature of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signature {
    None,
    /// Trusted by Windows, with the name of its signer.
    Trusted(String),
    /// There, but Windows does not trust it, with the reason.
    Untrusted(String),
}

/// Where a DLL comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub sha256: String,
    pub signature: Signature,
}

impl Trust {
    /// Whether the SHA-256 is in the list.
    pub fn listed(&self, sha256: &str) -> bool {
        self.sha256.iter().any(|s| s.eq_ignore_ascii_case(sha256))
    }

    /// Puts the SHA-256 into the list or takes it out.
    pub fn set(&mut self, sha256: &str, trusted: bool) {
        self.sha256.retain(|s| !s.eq_ignore_ascii_case(sha256));
        if trusted {
            self.sha256.push(sha256.to_ascii_lowercase());
        }
    }

    /// Whether a DLL of this origin loads, or why not.
    pub fn admits(&self, origin: &Origin) -> Result<(), String> {
        if !self.only_trusted || self.listed(&origin.sha256) {
            return Ok(());
        }
        if self.signed && matches!(origin.signature, Signature::Trusted(_)) {
            return Ok(());
        }
        Err(format!(
            "not trusted: its SHA-256 {} is not in the list of the trusted plugins",
            origin.sha256
        ))
    }
}

/// The SHA-256 of a file, in hexadecimal, read a block at a time.
pub fn sha256_of(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut block = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut block).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&block[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Where a DLL comes from: its SHA-256 and its signature.
pub fn origin_of(path: &Path) -> Result<Origin, String> {
    Ok(Origin {
        sha256: sha256_of(path)?,
        signature: signature(path),
    })
}

/// The Authenticode signature embedded in a file, as Windows judges it.
#[cfg(windows)]
pub fn signature(path: &Path) -> Signature {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::Security::Cryptography::{
        CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW,
    };
    use windows::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvSignerFromChain,
        WTHelperProvDataFromStateData, WinVerifyTrust,
    };
    use windows::core::{HSTRING, PCWSTR};

    /// TRUST_E_NOSIGNATURE, TRUST_E_SUBJECT_FORM_UNKNOWN and TRUST_E_PROVIDER_UNKNOWN: no
    /// signature of a kind Windows reads in the file.
    const NONE: [u32; 3] = [0x800B_0100, 0x800B_0003, 0x800B_0001];

    let wide = HSTRING::from(path);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 {
            pFile: &raw mut file,
        },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: the structures are filled as WinVerifyTrust documents them and live for the calls;
    // the state the first call keeps is read before the second call closes it.
    unsafe {
        let result = WinVerifyTrust(
            HWND(-1isize as *mut std::ffi::c_void),
            &raw mut action,
            (&raw mut data).cast(),
        ) as u32;
        let signer = if result == 0 {
            let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
            let sgnr = if provider.is_null() {
                std::ptr::null_mut()
            } else {
                WTHelperGetProvSignerFromChain(provider, 0, false, 0)
            };
            let mut name = String::new();
            if !sgnr.is_null() && (*sgnr).csCertChain > 0 && !(*sgnr).pasCertChain.is_null() {
                let cert = (*(*sgnr).pasCertChain).pCert;
                let mut buffer = vec![0u16; 512];
                let n = CertGetNameStringW(
                    cert,
                    CERT_NAME_SIMPLE_DISPLAY_TYPE,
                    0,
                    None,
                    Some(&mut buffer),
                ) as usize;
                name = String::from_utf16_lossy(&buffer[..n.saturating_sub(1).min(buffer.len())]);
            }
            Signature::Trusted(name)
        } else if NONE.contains(&result) {
            Signature::None
        } else {
            Signature::Untrusted(
                windows::core::Error::from_hresult(windows::core::HRESULT(result as i32)).message(),
            )
        };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrust(
            HWND(-1isize as *mut std::ffi::c_void),
            &raw mut action,
            (&raw mut data).cast(),
        );
        signer
    }
}

#[cfg(not(windows))]
pub fn signature(_path: &Path) -> Signature {
    Signature::None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(sha256: &str, signature: Signature) -> Origin {
        Origin {
            sha256: sha256.to_owned(),
            signature,
        }
    }

    #[test]
    fn only_the_plugins_trusted_load_when_the_setting_says_so() {
        let mut trust = Trust::default();
        let unsigned = origin("ab12", Signature::None);
        let signed = origin("cd34", Signature::Trusted("Someone".to_owned()));
        assert!(trust.admits(&unsigned).is_ok(), "every plugin by default");
        trust.only_trusted = true;
        let why = trust.admits(&unsigned).unwrap_err();
        assert!(why.contains("ab12"), "{why}");
        assert!(trust.admits(&signed).is_err());
        trust.signed = true;
        assert!(trust.admits(&signed).is_ok(), "a signature Windows trusts");
        trust.set("AB12", true);
        assert!(trust.admits(&unsigned).is_ok() && trust.listed("ab12"));
        assert_eq!(trust.sha256, ["ab12"]);
        trust.set("ab12", false);
        assert!(trust.sha256.is_empty());
    }

    #[test]
    fn the_sha256_of_a_file_and_no_signature() {
        let path = std::env::temp_dir().join(format!("wwrt-trust-{}.bin", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let o = origin_of(&path).unwrap();
        assert_eq!(
            o.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(o.signature, Signature::None);
        let _ = std::fs::remove_file(&path);
    }

    /// A program with a signature in its file, where this Windows has it.
    #[test]
    #[cfg(windows)]
    fn a_signature_in_a_file_names_its_signer() {
        let edge = Path::new(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
        if !edge.is_file() {
            eprintln!("no {} on this Windows", edge.display());
            return;
        }
        match signature(edge) {
            Signature::Trusted(name) => assert!(name.contains("Microsoft"), "{name}"),
            other => panic!("{other:?}"),
        }
    }
}
