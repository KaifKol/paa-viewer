use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcMessage {
    Activate,
    Open(PathBuf),
}

static IPC_CHANNEL: LazyLock<(Sender<IpcMessage>, std::sync::Mutex<Receiver<IpcMessage>>)> =
    LazyLock::new(|| {
        let (tx, rx) = channel();
        (tx, std::sync::Mutex::new(rx))
    });
static REPAINT_CONTEXT: LazyLock<std::sync::Mutex<Option<eframe::egui::Context>>> =
    LazyLock::new(|| std::sync::Mutex::new(None));

#[cfg(unix)]
fn socket_path() -> PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime).join("paa-viewer.sock")
    } else {
        let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        std::env::temp_dir().join(format!("paa-viewer-{user}.sock"))
    }
}

pub fn set_repaint_context(context: eframe::egui::Context) {
    if let Ok(mut repaint_context) = REPAINT_CONTEXT.lock() {
        *repaint_context = Some(context);
    }
}

fn wake_gui() {
    let context = REPAINT_CONTEXT
        .lock()
        .ok()
        .and_then(|repaint_context| repaint_context.clone());
    if let Some(context) = context {
        context.request_repaint();
    }
}

#[cfg(any(windows, test))]
const PROTOCOL_HANDSHAKE_REQUEST: &str = "PAA_VIEWER_IPC_HELLO_V1";
#[cfg(any(windows, test))]
const PROTOCOL_HANDSHAKE_RESPONSE: &str = "PAA_VIEWER_IPC_OK_V1";
#[cfg(any(windows, test))]
const PROTOCOL_ACK: &str = "PAA_VIEWER_IPC_ACK_V1";
#[cfg(any(windows, test))]
const PROTOCOL_CMD_ACTIVATE: &str = "ACTIVATE";
#[cfg(any(windows, test))]
const PROTOCOL_CMD_OPEN_PREFIX: &str = "OPEN:";

#[cfg(any(windows, test))]
fn port_file_path() -> PathBuf {
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime).join("paa-viewer.port")
    } else {
        let user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "user".to_string());
        std::env::temp_dir().join(format!("paa-viewer-{user}.port"))
    }
}

#[cfg(any(windows, test))]
fn handle_loopback_stream(stream: std::net::TcpStream) {
    let timeout = std::time::Duration::from_secs(1);
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    // 1. Verify handshake request from secondary instance
    if reader.read_line(&mut line).is_err() {
        return;
    }
    if line.trim_end_matches(['\r', '\n']) != PROTOCOL_HANDSHAKE_REQUEST {
        return;
    }

    // 2. Respond to confirm this is paa-viewer
    if reader
        .get_mut()
        .write_all(format!("{PROTOCOL_HANDSHAKE_RESPONSE}\n").as_bytes())
        .is_err()
    {
        return;
    }
    if reader.get_mut().flush().is_err() {
        return;
    }

    // 3. Read command payload
    line.clear();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let payload = line.trim_end_matches(['\r', '\n']);
    let message = if payload == PROTOCOL_CMD_ACTIVATE {
        Some(IpcMessage::Activate)
    } else if let Some(path) = payload.strip_prefix(PROTOCOL_CMD_OPEN_PREFIX) {
        Some(IpcMessage::Open(PathBuf::from(path)))
    } else if !payload.is_empty() {
        Some(IpcMessage::Open(PathBuf::from(payload)))
    } else {
        None
    };

    if let Some(msg) = message {
        if IPC_CHANNEL.0.send(msg).is_ok() {
            wake_gui();
        }
    }

    // 4. Send acknowledgment
    let _ = reader
        .get_mut()
        .write_all(format!("{PROTOCOL_ACK}\n").as_bytes());
    let _ = reader.get_mut().flush();
}

