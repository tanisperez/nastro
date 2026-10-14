//! Skin errors.

use thiserror::Error;

/// Everything that can go wrong while reading a skin.
#[derive(Debug, Error)]
pub enum SkinError {
    /// A skin file or folder could not be read.
    #[error("cannot read skin: {0}")]
    Io(#[from] std::io::Error),
    /// The file is not valid TOML, or a value has the wrong type. Carries the parser message.
    #[error("invalid skin file: {0}")]
    Toml(String),
    /// A color value is not `#rgb`, `#rrggbb` or `#rrggbbaa`.
    #[error("invalid color for `{key}`: `{value}` (use #rgb, #rrggbb or #rrggbbaa)")]
    InvalidColor {
        /// The key, as `section.name`.
        key: String,
        /// The offending text.
        value: String,
    },
    /// The `format` key is not a version this build understands.
    #[error("unsupported skin format {0} (this version reads format 1)")]
    UnsupportedFormat(u32),
    /// `[skin] name` is missing.
    #[error("missing `name` in the [skin] section")]
    MissingName,
    /// A number is outside its allowed range.
    #[error("value out of range for `{key}`")]
    OutOfRange {
        /// The key, as `section.name`.
        key: String,
    },
    /// The skin id is already taken (by a built-in skin or by another user skin).
    #[error("skin id `{0}` is already in use")]
    DuplicateId(String),
}
