//! One-shot text recognition for a selected screen region.
//!
//! The controller runs at most one request at a time. Its worker owns the whole
//! job — PNG encoding, the Tesseract invocation, and the clipboard publication —
//! so the event loop only ever learns a request identity and a privacy-safe
//! outcome. Recognized text never reaches application state, a log line, or a
//! `Debug` rendering.

use std::fmt;

mod controller;
mod tesseract;

pub(crate) use controller::{OcrController, OcrPoll, OcrSubmitError};
pub(crate) use tesseract::{TesseractRecognizer, WlCopyPublisher};

/// Turns encoded image bytes into text. The production implementation shells
/// out to Tesseract; tests substitute deterministic fakes.
pub(crate) trait TextRecognizer {
    fn recognize(
        &self,
        png: &[u8],
        languages: &OcrLanguages,
    ) -> Result<RecognizedOutput, OcrFailure>;
}

/// Publishes recognized text to the system clipboard.
pub(crate) trait OcrTextPublisher {
    fn publish(&self, text: &str) -> Result<(), OcrFailure>;
}

/// Tightly packed premultiplied ARGB32 pixels in Cairo's native byte order.
///
/// Owned by the request: the crop is taken from the displayed screen image on
/// the event thread and handed to the worker, so a later freeze, zoom, or
/// display change cannot change what is recognized.
pub(crate) struct OcrPixels {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stride: i32,
    pub(crate) data: Vec<u8>,
}

impl fmt::Debug for OcrPixels {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OcrPixels")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .field("bytes", &self.data.len())
            .finish()
    }
}

/// A validated Tesseract language argument, such as `eng` or `eng+deu`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OcrLanguages(String);

impl OcrLanguages {
    /// Wrap an already validated value. Config parsing is the only place that
    /// decides what is acceptable; see `config::capture::validate_ocr_languages`.
    pub(crate) fn from_validated(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct OcrRequest {
    pub(crate) pixels: OcrPixels,
    pub(crate) languages: OcrLanguages,
}

/// Recognized text on its way to the clipboard.
///
/// Deliberately opaque: a plain `String` here would eventually reach a log line
/// or an error message and put whatever was on screen into a file. Only the
/// character count and the redacted `Debug` leave this type.
pub(crate) struct RecognizedText(String);

impl RecognizedText {
    /// Trim the recognizer's output. Tesseract always ends with a newline and
    /// pads empty regions with whitespace, so the trimmed value is what decides
    /// between a copy and `No text found`.
    pub(crate) fn trimmed(raw: &str) -> Self {
        Self(raw.trim().to_string())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn character_count(&self) -> usize {
        self.0.chars().count()
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RecognizedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "RecognizedText(<redacted {} chars>)",
            self.character_count()
        )
    }
}

/// What a finished recognition produced. Carries no recognized text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OcrSuccess {
    Copied {
        character_count: usize,
        /// The engine returned bytes that were not valid UTF-8 and unreadable
        /// sequences were replaced before copying.
        replaced_invalid_utf8: bool,
    },
    NoTextFound,
}

/// Why a recognition did not copy anything. Each variant is a distinct
/// user-facing message; engine detail stays in the debug log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OcrFailure {
    /// No `tesseract` executable on `PATH`.
    EngineMissing,
    /// Tesseract could not load the configured language data.
    LanguageMissing { languages: String },
    /// Tesseract exited unsuccessfully for some other reason.
    EngineFailed,
    /// Tesseract did not finish inside the timeout.
    TimedOut,
    /// Recognized output exceeded the stdout cap.
    OutputTooLarge,
    /// The selected pixels could not be encoded as PNG.
    EncodeFailed,
    /// The temporary PNG could not be created or written.
    TemporaryFileFailed,
    /// The process broker was unavailable or rejected the request.
    EngineUnavailable,
    /// Text was recognized but `wl-copy` did not accept it.
    ClipboardFailed,
}

