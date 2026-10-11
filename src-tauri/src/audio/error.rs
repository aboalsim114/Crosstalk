//! Capture errors, with stable codes for the UI and French messages for users.

use super::types::ErrorInfo;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("Counter-Strike 2 n’est pas lancé : démarrez le jeu ou choisissez le périphérique de sortie.")]
    Cs2NotRunning,
    #[error("La capture audio n’est disponible que sous Windows.")]
    Unsupported,
    #[error("Aucun périphérique de sortie audio actif.")]
    NoDevice,
    #[error("Périphérique audio introuvable : {0}")]
    DeviceNotFound(String),
    #[error("Capture par processus indisponible sur ce système ({0}).")]
    ProcessLoopbackUnavailable(String),
    #[error("Format audio refusé par Windows ({0}).")]
    FormatUnsupported(String),
    #[error("Le périphérique audio a été retiré ou désactivé.")]
    DeviceInvalidated,
    #[error("Le service audio de Windows ne répond pas.")]
    ServiceNotRunning,
    #[error("Délai dépassé : {0}")]
    Timeout(&'static str),
    #[error("Erreur audio Windows : {0}")]
    Os(String),
}

impl AudioError {
    pub fn code(&self) -> &'static str {
        match self {
            AudioError::Cs2NotRunning => "cs2_not_running",
            AudioError::Unsupported => "unsupported",
            AudioError::NoDevice => "no_device",
            AudioError::DeviceNotFound(_) => "device_not_found",
            AudioError::ProcessLoopbackUnavailable(_) => "process_loopback_unavailable",
            AudioError::FormatUnsupported(_) => "format_unsupported",
            AudioError::DeviceInvalidated => "device_invalidated",
            AudioError::ServiceNotRunning => "service_not_running",
            AudioError::Timeout(_) => "timeout",
            AudioError::Os(_) => "os",
        }
    }

    /// Errors after which reopening the same source may succeed.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AudioError::DeviceInvalidated | AudioError::ServiceNotRunning
        )
    }

    pub fn info(&self) -> ErrorInfo {
        ErrorInfo {
            code: self.code(),
            message: self.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable_and_messages_french() {
        assert_eq!(AudioError::Cs2NotRunning.code(), "cs2_not_running");
        assert!(AudioError::Cs2NotRunning
            .to_string()
            .contains("Counter-Strike 2"));
        let info = AudioError::DeviceNotFound("abc".into()).info();
        assert_eq!(info.code, "device_not_found");
        assert!(info.message.ends_with("abc"));
    }

    #[test]
    fn only_device_loss_is_recoverable() {
        assert!(AudioError::DeviceInvalidated.is_recoverable());
        assert!(AudioError::ServiceNotRunning.is_recoverable());
        assert!(!AudioError::Cs2NotRunning.is_recoverable());
        assert!(!AudioError::FormatUnsupported("x".into()).is_recoverable());
    }
}
