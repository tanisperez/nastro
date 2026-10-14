//! Built-in equalizer presets and the automatic choice by genre. The curves are Nastro's own.
//! Names are not stored here: the app translates them from the preset `id`.

use crate::eq::{EQ_BAND_COUNT, EqSettings};

/// A named equalizer curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqPreset {
    /// Stable identifier in snake_case, the key of its translated name and of the settings file.
    pub id: &'static str,
    /// Preamp in dB, lowered on boosting curves so the signal does not clip much.
    pub preamp_db: f32,
    /// Gain of each band in dB, in the order of [`EQ_BANDS_HZ`](crate::EQ_BANDS_HZ).
    pub bands_db: [f32; EQ_BAND_COUNT],
}

const fn preset(id: &'static str, preamp_db: f32, bands_db: [f32; EQ_BAND_COUNT]) -> EqPreset {
    EqPreset {
        id,
        preamp_db,
        bands_db,
    }
}

/// Every preset, in the order the menu lists them. `flat` is first.
pub const EQ_PRESETS: &[EqPreset] = &[
    preset("flat", 0.0, [0.0; EQ_BAND_COUNT]),
    preset(
        "rock",
        -4.0,
        [4.0, 3.0, 2.0, -1.0, -2.0, -1.0, 2.0, 3.0, 4.0, 4.0],
    ),
    preset(
        "pop",
        -3.0,
        [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0],
    ),
    preset(
        "jazz",
        -3.0,
        [3.0, 2.0, 1.0, 2.0, -2.0, -2.0, 0.0, 1.0, 2.0, 3.0],
    ),
    preset(
        "classical",
        -2.0,
        [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, -2.0, -2.0, -3.0],
    ),
    preset(
        "dance",
        -5.0,
        [6.0, 5.0, 3.0, 1.0, 0.0, -1.0, -1.0, 3.0, 4.0, 3.0],
    ),
    preset(
        "electronic",
        -4.0,
        [5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 2.0, 4.0, 5.0],
    ),
    preset(
        "hip_hop",
        -5.0,
        [6.0, 5.0, 3.0, 3.0, -1.0, -1.0, 1.0, 0.0, 2.0, 3.0],
    ),
    preset(
        "acoustic",
        -3.0,
        [3.0, 3.0, 2.0, 1.0, 2.0, 1.0, 2.0, 3.0, 3.0, 2.0],
    ),
    preset(
        "vocal",
        -3.0,
        [-3.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 2.0, 0.0, -2.0],
    ),
    preset(
        "bass_boost",
        -5.0,
        [7.0, 6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    preset(
        "treble_boost",
        -5.0,
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 6.0, 7.0],
    ),
    preset(
        "loudness",
        -4.0,
        [6.0, 4.0, 0.0, -1.0, -2.0, -2.0, 0.0, 2.0, 4.0, 5.0],
    ),
    preset(
        "soft",
        -1.0,
        [1.0, 2.0, 1.0, 0.0, -1.0, -1.0, 0.0, -2.0, -3.0, -4.0],
    ),
    preset(
        "headphones",
        -3.0,
        [3.0, 5.0, 3.0, -1.0, -2.0, 0.0, 2.0, 3.0, 4.0, 5.0],
    ),
    preset(
        "small_speakers",
        -4.0,
        [6.0, 5.0, 4.0, 3.0, 1.0, 0.0, -1.0, -1.0, 0.0, 0.0],
    ),
];

/// The preset with this `id`.
pub fn eq_preset(id: &str) -> Option<&'static EqPreset> {
    EQ_PRESETS.iter().find(|preset| preset.id == id)
}

/// Genre keywords per preset id, checked in this order against the lowercased genre tag.
const GENRE_KEYWORDS: &[(&str, &[&str])] = &[
    ("rock", &["rock", "metal", "punk"]),
    ("pop", &["pop"]),
    ("jazz", &["jazz", "blues", "swing"]),
    (
        "classical",
        &["classical", "baroque", "opera", "orchestral"],
    ),
    ("dance", &["dance", "disco", "house"]),
    (
        "electronic",
        &["electronic", "techno", "trance", "edm", "ambient"],
    ),
    ("hip_hop", &["hip hop", "hip-hop", "rap", "r&b"]),
    ("acoustic", &["folk", "acoustic", "country"]),
    ("vocal", &["spoken", "podcast", "audiobook"]),
];

