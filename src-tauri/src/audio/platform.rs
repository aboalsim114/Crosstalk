//! Platform facts used to choose and explain the capture source.

use serde::Serialize;

use super::plan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    /// Windows build number (e.g. 19045 for Windows 10 22H2), `None` elsewhere.
    pub windows_build: Option<u32>,
    /// Whether process loopback is documented for this build. It is still tried
    /// on older builds; the actual result is what counts.
    pub process_loopback_documented: Option<bool>,
}

pub fn info() -> PlatformInfo {
    let windows_build = windows_build();
    PlatformInfo {
        windows_build,
        process_loopback_documented: plan::process_loopback_documented(windows_build),
    }
}

/// Real build number (`RtlGetVersion` is not subject to the compatibility shims
/// that make `GetVersionEx` lie).
#[cfg(windows)]
pub fn windows_build() -> Option<u32> {
    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut v = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `v` is a properly sized, writable OSVERSIONINFOW.
    let status = unsafe { RtlGetVersion(&mut v) };
    status.is_ok().then_some(v.dwBuildNumber)
}

#[cfg(not(windows))]
pub fn windows_build() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_is_consistent() {
        let i = info();
        assert_eq!(
            i.process_loopback_documented.is_some(),
            i.windows_build.is_some()
        );
        #[cfg(windows)]
        assert!(i.windows_build.unwrap() >= 10240, "Windows 10 or later");
    }
}
