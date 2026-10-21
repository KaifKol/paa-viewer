use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::LazyLock;

static IPC_CHANNEL: LazyLock<(Sender<PathBuf>, std::sync::Mutex<Receiver<PathBuf>>)> = LazyLock::new(|| {
    let (tx, rx) = channel();
    (tx, std::sync::Mutex::new(rx))
});

#[cfg(unix)]
fn socket_path() -> PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime).join("paa-viewer.sock")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        std::env::temp_dir().join(format!("paa-viewer-{user}.sock"))
    }
}

/// Try to forward the file path to an already running instance.
/// Returns true if an existing instance was found and the file was forwarded (caller should exit).
/// Returns false if this is the primary instance and should proceed to open the GUI.
pub fn try_forward_to_existing(file: Option<&Path>) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;

        let sock = socket_path();
        if let Ok(mut stream) = UnixStream::connect(&sock) {
            let msg = if let Some(p) = file {
                p.to_string_lossy().to_string()
            } else {
                "ACTIVATE".to_string()
            };
            if writeln!(stream, "{msg}").is_ok() {
                return true;
            }
        }
    }

    false
}

/// Start listening for file open requests from secondary instances.
pub fn start_listener() {
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixListener;

        let sock = socket_path();
        let _ = std::fs::remove_file(&sock);

        if let Ok(listener) = UnixListener::bind(&sock) {
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && trimmed != "ACTIVATE" {
                            let _ = IPC_CHANNEL.0.send(PathBuf::from(trimmed));
                        }
                    }
                }
            });
        }
    }
}

/// Check for any new files sent from secondary instances.
pub fn check_incoming_file() -> Option<PathBuf> {
    if let Ok(rx) = IPC_CHANNEL.1.lock() {
        let mut latest = None;
        while let Ok(path) = rx.try_recv() {
            latest = Some(path);
        }
        return latest;
    }
    None
}

/// Cleanup socket on exit
pub fn cleanup() {
    #[cfg(unix)]
    {
        let sock = socket_path();
        let _ = std::fs::remove_file(&sock);
    }
}