#[cfg(any(windows, test))]
fn loopback_try_forward(port_file: &Path, file: Option<&Path>) -> bool {
    let port_str = match std::fs::read_to_string(port_file) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let port: u16 = match port_str.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = std::fs::remove_file(port_file);
            return false;
        }
    };

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = std::time::Duration::from_secs(1);
    let stream = match std::net::TcpStream::connect_timeout(&addr, timeout) {
        Ok(s) => s,
        Err(_) => {
            let _ = std::fs::remove_file(port_file);
            return false;
        }
    };

    if stream.set_read_timeout(Some(timeout)).is_err()
        || stream.set_write_timeout(Some(timeout)).is_err()
    {
        return false;
    }

    let mut reader = BufReader::new(stream);

    // 1. Send handshake
    if reader
        .get_mut()
        .write_all(format!("{PROTOCOL_HANDSHAKE_REQUEST}\n").as_bytes())
        .is_err()
    {
        return false;
    }
    if reader.get_mut().flush().is_err() {
        return false;
    }

    // 2. Read and verify handshake response
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        let _ = std::fs::remove_file(port_file);
        return false;
    }
    if line.trim_end_matches(['\r', '\n']) != PROTOCOL_HANDSHAKE_RESPONSE {
        let _ = std::fs::remove_file(port_file);
        return false;
    }

    // 3. Send command
    let cmd = if let Some(path) = file {
        format!("{PROTOCOL_CMD_OPEN_PREFIX}{}\n", path.to_string_lossy())
    } else {
        format!("{PROTOCOL_CMD_ACTIVATE}\n")
    };
    if reader.get_mut().write_all(cmd.as_bytes()).is_err() {
        return false;
    }
    if reader.get_mut().flush().is_err() {
        return false;
    }

    // 4. Read acknowledgment
    line.clear();
    if reader.read_line(&mut line).is_err() {
        return false;
    }
    line.trim_end_matches(['\r', '\n']) == PROTOCOL_ACK
}

#[cfg(any(windows, test))]
fn loopback_start_listener(port_file: &Path) {
    let _ = std::fs::remove_file(port_file);

    if let Some(parent) = port_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = match std::net::TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(_) => return,
    };

    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(_) => return,
    };

    if std::fs::write(port_file, format!("{port}\n")).is_err() {
        return;
    }

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                handle_loopback_stream(stream);
            });
        }
    });
}

#[cfg(any(windows, test))]
fn loopback_cleanup(port_file: &Path) {
    let _ = std::fs::remove_file(port_file);
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
    #[cfg(windows)]
    {
        let port_path = port_file_path();
        if loopback_try_forward(&port_path, file) {
            return true;
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
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
                        let message = line.trim_end_matches(['\r', '\n']);
                        let message = match message {
                            "" => None,
                            "ACTIVATE" => Some(IpcMessage::Activate),
                            path => Some(IpcMessage::Open(PathBuf::from(path))),
                        };
                        if let Some(message) = message {
                            if IPC_CHANNEL.0.send(message).is_ok() {
                                wake_gui();
                            }
                        }
                    }
                }
            });
        }
    }
    #[cfg(windows)]
    {
        let port_path = port_file_path();
        loopback_start_listener(&port_path);
    }
}

/// Check for the latest request sent from a secondary instance.
pub fn check_incoming_message() -> Option<IpcMessage> {
    if let Ok(rx) = IPC_CHANNEL.1.lock() {
        let mut latest = None;
        while let Ok(message) = rx.try_recv() {
            latest = Some(message);
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
    #[cfg(windows)]
    {
        let port_path = port_file_path();
        loopback_cleanup(&port_path);
    }
    if let Ok(mut repaint_context) = REPAINT_CONTEXT.lock() {
        *repaint_context = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_loopback_forward_and_activate() {
        let _lock = TEST_MUTEX.lock().unwrap();
        while check_incoming_message().is_some() {}

        let temp_dir = std::env::temp_dir().join(format!(
            "paa-test-loopback-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let port_file = temp_dir.join("test.port");

        loopback_start_listener(&port_file);

        // Forward activation
        assert!(loopback_try_forward(&port_file, None));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(msg) = check_incoming_message() {
                assert_eq!(msg, IpcMessage::Activate);
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Timed out waiting for activate"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Forward open file
        let sample_path = Path::new("/test/texture.paa");
        assert!(loopback_try_forward(&port_file, Some(sample_path)));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(msg) = check_incoming_message() {
                assert_eq!(msg, IpcMessage::Open(sample_path.to_path_buf()));
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Timed out waiting for open"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        loopback_cleanup(&port_file);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_loopback_rejects_unrelated_listener() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let temp_dir = std::env::temp_dir().join(format!(
            "paa-test-unrelated-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let port_file = temp_dir.join("test.port");

        // Bind dummy server that does NOT speak our protocol
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::fs::write(&port_file, format!("{port}\n")).unwrap();

        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::Write;
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nHello");
            }
        });

        // Forward should fail and remove the port file
        assert!(!loopback_try_forward(&port_file, None));
        assert!(!port_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