impl OcrFailure {
    /// Stable user-facing message. Never includes engine output.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::EngineMissing => "Install Tesseract to use screen text recognition.".to_string(),
            Self::LanguageMissing { languages } => {
                format!("Tesseract has no language data for \"{languages}\".")
            }
            Self::EngineFailed => "Screen text recognition failed.".to_string(),
            Self::TimedOut => "Screen text recognition timed out.".to_string(),
            Self::OutputTooLarge => "That region produced too much text to copy.".to_string(),
            Self::EncodeFailed => {
                "Could not prepare the selected region for recognition.".to_string()
            }
            Self::TemporaryFileFailed => {
                "Could not create a temporary file for recognition.".to_string()
            }
            Self::EngineUnavailable => "Screen text recognition is unavailable.".to_string(),
            Self::ClipboardFailed => "Text recognized, but clipboard copy failed.".to_string(),
        }
    }
}

pub(crate) type OcrOutcome = Result<OcrSuccess, OcrFailure>;

/// Run one complete request on the worker thread.
///
/// Encoding, recognition, and publication live together so the recognized text
/// is created and consumed inside a single stack frame and is dropped before
/// the outcome travels back to the event loop.
fn run_request(
    request: OcrRequest,
    recognizer: &dyn TextRecognizer,
    publisher: &dyn OcrTextPublisher,
) -> OcrOutcome {
    let png = encode_png(&request.pixels)?;
    let recognized = recognizer.recognize(&png, &request.languages)?;
    if recognized.text.is_empty() {
        return Ok(OcrSuccess::NoTextFound);
    }
    let character_count = recognized.text.character_count();
    publisher.publish(recognized.text.as_str())?;
    Ok(OcrSuccess::Copied {
        character_count,
        replaced_invalid_utf8: recognized.replaced_invalid_utf8,
    })
}

/// A recognizer result: the trimmed text plus whether the engine's bytes had to
/// be repaired to become UTF-8.
pub(crate) struct RecognizedOutput {
    pub(crate) text: RecognizedText,
    pub(crate) replaced_invalid_utf8: bool,
}

