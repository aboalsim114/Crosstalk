//! Text recognition with the OCR engine built into Windows 10 / 11
//! (Windows.Media.Ocr): local, nothing to download, nothing sent anywhere.
//! It reads the languages installed in Windows; Cyrillic names need the
//! Russian language (Settings › Time & language › Language).

use super::screen::Frame;

#[cfg(windows)]
pub struct Ocr {
    engine: windows::Media::Ocr::OcrEngine,
    /// BCP-47 tag of the recognizer, e.g. `fr-FR`.
    pub lang: String,
}

#[cfg(windows)]
impl Ocr {
    pub fn new() -> Result<Self, String> {
        use windows::Media::Ocr::OcrEngine;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        // WinRT needs COM on this thread (already initialised: harmless).
        // SAFETY: no pointer arguments.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "aucune langue de Windows n'a la reconnaissance de texte".to_string())?;
        let lang = engine
            .RecognizerLanguage()
            .and_then(|l| l.LanguageTag())
            .map(|t| t.to_string_lossy())
            .unwrap_or_default();
        Ok(Ocr { engine, lang })
    }

    /// The text lines read in `frame`.
    pub fn read(&self, frame: &Frame) -> Result<Vec<String>, String> {
        use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
        use windows::Security::Cryptography::CryptographicBuffer;
        let err = |e: windows::core::Error| format!("reconnaissance de texte : {}", e.message());
        let buf = CryptographicBuffer::CreateFromByteArray(&frame.bgra).map_err(err)?;
        let bmp = SoftwareBitmap::CreateCopyFromBuffer(
            &buf,
            BitmapPixelFormat::Bgra8,
            frame.width as i32,
            frame.height as i32,
        )
        .map_err(err)?;
        let result = self
            .engine
            .RecognizeAsync(&bmp)
            .map_err(err)?
            .join()
            .map_err(err)?;
        let mut lines = Vec::new();
        for line in result.Lines().map_err(err)? {
            lines.push(line.Text().map_err(err)?.to_string_lossy());
        }
        Ok(lines)
    }
}

#[cfg(not(windows))]
pub struct Ocr {
    pub lang: String,
}

#[cfg(not(windows))]
impl Ocr {
    pub fn new() -> Result<Self, String> {
        Err("reconnaissance de texte : Windows uniquement".into())
    }

    pub fn read(&self, _frame: &Frame) -> Result<Vec<String>, String> {
        Err("reconnaissance de texte : Windows uniquement".into())
    }
}
