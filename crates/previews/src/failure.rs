// SPDX-License-Identifier: GPL-3.0-or-later

//! Why a photo could not be shown: [`Failure`], in words a photographer
//! can read, made from the errors of the file system, of raw-files and of
//! the JPEG decoder.

use std::error::Error;
use std::fmt;
use std::io;

use image::ImageError;

/// Why a photo could not be shown.
///
/// Every file the app is given may be damaged or hostile, so this is a
/// value, never a panic. Its `Display` is a sentence for the photographer,
/// with no full stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Failure {
    /// The file could not be read: it is gone, it may not be opened, or
    /// the disk failed, as the system said.
    Unreadable(io::ErrorKind),
    /// The file is not a photo the app shows: neither a JPEG nor a RAW file
    /// raw-files reads, or one using a feature they do not read.
    Unsupported,
    /// The file is a RAW file, but holds no preview to show.
    NoPreview,
    /// The file is of a kind the app reads, but its contents contradict
    /// themselves or end too soon.
    Damaged,
    /// Showing the photo would take more memory than the app allows any
    /// one file, which only a damaged or hostile file asks for.
    TooLarge,
}

impl Failure {
    /// The failure for an error of the file system.
    pub(crate) fn from_io(error: &io::Error) -> Failure {
        Failure::Unreadable(error.kind())
    }

    /// The failure for an error of raw-files.
    pub(crate) fn from_raw(error: &raw_files::Error) -> Failure {
        use raw_files::ErrorKind;
        match error.kind() {
            ErrorKind::Io => Failure::Unreadable(
                error
                    .source()
                    .and_then(|source| source.downcast_ref::<io::Error>())
                    .map_or(io::ErrorKind::Other, io::Error::kind),
            ),
            ErrorKind::UnknownFormat | ErrorKind::Unsupported => Failure::Unsupported,
            ErrorKind::Malformed => Failure::Damaged,
            ErrorKind::LimitExceeded => Failure::TooLarge,
            // raw-files may add kinds of error. Not one the app knows, so
            // not a file it can say more of than that it cannot show it.
            _ => Failure::Unsupported,
        }
    }

    /// The failure for an error of the JPEG decoder.
    pub(crate) fn from_image(error: &ImageError) -> Failure {
        match error {
            ImageError::IoError(error) => Failure::from_io(error),
            ImageError::Unsupported(_) => Failure::Unsupported,
            ImageError::Limits(_) => Failure::TooLarge,
            // Decoding only ever reads, so an error of encoding or of a
            // parameter can only come of what the file holds.
            ImageError::Decoding(_) | ImageError::Encoding(_) | ImageError::Parameter(_) => {
                Failure::Damaged
            }
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The system words these two, the ones a photographer meets
            // most (a card taken out, a file another user owns), for
            // programmers ("entity not found"), so they have words of
            // their own; any other is in the system's words.
            Failure::Unreadable(io::ErrorKind::NotFound) => {
                f.write_str("The file is no longer there")
            }
            Failure::Unreadable(io::ErrorKind::PermissionDenied) => {
                f.write_str("You do not have permission to open the file")
            }
            Failure::Unreadable(kind) => write!(f, "The file can't be read ({kind})"),
            Failure::Unsupported => f.write_str("This is not a kind of photo the app can show"),
            Failure::NoPreview => f.write_str("This RAW file holds no preview to show"),
            Failure::Damaged => f.write_str("The file is damaged"),
            Failure::TooLarge => f.write_str("The photo is too large to show"),
        }
    }
}

impl Error for Failure {}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use image::error::{
        DecodingError, EncodingError, LimitError, LimitErrorKind, ParameterError,
        ParameterErrorKind, UnsupportedError, UnsupportedErrorKind,
    };

    /// Every kind of the decoder's errors has its failure.
    #[test]
    fn each_decoder_error_is_a_failure() {
        let jpeg = || ImageFormat::Jpeg.into();
        let cases = [
            (
                ImageError::IoError(io::ErrorKind::UnexpectedEof.into()),
                Failure::Unreadable(io::ErrorKind::UnexpectedEof),
            ),
            (
                ImageError::Unsupported(UnsupportedError::from_format_and_kind(
                    jpeg(),
                    UnsupportedErrorKind::GenericFeature("arithmetic coding".into()),
                )),
                Failure::Unsupported,
            ),
            (
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::DimensionError)),
                Failure::TooLarge,
            ),
            (
                ImageError::Limits(LimitError::from_kind(LimitErrorKind::InsufficientMemory)),
                Failure::TooLarge,
            ),
            (
                ImageError::Decoding(DecodingError::new(jpeg(), "no frame header")),
                Failure::Damaged,
            ),
            (
                ImageError::Encoding(EncodingError::new(jpeg(), "never while decoding")),
                Failure::Damaged,
            ),
            (
                ImageError::Parameter(ParameterError::from_kind(
                    ParameterErrorKind::DimensionMismatch,
                )),
                Failure::Damaged,
            ),
        ];
        for (error, failure) in cases {
            assert_eq!(Failure::from_image(&error), failure, "{error:?}");
        }
    }

    /// A reader's error from raw-files keeps the system's kind of error.
    #[test]
    fn a_raw_files_read_error_keeps_the_system_s_kind() {
        let error = raw_files::Error::from(io::Error::from(io::ErrorKind::TimedOut));
        assert_eq!(
            Failure::from_raw(&error),
            Failure::Unreadable(io::ErrorKind::TimedOut)
        );
    }

    /// Every failure says what happened in words, without a full stop; the
    /// two errors a photographer meets most have words of their own.
    #[test]
    fn a_failure_is_said_for_the_photographer() {
        let said = |failure: Failure| failure.to_string();
        assert_eq!(
            said(Failure::Unreadable(io::ErrorKind::NotFound)),
            "The file is no longer there"
        );
        assert_eq!(
            said(Failure::Unreadable(io::ErrorKind::PermissionDenied)),
            "You do not have permission to open the file"
        );
        assert_eq!(
            said(Failure::Unreadable(io::ErrorKind::TimedOut)),
            "The file can't be read (timed out)"
        );
        for failure in [
            Failure::Unsupported,
            Failure::NoPreview,
            Failure::Damaged,
            Failure::TooLarge,
        ] {
            let text = said(failure);
            assert!(!text.is_empty() && !text.ends_with('.'), "{text}");
        }
    }
}
