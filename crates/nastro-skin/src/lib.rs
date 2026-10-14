//! Nastro's skin model and loaders.
//!
//! A skin is plain data: colors, fonts and a few numbers read from a `skin.toml` file. This
//! crate has no UI types; the app converts [`Color`] once per skin load. See
//! `docs/skin-format.md` for the file format.

mod builtin;
mod color;
mod contrast;
mod error;
mod parse;
mod skin;
mod user;

pub use builtin::{DEFAULT_SKIN_ID, builtin_skin, builtin_skins, default_skin};
pub use color::{Color, contrast_ratio};
pub use contrast::{ContrastIssue, contrast_issues};
pub use error::SkinError;
pub use parse::{ParsedSkin, parse_skin};
pub use skin::{PlaylistColors, Skin, SkinColors, SkinFonts, SkinOrigin};
pub use user::load_user_skins;
