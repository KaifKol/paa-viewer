use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ContextMenuManager;

impl ContextMenuManager {
    pub fn is_installed() -> bool {
        #[cfg(target_os = "windows")]
        {
            Self::is_installed_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::is_installed_linux()
        }
    }

    pub fn config_dir() -> PathBuf {
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(config_home).join("paa-viewer")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config").join("paa-viewer")
        } else if let Ok(appdata) = std::env::var("APPDATA") {
            PathBuf::from(appdata).join("paa-viewer")
        } else {
            std::env::temp_dir().join("paa-viewer-cfg")
        }
    }

    pub fn disabled_marker_path() -> PathBuf {
        Self::config_dir().join("disabled-auto-register")
    }

    pub fn resolve_executable_path() -> Result<PathBuf, String> {
        // 1. If running inside an AppImage, the real permanent executable is the .AppImage file itself
        if let Ok(appimage) = std::env::var("APPIMAGE") {
            if !appimage.is_empty() {
                let p = PathBuf::from(appimage);
                if p.exists() {
                    return Ok(p);
                }
            }
        }

        if let Ok(argv0) = std::env::var("ARGV0") {
            if !argv0.is_empty() {
                let p = PathBuf::from(argv0);
                if p.exists() && !p.to_string_lossy().contains("/tmp/.mount_") {
                    return Ok(p);
                }
            }
        }

        let exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {e}"))?;

        // 2. If current_exe points into a temporary AppImage mount (/tmp/.mount_*), find the real AppImage
        let exe_str = exe.to_string_lossy();
        if exe_str.contains("/tmp/.mount_") {
            if let Ok(owd) = std::env::var("OWD") {
                let owd_path = PathBuf::from(owd);
                if let Ok(entries) = std::fs::read_dir(&owd_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if name.to_lowercase().contains("paa") && name.ends_with(".AppImage") {
                                return Ok(path);
                            }
                        }
                    }
                }
            }
        }

        Ok(exe)
    }

    pub fn auto_register_if_needed() {
        if Self::disabled_marker_path().exists() {
            return;
        }

        let target_exe = match Self::resolve_executable_path() {
            Ok(e) => e,
            Err(_) => return,
        };
        let target_exe_str = target_exe.to_string_lossy();

        let needs_install = if !Self::is_installed() {
            true
        } else {
            #[cfg(not(target_os = "windows"))]
            {
                let desktop_file = Self::linux_user_data_dir().join("applications").join("paa-viewer.desktop");
                if let Ok(content) = std::fs::read_to_string(&desktop_file) {
                    // Re-register if desktop file has stale /tmp/.mount_ or points to different exe
                    content.contains("/tmp/.mount_") || !content.contains(&*target_exe_str)
                } else {
                    true
                }
            }
            #[cfg(target_os = "windows")]
            {
                false
            }
        };

        if needs_install {
            let _ = Self::install();
        }
    }

    pub fn install() -> Result<String, String> {
        let exe = Self::resolve_executable_path()?;

        let _ = std::fs::remove_file(Self::disabled_marker_path());

        #[cfg(target_os = "windows")]
        {
            Self::install_windows(&exe)
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::install_linux(&exe)
        }
    }

    pub fn uninstall() -> Result<String, String> {
        let cfg = Self::config_dir();
        let _ = std::fs::create_dir_all(&cfg);
        let _ = std::fs::write(Self::disabled_marker_path(), "disabled by user request");

        #[cfg(target_os = "windows")]
        {
            Self::uninstall_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::uninstall_linux()
        }
    }

    /// Display a native system notification on Linux or Windows
    pub fn notify(title: &str, body: &str, is_error: bool) {
        #[cfg(target_os = "windows")]
        {
            let script = format!(
                "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
                $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
                $x = [xml]$t.GetXml(); \
                $x.GetElementsByTagName('text')[0].AppendChild($x.CreateTextNode('{}')) > $null; \
                $x.GetElementsByTagName('text')[1].AppendChild($x.CreateTextNode('{}')) > $null; \
                $n = [Windows.UI.Notifications.ToastNotification]::new($t); \
                [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('PAA Viewer').Show($n);",
                title.replace('\'', "''"),
                body.replace('\'', "''")
            );
            let _ = Command::new("powershell")
                .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
                .output();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut cmd = Command::new("notify-send");
            cmd.arg("-a").arg("PAA Viewer");
            if is_error {
                cmd.arg("-u").arg("critical").arg("-i").arg("dialog-error");
            } else {
                cmd.arg("-i").arg("paa-viewer");
            }
            cmd.arg(title).arg(body);
            let _ = cmd.output();
        }
    }

    // =========================================================================
    // Linux Implementation
    // =========================================================================

    fn linux_user_data_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(dir)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local").join("share")
        } else {
            PathBuf::from("/tmp/.local-share")
        }
    }

    pub fn is_installed_linux() -> bool {
        let data_dir = Self::linux_user_data_dir();
        let desktop_file = data_dir.join("applications").join("paa-viewer.desktop");
        let mime_file = data_dir.join("mime").join("packages").join("paa-viewer.xml");
        desktop_file.exists() && mime_file.exists()
    }

    pub fn install_linux(exe_path: &Path) -> Result<String, String> {
        let data_dir = Self::linux_user_data_dir();
        let app_dir = data_dir.join("applications");
        let mime_dir = data_dir.join("mime").join("packages");
        let kio_dir = data_dir.join("kio").join("servicemenus");
        let kservices_dir = data_dir.join("kservices5").join("ServiceMenus");
        let nautilus_scripts_dir = data_dir.join("nautilus").join("scripts");
        let nemo_actions_dir = data_dir.join("nemo").join("actions");
        let icons_svg_dir = data_dir.join("icons").join("hicolor").join("scalable").join("apps");
        let icons_png_dir = data_dir.join("icons").join("hicolor").join("256x256").join("apps");

        std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&mime_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&kio_dir).ok();
        std::fs::create_dir_all(&kservices_dir).ok();
        std::fs::create_dir_all(&nautilus_scripts_dir).ok();
        std::fs::create_dir_all(&nemo_actions_dir).ok();
        std::fs::create_dir_all(&icons_svg_dir).ok();
        std::fs::create_dir_all(&icons_png_dir).ok();

        let exe_str = exe_path.to_string_lossy();

        // 1. Install MIME XML for *.paa and *.pac
        let mime_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="image/x-paa">
    <comment>Bohemia Interactive PAA Texture</comment>
    <glob pattern="*.paa"/>
    <glob pattern="*.pac"/>
    <icon name="paa-viewer"/>
  </mime-type>