fn encode_png(pixels: &OcrPixels) -> Result<Vec<u8>, OcrFailure> {
    if pixels.width == 0 || pixels.height == 0 {
        return Err(OcrFailure::EncodeFailed);
    }
    let width = i32::try_from(pixels.width).map_err(|_| OcrFailure::EncodeFailed)?;
    let height = i32::try_from(pixels.height).map_err(|_| OcrFailure::EncodeFailed)?;
    let surface = cairo::ImageSurface::create_for_data(
        pixels.data.clone(),
        cairo::Format::ARgb32,
        width,
        height,
        pixels.stride,
    )
    .map_err(|err| {
        log::warn!("OCR crop surface creation failed: {err}");
        OcrFailure::EncodeFailed
    })?;
    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).map_err(|err| {
        log::warn!("OCR crop PNG encoding failed: {err}");
        OcrFailure::EncodeFailed
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubRecognizer(fn() -> Result<RecognizedOutput, OcrFailure>);

    impl TextRecognizer for StubRecognizer {
        fn recognize(
            &self,
            _png: &[u8],
            _languages: &OcrLanguages,
        ) -> Result<RecognizedOutput, OcrFailure> {
            (self.0)()
        }
    }

    struct RecordingPublisher(std::sync::Mutex<Vec<String>>);

    impl OcrTextPublisher for RecordingPublisher {
        fn publish(&self, text: &str) -> Result<(), OcrFailure> {
            self.0.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    struct FailingPublisher;

    impl OcrTextPublisher for FailingPublisher {
        fn publish(&self, _text: &str) -> Result<(), OcrFailure> {
            Err(OcrFailure::ClipboardFailed)
        }
    }

    pub(super) fn pixels(width: u32, height: u32) -> OcrPixels {
        OcrPixels {
            width,
            height,
            stride: (width * 4) as i32,
            data: vec![0xFF; (width * height * 4) as usize],
        }
    }

    fn request() -> OcrRequest {
        OcrRequest {
            pixels: pixels(2, 2),
            languages: OcrLanguages::from_validated("eng".to_string()),
        }
    }

    #[test]
    fn recognized_text_is_trimmed_and_never_formatted_into_debug_output() {
        let text = RecognizedText::trimmed("  secret account number \n");
        assert_eq!(text.as_str(), "secret account number");
        assert_eq!(text.character_count(), 21);

        let rendered = format!("{text:?}");
        assert_eq!(rendered, "RecognizedText(<redacted 21 chars>)");
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn pixel_debug_reports_geometry_without_the_captured_bytes() {
        let rendered = format!("{:?}", pixels(3, 4));
        assert!(rendered.contains("width: 3"));
        assert!(rendered.contains("bytes: 48"));
        assert!(!rendered.contains("255"));
    }

    #[test]
    fn empty_recognition_does_not_publish_an_empty_clipboard_value() {
        let publisher = RecordingPublisher(std::sync::Mutex::new(Vec::new()));
        let outcome = run_request(
            request(),
            &StubRecognizer(|| {
                Ok(RecognizedOutput {
                    text: RecognizedText::trimmed("   \n\t "),
                    replaced_invalid_utf8: false,
                })
            }),
            &publisher,
        );

        assert_eq!(outcome, Ok(OcrSuccess::NoTextFound));
        assert!(publisher.0.lock().unwrap().is_empty());
    }

    #[test]
    fn successful_recognition_publishes_text_but_reports_only_a_character_count() {
        let publisher = RecordingPublisher(std::sync::Mutex::new(Vec::new()));
        let outcome = run_request(
            request(),
            &StubRecognizer(|| {
                Ok(RecognizedOutput {
                    text: RecognizedText::trimmed("hello\n"),
                    replaced_invalid_utf8: false,
                })
            }),
            &publisher,
        );

        assert_eq!(
            outcome,
            Ok(OcrSuccess::Copied {
                character_count: 5,
                replaced_invalid_utf8: false,
            })
        );
        assert_eq!(publisher.0.lock().unwrap().as_slice(), ["hello"]);
        assert!(!format!("{outcome:?}").contains("hello"));
    }

    #[test]
    fn clipboard_failure_is_distinct_from_engine_failure() {
        let outcome = run_request(
            request(),
            &StubRecognizer(|| {
                Ok(RecognizedOutput {
                    text: RecognizedText::trimmed("hello"),
                    replaced_invalid_utf8: false,
                })
            }),
            &FailingPublisher,
        );

        assert_eq!(outcome, Err(OcrFailure::ClipboardFailed));
        assert_ne!(
            OcrFailure::ClipboardFailed.message(),
            OcrFailure::EngineFailed.message()
        );
    }

    #[test]
    fn every_failure_category_has_its_own_user_facing_message() {
        let messages = [
            OcrFailure::EngineMissing,
            OcrFailure::LanguageMissing {
                languages: "deu".to_string(),
            },
            OcrFailure::EngineFailed,
            OcrFailure::TimedOut,
            OcrFailure::OutputTooLarge,
            OcrFailure::EncodeFailed,
            OcrFailure::TemporaryFileFailed,
            OcrFailure::EngineUnavailable,
            OcrFailure::ClipboardFailed,
        ]
        .map(|failure| failure.message());
        let unique: std::collections::BTreeSet<_> = messages.iter().collect();

        assert_eq!(unique.len(), messages.len());
        assert!(messages[1].contains("deu"));
    }

    #[test]
    fn encoding_rejects_an_empty_crop_before_touching_cairo() {
        assert_eq!(encode_png(&pixels(0, 4)), Err(OcrFailure::EncodeFailed));
        assert_eq!(encode_png(&pixels(4, 0)), Err(OcrFailure::EncodeFailed));
    }

    #[test]
    fn encoding_preserves_opaque_transparent_and_premultiplied_pixels() {
        let source = OcrPixels {
            width: 3,
            height: 1,
            stride: 12,
            // Cairo ARgb32 is native-endian premultiplied BGRA on little-endian
            // targets: opaque blue, fully transparent, half-alpha grey.
            data: vec![255, 0, 0, 255, 0, 0, 0, 0, 64, 64, 64, 128],
        };

        let png = encode_png(&source).expect("crop encodes");
        let decoded =
            cairo::ImageSurface::create_from_png(&mut png.as_slice()).expect("encoded PNG decodes");
        assert_eq!((decoded.width(), decoded.height()), (3, 1));

        let mut decoded = decoded;
        let stride = decoded.stride() as usize;
        let data = decoded.data().expect("decoded pixels");
        assert_eq!(&data[..12], &source.data[..12]);
        assert!(stride >= 12);
    }
}
