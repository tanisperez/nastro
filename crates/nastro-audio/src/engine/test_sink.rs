//! An in-memory sink: a thread drains the renderer much faster than real time while the sink
//! is resumed, and records only the real (non-underrun) frames.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::render::Renderer;
use super::sink::{AudioSink, SinkFormat};
use crate::error::AudioError;

#[derive(Default)]
struct Inner {
    running: AtomicBool,
    quit: AtomicBool,
    opened: AtomicBool,
    collected: Mutex<Vec<f32>>,
}

/// The test side of the sink.
#[derive(Clone)]
pub struct Handle(Arc<Inner>);

impl Handle {
    /// Everything consumed so far, interleaved.
    pub fn samples(&self) -> Vec<f32> {
        self.0
            .collected
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Number of samples consumed so far.
    pub fn len(&self) -> usize {
        self.0.collected.lock().map(|v| v.len()).unwrap_or(0)
    }

    /// Whether the device was opened.
    pub fn opened(&self) -> bool {
        self.0.opened.load(Ordering::SeqCst)
    }

    /// Whether the sink was dropped, which happens when the engine thread ends.
    pub fn closed(&self) -> bool {
        self.0.quit.load(Ordering::SeqCst)
    }

    /// Whether the device is being driven.
    pub fn running(&self) -> bool {
        self.0.running.load(Ordering::SeqCst)
    }
}

/// The engine side of the sink.
pub struct TestSink {
    format: SinkFormat,
    inner: Arc<Inner>,
}

impl TestSink {
    /// A sink with the given device format and the handle to look at it.
    pub fn new(sample_rate: u32, channels: u16) -> (TestSink, Handle) {
        let inner = Arc::new(Inner::default());
        let sink = TestSink {
            format: SinkFormat {
                sample_rate,
                channels,
            },
            inner: inner.clone(),
        };
        (sink, Handle(inner))
    }
}

impl Drop for TestSink {
    fn drop(&mut self) {
        self.inner.quit.store(true, Ordering::SeqCst);
    }
}

impl AudioSink for TestSink {
    fn format(&mut self) -> Result<SinkFormat, AudioError> {
        Ok(self.format)
    }

    fn open(&mut self, mut renderer: Renderer) -> Result<(), AudioError> {
        self.inner.opened.store(true, Ordering::SeqCst);
        let inner = self.inner.clone();
        let channels = usize::from(self.format.channels);
        std::thread::spawn(move || {
            let mut buffer = vec![0.0f32; 512 * channels];
            while !inner.quit.load(Ordering::SeqCst) {
                if !inner.running.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                let frames = renderer.render(&mut buffer);
                if frames > 0
                    && let Ok(mut collected) = inner.collected.lock()
                {
                    collected.extend_from_slice(&buffer[..frames * channels]);
                }
                std::thread::sleep(Duration::from_micros(if frames == 0 { 1000 } else { 400 }));
            }
        });
        Ok(())
    }

    fn resume(&mut self) -> Result<(), AudioError> {
        self.inner.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn pause(&mut self) -> Result<(), AudioError> {
        self.inner.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}