</mime-info>
"#;
        std::fs::write(mime_dir.join("paa-viewer.xml"), mime_xml)
            .map_err(|e| format!("Failed to write MIME XML: {e}"))?;

        // 2. Install Desktop Entry (for launcher, associations, and FreeDesktop actions)
        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Name=PAA Viewer & Converter
GenericName=Texture Viewer
Comment=Bohemia Interactive PAA texture viewer, converter, and POT resizer
Exec="{exe_str}" %f
Icon=paa-viewer
Terminal=false
Categories=Graphics;2DGraphics;RasterGraphics;Viewer;
MimeType=image/x-paa;image/png;image/jpeg;image/x-tga;image/webp;image/bmp;
StartupNotify=true
Actions=ConvertToPng;ConvertToPaaDxt5;ConvertToPaaDxt1;

[Desktop Action ConvertToPng]
Name=Convert to PNG
Icon=image-png
Exec="{exe_str}" convert %f --notify

[Desktop Action ConvertToPaaDxt5]
Name=Convert to PAA (DXT5 POT)
Icon=image-x-generic
Exec="{exe_str}" convert %f --format dxt5 --resize pot-nearest --notify

[Desktop Action ConvertToPaaDxt1]
Name=Convert to PAA (DXT1 POT)
Icon=image-x-generic
Exec="{exe_str}" convert %f --format dxt1 --resize pot-nearest --notify
"#
        );
        std::fs::write(app_dir.join("paa-viewer.desktop"), desktop_content)
            .map_err(|e| format!("Failed to write desktop file: {e}"))?;

        // 3. Install KDE Dolphin / KIO Service Menus
        let kio_paa_content = format!(
            r#"[Desktop Entry]
Type=Service
ServiceTypes=KonqPopupMenu/Plugin
MimeType=image/x-paa;
Actions=ConvertPng;
X-KDE-Priority=TopLevel

[Desktop Action ConvertPng]
Name=Convert to PNG (via PAA Viewer)
Icon=image-png
Exec="{exe_str}" convert %f --notify
"#
        );
        let _ = std::fs::write(kio_dir.join("paa-convert-png.desktop"), &kio_paa_content);
        let _ = std::fs::write(kservices_dir.join("paa-convert-png.desktop"), &kio_paa_content);

        let kio_img_content = format!(
            r#"[Desktop Entry]
Type=Service
ServiceTypes=KonqPopupMenu/Plugin
MimeType=image/png;image/jpeg;image/x-tga;image/webp;image/bmp;
Actions=ConvertPaaDxt5;ConvertPaaDxt1;
X-KDE-Priority=TopLevel

[Desktop Action ConvertPaaDxt5]
Name=Convert to PAA (DXT5 POT)
Icon=image-x-generic
Exec="{exe_str}" convert %f --format dxt5 --resize pot-nearest --notify

[Desktop Action ConvertPaaDxt1]
Name=Convert to PAA (DXT1 POT)
Icon=image-x-generic
Exec="{exe_str}" convert %f --format dxt1 --resize pot-nearest --notify
"#
        );
        let _ = std::fs::write(kio_dir.join("paa-convert-paa.desktop"), &kio_img_content);
        let _ = std::fs::write(kservices_dir.join("paa-convert-paa.desktop"), &kio_img_content);

        // 4. Install GNOME Files / Nautilus Scripts
        let nautilus_script_png = format!(
            r#"#!/bin/bash
while IFS= read -r file; do
    [ -n "$file" ] && [ -f "$file" ] && "{exe_str}" convert "$file" --notify
done <<< "$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS"
"#
        );
        let script_png_path = nautilus_scripts_dir.join("Convert to PNG (PAA Viewer)");
        let _ = std::fs::write(&script_png_path, nautilus_script_png);
        Self::make_executable(&script_png_path);

        let nautilus_script_paa = format!(
            r#"#!/bin/bash
while IFS= read -r file; do
    [ -n "$file" ] && [ -f "$file" ] && "{exe_str}" convert "$file" --format dxt5 --resize pot-nearest --notify
done <<< "$NAUTILUS_SCRIPT_SELECTED_FILE_PATHS"
"#
        );
        let script_paa_path = nautilus_scripts_dir.join("Convert to PAA (DXT5 POT)");
        let _ = std::fs::write(&script_paa_path, nautilus_script_paa);
        Self::make_executable(&script_paa_path);

        // 5. Install Nemo Actions (Cinnamon / Mint)
        let nemo_paa_content = format!(
            r#"[Nemo Action]
Name=Convert to PNG
Comment=Convert PAA texture to PNG image via PAA Viewer
Exec="{exe_str}" convert %f --notify
Icon-Name=image-png
Selection=s
Extensions=paa;pac;
Quote=double
"#
        );
        let _ = std::fs::write(nemo_actions_dir.join("paa-to-png.nemo_action"), nemo_paa_content);

        let nemo_img_content = format!(
            r#"[Nemo Action]
Name=Convert to PAA (DXT5 POT)
Comment=Convert image to PAA DXT5 with POT resizing
Exec="{exe_str}" convert %f --format dxt5 --resize pot-nearest --notify
Icon-Name=image-x-generic
Selection=s
Extensions=png;jpg;jpeg;tga;bmp;webp;
Quote=double
"#
        );
        let _ = std::fs::write(nemo_actions_dir.join("image-to-paa.nemo_action"), nemo_img_content);

        // 6. Install Application Icons
        let svg_icon = include_str!("../packaging/paa-viewer.svg");
        let _ = std::fs::write(icons_svg_dir.join("paa-viewer.svg"), svg_icon);

        let png_icon = include_bytes!("../packaging/paa-viewer.png");
        let _ = std::fs::write(icons_png_dir.join("paa-viewer.png"), png_icon);

        // 7. Update system MIME and desktop databases
        let _ = Command::new("update-mime-database")
            .arg(data_dir.join("mime"))
            .output();

        let _ = Command::new("update-desktop-database")
            .arg(&app_dir)
            .output();

        // 8. Set as default application for double-click opening of PAA textures
        let _ = Command::new("xdg-mime")
            .args(["default", "paa-viewer.desktop", "image/x-paa"])
            .output();

        Self::update_mimeapps_list("paa-viewer.desktop", "image/x-paa", true);

        Ok(format!(
            "Registered successfully:\n\
             - Double click: default handler for image/x-paa (*.paa, *.pac)\n\
             - Desktop file: {}\n\
             - MIME database: {}\n\
             - Dolphin context menus: {}\n\
             - Nautilus scripts: {}\n\
             - Nemo actions: {}",
            app_dir.join("paa-viewer.desktop").display(),
            mime_dir.join("paa-viewer.xml").display(),
            kio_dir.join("paa-convert-png.desktop").display(),
            nautilus_scripts_dir.display(),
            nemo_actions_dir.display()
        ))
    }

    pub fn uninstall_linux() -> Result<String, String> {
        let data_dir = Self::linux_user_data_dir();
        let app_dir = data_dir.join("applications");
        let mime_dir = data_dir.join("mime").join("packages");
        let kio_dir = data_dir.join("kio").join("servicemenus");
        let kservices_dir = data_dir.join("kservices5").join("ServiceMenus");
        let nautilus_scripts_dir = data_dir.join("nautilus").join("scripts");
        let nemo_actions_dir = data_dir.join("nemo").join("actions");

        let _ = std::fs::remove_file(app_dir.join("paa-viewer.desktop"));
        let _ = std::fs::remove_file(mime_dir.join("paa-viewer.xml"));
        let _ = std::fs::remove_file(kio_dir.join("paa-convert-png.desktop"));
        let _ = std::fs::remove_file(kio_dir.join("paa-convert-paa.desktop"));
        let _ = std::fs::remove_file(kio_dir.join("paa-actions.desktop"));
        let _ = std::fs::remove_file(kservices_dir.join("paa-convert-png.desktop"));
        let _ = std::fs::remove_file(kservices_dir.join("paa-convert-paa.desktop"));
        let _ = std::fs::remove_file(kservices_dir.join("paa-actions.desktop"));
        let _ = std::fs::remove_file(nautilus_scripts_dir.join("Convert to PNG (PAA Viewer)"));
        let _ = std::fs::remove_file(nautilus_scripts_dir.join("Convert to PAA (DXT5 POT)"));
        let _ = std::fs::remove_file(nemo_actions_dir.join("paa-to-png.nemo_action"));
        let _ = std::fs::remove_file(nemo_actions_dir.join("image-to-paa.nemo_action"));

        Self::update_mimeapps_list("paa-viewer.desktop", "image/x-paa", false);

        let _ = Command::new("update-mime-database")
            .arg(data_dir.join("mime"))
            .output();

        let _ = Command::new("update-desktop-database")
            .arg(&app_dir)
            .output();

        Ok("Unregistered PAA Viewer desktop entries, associations, and context menus successfully".into())
    }

    fn make_executable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
        #[cfg(not(unix))]
        {
            let _ = path;
        }
    }

    fn update_mimeapps_list(desktop_id: &str, mime_type: &str, add: bool) {
        let mut paths = Vec::new();

        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            paths.push(PathBuf::from(config_home).join("mimeapps.list"));
        } else if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join(".config").join("mimeapps.list"));
        }

        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            paths.push(PathBuf::from(data_home).join("applications").join("mimeapps.list"));
        } else if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(&home).join(".local").join("share").join("applications").join("mimeapps.list"));
        }

        for path in paths {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let updated = Self::apply_mimeapps_patch(&content, desktop_id, mime_type, add);
            let _ = std::fs::write(&path, updated);
        }
    }

    fn apply_mimeapps_patch(content: &str, desktop_id: &str, mime_type: &str, add: bool) -> String {
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let sections = ["[Default Applications]", "[Added Associations]"];

        for section in sections {
            let mut section_idx = None;
            for (i, line) in lines.iter().enumerate() {
                if line.trim() == section {
                    section_idx = Some(i);
                    break;
                }
            }

            if add {
                let entry = if section == "[Default Applications]" {
                    format!("{mime_type}={desktop_id}")
                } else {
                    format!("{mime_type}={desktop_id};")
                };

                if let Some(idx) = section_idx {
                    let mut found = false;
                    let mut next_sec = lines.len();
                    for (j, line) in lines.iter().enumerate().skip(idx + 1) {
                        if line.trim().starts_with('[') {
                            next_sec = j;
                            break;
                        }
                        if line.trim().starts_with(mime_type) {
                            lines[j] = entry.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        lines.insert(next_sec, entry);
                    }
                } else {
                    lines.push(String::new());
                    lines.push(section.to_string());
                    lines.push(entry);
                }
            } else if let Some(idx) = section_idx {
                let mut remove_idx = None;
                for (j, line) in lines.iter().enumerate().skip(idx + 1) {
                    if line.trim().starts_with('[') {
                        break;
                    }
                    if line.trim().starts_with(mime_type) && line.contains(desktop_id) {
                        remove_idx = Some(j);
                        break;
                    }
                }
                if let Some(ri) = remove_idx {
                    lines.remove(ri);
                }
            }
        }

        lines.join("\n") + "\n"
    }

    // =========================================================================
    // Windows Implementation
    // =========================================================================

    pub fn is_installed_windows() -> bool {
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("reg")
                .args(["query", r"HKCU\Software\Classes\.paa"])
                .output();
            if let Ok(out) = output {
                return out.status.success();
            }
        }
        false
    }

    pub fn generate_windows_reg(exe_path: &Path) -> String {
        let exe_str = exe_path.to_string_lossy().replace('\\', "\\\\");

        format!(
            r#"Windows Registry Editor Version 5.00

; File Association for .paa and .pac (Double click opens with PAA Viewer)
[HKEY_CURRENT_USER\Software\Classes\.paa]
@="PaaViewer.Texture"

[HKEY_CURRENT_USER\Software\Classes\.paa\OpenWithProgids]
"PaaViewer.Texture"=""

[HKEY_CURRENT_USER\Software\Classes\.pac]
@="PaaViewer.Texture"

[HKEY_CURRENT_USER\Software\Classes\.pac\OpenWithProgids]
"PaaViewer.Texture"=""

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture]
@="Bohemia Interactive PAA Texture"
"FriendlyTypeName"="Bohemia Interactive PAA Texture"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\DefaultIcon]
@="{exe_str},0"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\shell]
@="open"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\shell\open]
@="Open with PAA Viewer"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\shell\open\command]
@="\"{exe_str}\" \"%1\""