/// The preset that suits a genre tag, matched by keyword ignoring case. Unknown genres get
/// `flat`.
pub fn preset_for_genre(genre: &str) -> &'static EqPreset {
    let genre = genre.to_lowercase();
    GENRE_KEYWORDS
        .iter()
        .find(|(_, keywords)| keywords.iter().any(|keyword| genre.contains(keyword)))
        .and_then(|(id, _)| eq_preset(id))
        .unwrap_or(&EQ_PRESETS[0])
}

impl EqSettings {
    /// These settings with the curve of `preset`. `enabled` is kept.
    pub fn with_preset(&self, preset: &EqPreset) -> EqSettings {
        EqSettings {
            enabled: self.enabled,
            preamp_db: preset.preamp_db,
            bands_db: preset.bands_db,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::eq::EQ_GAIN_RANGE_DB;

    #[test]
    fn ids_are_in_the_documented_order() {
        let ids: Vec<_> = EQ_PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "flat",
                "rock",
                "pop",
                "jazz",
                "classical",
                "dance",
                "electronic",
                "hip_hop",
                "acoustic",
                "vocal",
                "bass_boost",
                "treble_boost",
                "loudness",
                "soft",
                "headphones",
                "small_speakers",
            ]
        );
    }

    #[test]
    fn gains_are_in_range_and_the_preamp_compensates_boosts() {
        for preset in EQ_PRESETS {
            assert!(
                EQ_GAIN_RANGE_DB.contains(&preset.preamp_db),
                "{}",
                preset.id
            );
            assert!(
                preset.bands_db.iter().all(|g| EQ_GAIN_RANGE_DB.contains(g)),
                "{}",
                preset.id
            );
            let boost = preset.bands_db.iter().copied().fold(0.0_f32, f32::max);
            assert!(preset.preamp_db <= 0.0, "{}", preset.id);
            assert!(
                preset.preamp_db <= -boost / 2.0,
                "{} preamp too high",
                preset.id
            );
        }
        assert!(eq_preset("flat").is_some_and(|flat| flat.bands_db == [0.0; EQ_BAND_COUNT]));
    }

    #[test]
    fn lookup_by_id() {
        assert_eq!(eq_preset("jazz").map(|p| p.id), Some("jazz"));
        assert!(eq_preset("Jazz").is_none());
        assert!(eq_preset("nope").is_none());
    }

    #[test]
    fn genres_map_to_presets() {
        let id = |genre| preset_for_genre(genre).id;
        assert_eq!(id("Hard Rock"), "rock");
        assert_eq!(id("Heavy METAL"), "rock");
        assert_eq!(id("Synth-Pop"), "pop");
        assert_eq!(id("Delta Blues"), "jazz");
        assert_eq!(id("Baroque"), "classical");
        assert_eq!(id("Disco"), "dance");
        assert_eq!(id("Ambient"), "electronic");
        assert_eq!(id("Hip-Hop"), "hip_hop");
        assert_eq!(id("hip hop"), "hip_hop");
        assert_eq!(id("R&B"), "hip_hop");
        assert_eq!(id("Folk"), "acoustic");
        assert_eq!(id("Audiobook"), "vocal");
        assert_eq!(id("Bossa Nova"), "flat");
        assert_eq!(id(""), "flat");
    }

    #[test]
    fn with_preset_keeps_enabled() {
        let eq = EqSettings {
            enabled: false,
            ..EqSettings::default()
        };
        let rock = eq_preset("rock").unwrap();
        let applied = eq.with_preset(rock);
        assert!(!applied.enabled);
        assert_eq!(applied.preamp_db, rock.preamp_db);
        assert_eq!(applied.bands_db, rock.bands_db);
        assert!(
            EqSettings::default()
                .with_preset(eq_preset("flat").unwrap())
                .is_flat()
        );
    }
}
