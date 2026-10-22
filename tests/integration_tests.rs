use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TestHome {
    root: PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "paa-viewer-integration-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("temporary home should be created");
        Self { root }
    }

    fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(target_os = "linux")]
fn install_in_isolated_home(home: &TestHome) {
    let output = Command::new(env!("CARGO_BIN_EXE_paa-viewer"))
        .arg("install")
        .env("HOME", &home.root)
        .env("XDG_DATA_HOME", home.data_dir())
        .env("XDG_CONFIG_HOME", home.config_dir())
        .env("PATH", "")
        .output()
        .expect("paa-viewer install should start");

    assert!(
        output.status.success(),
        "paa-viewer install failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "linux")]
fn read_installed(home: &TestHome, relative_path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(home.data_dir().join(relative_path))
        .expect("installed integration file should be readable")
}

#[cfg(target_os = "linux")]
#[test]
fn dolphin_service_menu_defines_every_advertised_action() {
    let home = TestHome::new();
    install_in_isolated_home(&home);

    let service_menu = read_installed(&home, "kio/servicemenus/paa-convert-paa.desktop");
    let advertised = service_menu
        .lines()
        .find_map(|line| line.strip_prefix("Actions="))
        .expect("service menu should advertise actions");

    for action in advertised.split(';').filter(|action| !action.is_empty()) {
        assert!(
            service_menu
                .lines()
                .any(|line| line == format!("[Desktop Action {action}]")),
            "advertised action {action} has no matching desktop action section"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn nemo_conversion_actions_accept_exactly_one_input() {
    let home = TestHome::new();
    install_in_isolated_home(&home);

    for relative_path in [
        "nemo/actions/paa-to-png.nemo_action",
        "nemo/actions/image-to-paa.nemo_action",
    ] {
        let action = read_installed(&home, relative_path);
        let selection = action
            .lines()
            .find_map(|line| line.strip_prefix("Selection="));
        let exec = action.lines().find_map(|line| line.strip_prefix("Exec="));

        assert_eq!(
            selection,
            Some("s"),
            "{relative_path} must not accept multiple inputs"
        );
        assert!(
            exec.is_some_and(|command| command.contains("%f") && !command.contains("%F")),
            "{relative_path} must pass only the first selected file"
        );
    }
}

#[test]
fn clean_path_preserves_unmatched_quote_path() {
    let path = PathBuf::from("'");

    assert_eq!(paa_viewer::gui::app::clean_path(path.clone()), path);
}

#[cfg(unix)]
static IPC_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

#[cfg(unix)]
impl EnvVarGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

#[cfg(unix)]
impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(previous) = &self.previous {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn secondary_launch_without_file_delivers_activation_message() {
    use paa_viewer::single_instance::{
        IpcMessage, check_incoming_message, cleanup, start_listener, try_forward_to_existing,
    };
    use std::time::{Duration, Instant};

    let _test_lock = IPC_TEST_LOCK
        .lock()
        .expect("IPC test lock should be available");
    let home = TestHome::new();
    let _runtime_dir = EnvVarGuard::set("XDG_RUNTIME_DIR", &home.root);
    start_listener();

    assert!(try_forward_to_existing(None));

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(message) = check_incoming_message() {
            cleanup();
            assert_eq!(message, IpcMessage::Activate);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "primary instance did not receive activation message"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[test]
fn incoming_ipc_message_wakes_the_gui_context() {
    use paa_viewer::single_instance::{
        IpcMessage, check_incoming_message, cleanup, set_repaint_context, start_listener,
        try_forward_to_existing,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let _test_lock = IPC_TEST_LOCK
        .lock()
        .expect("IPC test lock should be available");
    let home = TestHome::new();
    let _runtime_dir = EnvVarGuard::set("XDG_RUNTIME_DIR", &home.root);
    let repaint_requested = Arc::new(AtomicBool::new(false));
    let repaint_observer = Arc::clone(&repaint_requested);
    let context = eframe::egui::Context::default();
    context.set_request_repaint_callback(move |_| {
        repaint_observer.store(true, Ordering::SeqCst);
    });
    set_repaint_context(context);
    start_listener();

    let path = Path::new("/tmp/forwarded-texture.paa");
    assert!(try_forward_to_existing(Some(path)));

    let deadline = Instant::now() + Duration::from_secs(1);
    while !repaint_requested.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "incoming IPC message did not wake the GUI context"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        check_incoming_message(),
        Some(IpcMessage::Open(path.to_path_buf()))
    );
    cleanup();
}
