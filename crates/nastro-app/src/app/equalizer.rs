//! The equalizer as the user set it: ten bands, the preamp, on or off, the automatic preset and
//! the last preset applied. Every change goes straight to the engine.

use gpui_kit::Context;
use nastro_core::{EQ_BAND_COUNT, EqPreset, EqSettings, eq_preset, preset_for_genre};

use crate::app::Models;

/// The equalizer entity.
pub struct EqModel {
    /// Bands, preamp and on or off.
    pub settings: EqSettings,
    /// Whether a preset is picked from each track's genre when it starts.
    pub auto: bool,
    /// The id of the last preset applied, if the bands still match it.
    pub preset: Option<String>,
}

impl EqModel {
    /// The equalizer of the session.
    pub fn new(settings: EqSettings, auto: bool, preset: Option<String>) -> Self {
        EqModel {
            settings,
            auto,
            preset,
        }
    }

    /// Sets band `index` to `gain_db`.
    pub fn set_band(&mut self, index: usize, gain_db: f32, cx: &mut Context<Self>) {
        if index < EQ_BAND_COUNT && self.settings.bands_db[index] != gain_db {
            self.settings.bands_db[index] = gain_db;
            self.preset = None;
            self.apply(cx);
        }
    }

    /// Sets the preamp to `gain_db`.
    pub fn set_preamp(&mut self, gain_db: f32, cx: &mut Context<Self>) {
        if self.settings.preamp_db != gain_db {
            self.settings.preamp_db = gain_db;
            self.preset = None;
            self.apply(cx);
        }
    }

    /// Turns the equalizer on or off.
    pub fn toggle_enabled(&mut self, cx: &mut Context<Self>) {
        self.settings.enabled = !self.settings.enabled;
        self.apply(cx);
    }

    /// Turns the automatic preset on or off.
    pub fn toggle_auto(&mut self, cx: &mut Context<Self>) {
        self.auto = !self.auto;
        crate::app::persist::schedule(cx);
        cx.notify();
    }

    /// Applies `preset`, keeping on or off as it is.
    pub fn apply_preset(&mut self, preset: &EqPreset, cx: &mut Context<Self>) {
        self.settings = self.settings.with_preset(preset);
        self.preset = Some(preset.id.to_string());
        self.apply(cx);
    }

    /// Applies the preset with id `id`, if it exists.
    pub fn apply_preset_id(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(preset) = eq_preset(id) {
            self.apply_preset(preset, cx);
        }
    }

    /// Picks the preset of `genre` when the automatic preset is on. A track without a genre
    /// leaves the equalizer as it is.
    pub fn track_started(&mut self, genre: Option<&str>, cx: &mut Context<Self>) {
        if self.auto
            && let Some(genre) = genre
        {
            let preset = preset_for_genre(genre);
            log::debug!(
                "automatic equalizer preset {} for genre {genre:?}",
                preset.id
            );
            self.apply_preset(preset, cx);
        }
    }

    /// Sets every band and the preamp to 0 dB.
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.settings.preamp_db = 0.0;
        self.settings.bands_db = [0.0; EQ_BAND_COUNT];
        self.preset = Some("flat".to_string());
        self.apply(cx);
    }

    /// Sends the settings to the engine, schedules a session save and repaints.
    fn apply(&mut self, cx: &mut Context<Self>) {
        if let Some(models) = cx.try_global::<Models>() {
            models.transport.read(cx).player.set_eq(self.settings);
        }
        crate::app::persist::schedule(cx);
        cx.notify();
    }
}
