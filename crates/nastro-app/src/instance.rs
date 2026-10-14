//! One Nastro at a time on Linux and macOS. The first instance listens on a local socket; a
//! later one (a file opened with Nastro from the file manager while it plays) hands its files
//! to the first through the socket and exits, so two players never sound at once. Windows has no
//! Unix sockets in `std`, so there every launch is its own instance for now.
//!
//! The protocol is plain text: one absolute path per line, then the end of the stream. No line
//! at all means "show yourself".

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// The socket file name inside [`socket_path`]'s folder.
const SOCKET_NAME: &str = "nastro.sock";

/// The socket path: in `$XDG_RUNTIME_DIR` on Linux (private to the user and cleared at logout),
/// in the state folder elsewhere.
pub fn socket_path() -> Option<PathBuf> {
    dirs::runtime_dir()
        .or_else(crate::state::session::state_dir)
        .map(|dir| dir.join(SOCKET_NAME))
}

/// What this process is.
pub enum Claim {
    /// The first instance, with the socket to listen on.
    Primary(UnixListener),
    /// Another instance is running and took the files.
    Handed,
    /// The socket could not be used; run as a standalone instance.
    Standalone,
}

/// Becomes the primary instance, or hands `paths` to the running one.
pub fn claim(socket: &Path, paths: &[PathBuf]) -> Claim {
    if let Ok(stream) = UnixStream::connect(socket) {
        return match send(stream, paths) {
            Ok(()) => Claim::Handed,
            Err(error) => {
                log::warn!("could not hand the files to the running Nastro: {error}");
                Claim::Standalone
            }
        };
    }
    // Nobody answers: a socket file left by a crash, or none at all.
    let _ = std::fs::remove_file(socket);
    if let Some(parent) = socket.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match UnixListener::bind(socket) {
        Ok(listener) => Claim::Primary(listener),
        Err(error) => {
            log::warn!("could not listen on {}: {error}", socket.display());
            Claim::Standalone
        }
    }
}

/// Writes `paths`, made absolute, one per line.
fn send(mut stream: UnixStream, paths: &[PathBuf]) -> std::io::Result<()> {
    let current = std::env::current_dir().unwrap_or_default();
    for path in paths {
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            current.join(path)
        };
        writeln!(stream, "{}", absolute.display())?;
    }
    stream.flush()
}

/// Accepts connections forever on a thread of its own, passing each request's paths to
/// `deliver`. The thread sleeps in `accept`, so it costs nothing while idle.
pub fn serve(listener: UnixListener, deliver: impl Fn(Vec<PathBuf>) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("nastro-instance".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => deliver(read_paths(stream)),
                    Err(error) => log::warn!("instance socket: {error}"),
                }
            }
        });
    if let Err(error) = spawned {
        log::warn!("could not start the instance listener: {error}");
    }
}

/// The paths of one request.
fn read_paths(stream: UnixStream) -> Vec<PathBuf> {
    BufReader::new(stream)
        .lines()
        .map_while(Result::ok)
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Removes the socket file when the primary instance quits.
pub fn release(socket: &Path) {
    let _ = std::fs::remove_file(socket);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_second_instance_hands_its_files_to_the_first() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let socket = dir.path().join("test.sock");
        let Claim::Primary(listener) = claim(&socket, &[]) else {
            panic!("the first claim must be primary");
        };
        let (sender, receiver) = mpsc::channel();
        serve(listener, move |paths| {
            let _ = sender.send(paths);
        });
        let files = vec![
            PathBuf::from("/music/a.flac"),
            PathBuf::from("/music/b c.mp3"),
        ];
        assert!(matches!(claim(&socket, &files), Claim::Handed));
        let received = receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(received, files);

        // A launch without files asks the first instance to show itself.
        assert!(matches!(claim(&socket, &[]), Claim::Handed));
        let received = receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(received.is_empty());
    }

    #[test]
    fn a_stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("{error}"));
        let socket = dir.path().join("stale.sock");
        std::fs::write(&socket, b"").unwrap_or_else(|error| panic!("{error}"));
        assert!(matches!(claim(&socket, &[]), Claim::Primary(_)));
    }
}
