//! Which sources to try, in which order, for a capture request.

use super::error::AudioError;
use super::types::{CaptureMode, CaptureTarget};

/// First Windows build where process loopback is documented (Windows Server
/// 2022 / Windows 11). Older builds (Windows 10 22H2 is 19045) are still tried:
/// the result is reported, the fallback covers a failure.
pub const PROCESS_LOOPBACK_MIN_BUILD: u32 = 20348;

pub fn attempts(
    mode: CaptureMode,
    device_id: Option<&str>,
    cs2_pid: Option<u32>,
) -> Result<Vec<CaptureTarget>, AudioError> {
    let device = CaptureTarget::Device {
        id: device_id.map(str::to_owned),
    };
    match (mode, cs2_pid) {
        (CaptureMode::Auto, Some(pid)) => Ok(vec![CaptureTarget::Process { pid }, device]),
        (CaptureMode::Process, Some(pid)) => Ok(vec![CaptureTarget::Process { pid }]),
        // Capturing the whole device would also capture the user's other apps:
        // only do it when the user chose it or as an announced fallback.
        (CaptureMode::Auto | CaptureMode::Process, None) => Err(AudioError::Cs2NotRunning),
        (CaptureMode::Device, _) => Ok(vec![device]),
    }
}

/// Hint shown in Diagnostic: is process loopback documented for this build?
pub fn process_loopback_documented(windows_build: Option<u32>) -> Option<bool> {
    windows_build.map(|b| b >= PROCESS_LOOPBACK_MIN_BUILD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_tries_process_then_device() {
        let plan = attempts(CaptureMode::Auto, None, Some(42)).unwrap();
        assert_eq!(
            plan,
            vec![
                CaptureTarget::Process { pid: 42 },
                CaptureTarget::Device { id: None }
            ]
        );
    }

    #[test]
    fn auto_falls_back_to_the_chosen_device() {
        let plan = attempts(CaptureMode::Auto, Some("dev-1"), Some(7)).unwrap();
        assert_eq!(
            plan[1],
            CaptureTarget::Device {
                id: Some("dev-1".into())
            }
        );
    }

    #[test]
    fn process_modes_need_cs2() {
        assert_eq!(
            attempts(CaptureMode::Auto, None, None),
            Err(AudioError::Cs2NotRunning)
        );
        assert_eq!(
            attempts(CaptureMode::Process, None, None),
            Err(AudioError::Cs2NotRunning)
        );
        assert_eq!(
            attempts(CaptureMode::Process, None, Some(1)).unwrap(),
            vec![CaptureTarget::Process { pid: 1 }]
        );
    }

    #[test]
    fn device_mode_ignores_cs2() {
        for pid in [None, Some(3)] {
            assert_eq!(
                attempts(CaptureMode::Device, Some("x"), pid).unwrap(),
                vec![CaptureTarget::Device {
                    id: Some("x".into())
                }]
            );
        }
    }

    #[test]
    fn build_hint() {
        assert_eq!(process_loopback_documented(Some(19045)), Some(false));
        assert_eq!(process_loopback_documented(Some(22631)), Some(true));
        assert_eq!(process_loopback_documented(None), None);
    }
}
