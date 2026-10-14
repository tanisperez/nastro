//! Whether something is playing.

/// The transport state of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    /// Nothing is playing; the position is back at the start.
    #[default]
    Stopped,
    /// A track is playing.
    Playing,
    /// A track is loaded and paused at its current position.
    Paused,
}

impl PlayState {
    /// Whether audio is running, so the visualizer, the clock and the marquee animate.
    pub fn is_playing(self) -> bool {
        self == PlayState::Playing
    }
}
