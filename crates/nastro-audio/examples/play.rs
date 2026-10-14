//! Plays a file on the default output for a few seconds and prints the status once a second.
//!
//! Usage: `cargo run -p nastro-audio --example play -- <file> [seconds] [pause]`
//!
//! With `pause` as the third argument it also pauses for a second after the first second.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use nastro_audio::{Player, PlayerEvent};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: play <file> [seconds]");
        std::process::exit(2);
    };
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);
    let pause_test = args.next().as_deref() == Some("pause");

    let player = Player::new();
    let events = player.events();
    player.load(path, true);

    let start = Instant::now();
    let mut next_report = Duration::ZERO;
    let mut failed = false;
    let mut paused_at = None;
    while start.elapsed() < Duration::from_secs(seconds) {
        while let Ok(event) = events.try_recv() {
            println!("event: {event:?}");
            match event {
                PlayerEvent::Finished { .. } => return,
                PlayerEvent::Error { .. } => failed = true,
                _ => {}
            }
        }
        if pause_test {
            match paused_at {
                None if start.elapsed() >= Duration::from_secs(1) => {
                    println!("pausing");
                    player.pause();
                    paused_at = Some(Instant::now());
                }
                Some(at) if at.elapsed() >= Duration::from_secs(1) => {
                    println!("resuming");
                    player.play();
                    paused_at = Some(Instant::now() + Duration::from_secs(3600));
                }
                _ => {}
            }
        }
        if failed {
            std::process::exit(1);
        }
        if start.elapsed() >= next_report {
            let status = player.status();
            println!(
                "status: {:?} {:.2}s / {:?}",
                status.state,
                status.position.as_secs_f64(),
                status.duration
            );
            next_report += Duration::from_secs(1);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