; Context Menu for .paa Files: Open and Convert to PNG
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\OpenWithPaaViewer]
@="Open with PAA Viewer"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\OpenWithPaaViewer\command]
@="\"{exe_str}\" \"%1\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\ConvertToPng]
@="Convert to PNG (via PAA Viewer)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\ConvertToPng\command]
@="\"{exe_str}\" convert \"%1\" --notify"

; Context Menu for .pac Files
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.pac\Shell\OpenWithPaaViewer]
@="Open with PAA Viewer"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.pac\Shell\OpenWithPaaViewer\command]
@="\"{exe_str}\" \"%1\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.pac\Shell\ConvertToPng]
@="Convert to PNG (via PAA Viewer)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.pac\Shell\ConvertToPng\command]
@="\"{exe_str}\" convert \"%1\" --notify"

; Context Menu for standard images: Convert to PAA (DXT5 & DXT1)
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt1]
@="Convert to PAA (DXT1 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt1\command]
@="\"{exe_str}\" convert \"%1\" --format dxt1 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpeg\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpeg\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.bmp\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.bmp\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.webp\Shell\ConvertToPaaDxt5]
@="Convert to PAA (DXT5 POT)"
"Icon"="{exe_str}"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.webp\Shell\ConvertToPaaDxt5\command]
@="\"{exe_str}\" convert \"%1\" --format dxt5 --resize pot-nearest --notify"
"#
        )
    }

    pub fn install_windows(exe_path: &Path) -> Result<String, String> {
        let reg_content = Self::generate_windows_reg(exe_path);
        let temp_reg = std::env::temp_dir().join("paa-viewer-install.reg");

        // Write UTF-16LE with BOM so reg.exe never fails on non-ASCII paths
        let mut u16_bytes = vec![0xFF, 0xFE];
        for c in reg_content.encode_utf16() {
            u16_bytes.extend_from_slice(&c.to_le_bytes());
        }

        std::fs::write(&temp_reg, &u16_bytes)
            .map_err(|e| format!("Failed to write temporary registry file: {e}"))?;

        let status = Command::new("reg")
            .args(["import", &temp_reg.to_string_lossy()])
            .status()
            .map_err(|e| format!("Failed to run reg.exe import: {e}"))?;

        let _ = std::fs::remove_file(&temp_reg);

        if status.success() {
            Ok("Registered Windows file associations and context menu actions successfully".into())
        } else {
            Err("reg.exe returned non-zero exit status during import".into())
        }
    }

    pub fn uninstall_windows() -> Result<String, String> {
        let keys_to_delete = [
            r"HKCU\Software\Classes\.paa",
            r"HKCU\Software\Classes\.pac",
            r"HKCU\Software\Classes\PaaViewer.Texture",
            r"HKCU\Software\Classes\SystemFileAssociations\.paa\Shell\OpenWithPaaViewer",
            r"HKCU\Software\Classes\SystemFileAssociations\.paa\Shell\ConvertToPng",
            r"HKCU\Software\Classes\SystemFileAssociations\.pac\Shell\OpenWithPaaViewer",
            r"HKCU\Software\Classes\SystemFileAssociations\.pac\Shell\ConvertToPng",
            r"HKCU\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt5",
            r"HKCU\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaaDxt1",
            r"HKCU\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaaDxt5",
            r"HKCU\Software\Classes\SystemFileAssociations\.jpeg\Shell\ConvertToPaaDxt5",
            r"HKCU\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaaDxt5",
            r"HKCU\Software\Classes\SystemFileAssociations\.bmp\Shell\ConvertToPaaDxt5",
            r"HKCU\Software\Classes\SystemFileAssociations\.webp\Shell\ConvertToPaaDxt5",
        ];

        for key in &keys_to_delete {
            let _ = Command::new("reg")
                .args(["delete", key, "/f"])
                .status();
        }

        Ok("Removed Windows file associations and context menu actions successfully".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mimeapps_patch() {
        let initial = "[Default Applications]\nimage/png=org.gnome.Loupe.desktop\n\n[Added Associations]\nimage/png=org.gnome.Loupe.desktop;\n";
        let patched = ContextMenuManager::apply_mimeapps_patch(initial, "paa-viewer.desktop", "image/x-paa", true);
        assert!(patched.contains("image/x-paa=paa-viewer.desktop"));
        assert!(patched.contains("image/x-paa=paa-viewer.desktop;"));

        let unpatched = ContextMenuManager::apply_mimeapps_patch(&patched, "paa-viewer.desktop", "image/x-paa", false);
        assert!(!unpatched.contains("image/x-paa=paa-viewer.desktop"));
    }

    #[test]
    fn test_windows_reg_generation() {
        let reg = ContextMenuManager::generate_windows_reg(Path::new(r"C:\Tools\paa-viewer.exe"));
        assert!(reg.contains("PaaViewer.Texture"));
        assert!(reg.contains(r"C:\\Tools\\paa-viewer.exe"));
        assert!(reg.contains("ConvertToPng"));
        assert!(reg.contains("ConvertToPaaDxt5"));
        assert!(reg.contains(".pac"));
        assert!(reg.contains(".webp"));
    }
}
