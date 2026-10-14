//! The error type of the audio pipeline.

/// Everything that can go wrong while probing, decoding, seeking or playing a file.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// The file could not be opened or read.
    #[error("cannot read the file: {0}")]
    Io(#[from] std::io::Error),
    /// The container or the codec is not supported, or the file has no audio track.
    #[error("unsupported format: {0}")]
    Unsupported(String),
    /// The stream is damaged beyond recovery.
    #[error("decode error: {0}")]
    Decode(String),
    /// The output device is missing or refused to start.
    #[error("audio device error: {0}")]
    Device(String),
    /// The requested position cannot be reached.
    #[error("seek error: {0}")]
    Seek(String),
}
