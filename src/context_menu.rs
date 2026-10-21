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

    pub fn install() -> Result<String, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {e}"))?;

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
        #[cfg(target_os = "windows")]
        {
            Self::uninstall_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self::uninstall_linux()
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
        let icons_dir = data_dir.join("icons").join("hicolor").join("scalable").join("apps");

        std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&mime_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&kio_dir).ok();
        std::fs::create_dir_all(&kservices_dir).ok();
        std::fs::create_dir_all(&icons_dir).ok();

        let exe_str = exe_path.to_string_lossy();

        // 1. Install MIME XML
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

        // 2. Install Desktop Entry
        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Name=PAA Viewer & Converter
GenericName=Texture Viewer
Comment=Bohemia Interactive PAA texture viewer, converter, and POT resizer
Exec="{exe_str}" %F
Icon=paa-viewer
Terminal=false
Categories=Graphics;2DGraphics;RasterGraphics;Viewer;
MimeType=image/x-paa;image/png;image/jpeg;image/x-tga;image/webp;image/bmp;
Actions=ConvertToPng;ConvertToPaa;

[Desktop Action ConvertToPng]
Name=Convert to PNG
Exec="{exe_str}" convert %f

[Desktop Action ConvertToPaa]
Name=Convert to PAA (DXT5 POT)
Exec="{exe_str}" convert %f --format dxt5 --resize pot-nearest
"#
        );
        std::fs::write(app_dir.join("paa-viewer.desktop"), desktop_content)
            .map_err(|e| format!("Failed to write desktop file: {e}"))?;

        // 3. Install KDE Dolphin / KIO Context Menu Action
        let kio_content = format!(
            r#"[Desktop Entry]
Type=Service
ServiceTypes=KonqPopupMenu/Plugin
MimeType=image/x-paa;image/png;image/jpeg;image/x-tga;image/webp;image/bmp;
Actions=OpenPaa;ConvertPng;ConvertPaa;
X-KDE-Priority=TopLevel

[Desktop Action OpenPaa]
Name=Open with PAA Viewer
Icon=paa-viewer
Exec="{exe_str}" %F

[Desktop Action ConvertPng]
Name=Convert to PNG (via PAA Viewer)
Icon=image-png
Exec="{exe_str}" convert "%f" "%f.png"

[Desktop Action ConvertPaa]
Name=Convert to PAA (DXT5 POT)
Icon=image-x-generic
Exec="{exe_str}" convert "%f" "%f.paa" --format dxt5 --resize pot-nearest
"#
        );
        let _ = std::fs::write(kio_dir.join("paa-actions.desktop"), &kio_content);
        let _ = std::fs::write(kservices_dir.join("paa-actions.desktop"), &kio_content);

        // 4. Install Application Icon SVG
        let svg_icon = include_str!("../packaging/paa-viewer.svg");
        let _ = std::fs::write(icons_dir.join("paa-viewer.svg"), svg_icon);

        // 5. Update system databases if tools are present
        let _ = Command::new("update-mime-database")
            .arg(data_dir.join("mime"))
            .output();

        let _ = Command::new("update-desktop-database")
            .arg(&app_dir)
            .output();

        Ok(format!(
            "Registered successfully:\n  - Desktop file: {}\n  - MIME type: {}\n  - Dolphin action: {}",
            app_dir.join("paa-viewer.desktop").display(),
            mime_dir.join("paa-viewer.xml").display(),
            kio_dir.join("paa-actions.desktop").display()
        ))
    }

    pub fn uninstall_linux() -> Result<String, String> {
        let data_dir = Self::linux_user_data_dir();
        let app_file = data_dir.join("applications").join("paa-viewer.desktop");
        let mime_file = data_dir.join("mime").join("packages").join("paa-viewer.xml");
        let kio_file1 = data_dir.join("kio").join("servicemenus").join("paa-actions.desktop");
        let kio_file2 = data_dir.join("kservices5").join("ServiceMenus").join("paa-actions.desktop");
        let icon_file = data_dir.join("icons").join("hicolor").join("scalable").join("apps").join("paa-viewer.svg");

        let _ = std::fs::remove_file(app_file);
        let _ = std::fs::remove_file(mime_file);
        let _ = std::fs::remove_file(kio_file1);
        let _ = std::fs::remove_file(kio_file2);
        let _ = std::fs::remove_file(icon_file);

        let _ = Command::new("update-mime-database")
            .arg(data_dir.join("mime"))
            .output();

        let _ = Command::new("update-desktop-database")
            .arg(data_dir.join("applications"))
            .output();

        Ok("Unregistered PAA Viewer desktop entries and context menus successfully".into())
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

; File Association for .paa
[HKEY_CURRENT_USER\Software\Classes\.paa]
@="PaaViewer.Texture"

[HKEY_CURRENT_USER\Software\Classes\.pac]
@="PaaViewer.Texture"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture]
@="Bohemia Interactive PAA Texture"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\DefaultIcon]
@="\"{exe_str}\",0"

[HKEY_CURRENT_USER\Software\Classes\PaaViewer.Texture\shell\open\command]
@="\"{exe_str}\" \"%1\""

; Context Menu for .paa Files
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\OpenWithPaaViewer]
@="Open with PAA Viewer"
"Icon"="\"{exe_str}\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\OpenWithPaaViewer\command]
@="\"{exe_str}\" \"%1\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\ConvertToPng]
@="Convert to PNG"
"Icon"="\"{exe_str}\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.paa\Shell\ConvertToPng\command]
@="\"{exe_str}\" convert \"%1\" \"%1.png\""

; Context Menu for standard images (Convert to PAA)
[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaa]
@="Convert to PAA (DXT5 POT)"
"Icon"="\"{exe_str}\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaa\command]
@="\"{exe_str}\" convert \"%1\" \"%1.paa\" --format dxt5 --resize pot-nearest"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaa]
@="Convert to PAA (DXT1 POT)"
"Icon"="\"{exe_str}\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaa\command]
@="\"{exe_str}\" convert \"%1\" \"%1.paa\" --format dxt1 --resize pot-nearest"

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaa]
@="Convert to PAA (DXT5 POT)"
"Icon"="\"{exe_str}\""

[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaa\command]
@="\"{exe_str}\" convert \"%1\" \"%1.paa\" --format dxt5 --resize pot-nearest"
"#
        )
    }

    pub fn install_windows(exe_path: &Path) -> Result<String, String> {
        let reg_content = Self::generate_windows_reg(exe_path);
        let temp_reg = std::env::temp_dir().join("paa-viewer-install.reg");

        std::fs::write(&temp_reg, &reg_content)
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
            r"HKCU\Software\Classes\SystemFileAssociations\.png\Shell\ConvertToPaa",
            r"HKCU\Software\Classes\SystemFileAssociations\.jpg\Shell\ConvertToPaa",
            r"HKCU\Software\Classes\SystemFileAssociations\.tga\Shell\ConvertToPaa",
        ];

        for key in &keys_to_delete {
            let _ = Command::new("reg")
                .args(["delete", key, "/f"])
                .status();
        }

        Ok("Removed Windows file associations and context menu actions successfully".into())
    }
}
