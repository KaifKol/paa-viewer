use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use clap::Parser;

use paa_viewer::cli::{build_convert_options, Cli, Commands, ContextMenuAction};
use paa_viewer::context_menu::ContextMenuManager;
use paa_viewer::convert::{batch_convert, convert_image};
use paa_viewer::gui::run_gui;
use paa_viewer::paa::PaaImage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    match args.command {
        None => {
            // Default: run GUI, passing positional file argument if specified
            run_gui(args.file)?;
        }
        Some(Commands::Gui { file }) => {
            run_gui(file)?;
        }
        Some(Commands::Info { file, json }) => {
            let f = File::open(&file)
                .map_err(|e| format!("Failed to open file '{}': {e}", file.display()))?;
            let mut reader = BufReader::new(f);
            let paa = PaaImage::read(&mut reader)
                .map_err(|e| format!("Failed to parse PAA '{}': {e}", file.display()))?;
            let meta = paa.metadata();

            if json {
                let out = serde_json::to_string_pretty(&meta)?;
                println!("{out}");
            } else {
                print_info_table(&file, &meta);
            }
        }
        Some(Commands::Convert {
            input,
            output,
            format,
            resize,
            filter,
            no_mipmaps,
            notify,
        }) => {
            let options = build_convert_options(
                format,
                resize.as_deref(),
                filter,
                no_mipmaps,
            )?;

            let out_path = output.unwrap_or_else(|| {
                let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                let ext = input.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
                let target_ext = if ext == "paa" || ext == "pac" { "png" } else { "paa" };
                input.with_file_name(format!("{stem}.{target_ext}"))
            });

            println!("Converting '{}' -> '{}'...", input.display(), out_path.display());
            let report = match convert_image(&input, &out_path, &options) {
                Ok(r) => r,
                Err(e) => {
                    if notify {
                        let title = "PAA Viewer Error";
                        let msg = format!("Failed to convert '{}':\n{e}", input.display());
                        ContextMenuManager::notify(title, &msg, true);
                    }
                    return Err(e.into());
                }
            };

            println!("Conversion completed successfully!");
            println!("  Format:     {}", report.target_format_name);
            println!("  Dimensions: {}x{} -> {}x{}",
                report.original_dimensions.0, report.original_dimensions.1,
                report.final_dimensions.0, report.final_dimensions.1
            );
            println!("  File Size:  {} bytes -> {} bytes", report.input_size_bytes, report.output_size_bytes);

            if notify {
                let in_name = input.file_name().and_then(|s| s.to_str()).unwrap_or("image");
                let out_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("output");
                let title = "PAA Viewer";
                let msg = format!(
                    "Converted: {in_name} -> {out_name}\n({}x{}, {})",
                    report.final_dimensions.0,
                    report.final_dimensions.1,
                    report.target_format_name
                );
                ContextMenuManager::notify(title, &msg, false);
            }
        }
        Some(Commands::Batch {
            input_dir,
            out_dir,
            to,
            format,
            resize,
            filter,
            no_mipmaps,
            recursive,
            notify,
        }) => {
            let options = build_convert_options(
                format,
                resize.as_deref(),
                filter,
                no_mipmaps,
            )?;

            let mut files = Vec::new();
            collect_image_files(&input_dir, &mut files, recursive);

            if files.is_empty() {
                println!("No supported image/PAA files found in '{}'", input_dir.display());
                return Ok(());
            }

            println!("Found {} files to process...", files.len());
            let report = batch_convert(&files, &out_dir, &options, &to);

            println!("Batch conversion finished:");
            println!("  Total:      {}", report.total);
            println!("  Successful: {}", report.successful);
            println!("  Failed:     {}", report.failed);

            if !report.errors.is_empty() {
                eprintln!("\nErrors encountered:");
                for (p, err) in &report.errors {
                    eprintln!("  - {}: {err}", p.display());
                }
                if notify {
                    let title = "PAA Viewer Batch Error";
                    let msg = format!("Batch conversion failed for {} file(s)", report.failed);
                    ContextMenuManager::notify(title, &msg, true);
                }
                return Err(format!("Batch conversion completed with {} error(s)", report.failed).into());
            }

            if notify {
                let title = "PAA Viewer Batch";
                let msg = format!("Successfully converted {} files to {}", report.successful, to.to_uppercase());
                ContextMenuManager::notify(title, &msg, false);
            }
        }
        Some(Commands::ContextMenu { action }) => match action {
            ContextMenuAction::Install => {
                let msg = ContextMenuManager::install()?;
                println!("{msg}");
            }
            ContextMenuAction::Uninstall => {
                let msg = ContextMenuManager::uninstall()?;
                println!("{msg}");
            }
            ContextMenuAction::Status => {
                let installed = ContextMenuManager::is_installed();
                if installed {
                    println!("PAA Viewer is currently registered in system context menus.");
                } else {
                    println!("PAA Viewer is NOT registered in system context menus.");
                }
            }
            ContextMenuAction::ExportReg { output } => {
                let exe = std::env::current_exe()?;
                let reg = ContextMenuManager::generate_windows_reg(&exe);
                std::fs::write(&output, reg)?;
                println!("Exported Windows registry file to: {}", output.display());
            }
        },
    }

    Ok(())
}

fn collect_image_files(dir: &std::path::Path, out: &mut Vec<PathBuf>, recursive: bool) {
    let supported = ["paa", "pac", "png", "jpg", "jpeg", "tga", "bmp", "webp"];

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && recursive {
                collect_image_files(&path, out, true);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported.contains(&ext.to_ascii_lowercase().as_str()) {
                        out.push(path);
                    }
                }
            }
        }
    }
}

fn print_info_table(path: &std::path::Path, meta: &paa_viewer::paa::PaaMetadata) {
    println!("============================================================");
    println!("  PAA Texture Information: {}", path.display());
    println!("============================================================");
    println!("  Format:            {}", meta.format_name);
    println!("  Base Dimensions:   {}x{}", meta.width, meta.height);
    println!("  Aspect Ratio:      {} ({:.2})", meta.aspect_ratio, meta.aspect_ratio_num);
    println!("  Power of Two:      {}", if meta.is_power_of_two { "Yes (POT compatible)" } else { "No (Non-POT warning!)" });
    println!("  Mipmap Count:      {}", meta.mipmap_count);
    println!("  Total Mip Data:    {} bytes ({:.1} KB)", meta.total_data_size, meta.total_data_size as f64 / 1024.0);
    println!("  Transparency Flag: {}", if meta.has_alpha { "Yes (Alpha channel present)" } else { "No (Opaque)" });

    if let Some(avg) = meta.average_color {
        println!("  Average Color:     RGBA [{}, {}, {}, {}] (#{:02X}{:02X}{:02X}{:02X})",
            avg[0], avg[1], avg[2], avg[3], avg[0], avg[1], avg[2], avg[3]);
    }
    if let Some(max) = meta.max_color {
        println!("  Max Color:         RGBA [{}, {}, {}, {}] (#{:02X}{:02X}{:02X}{:02X})",
            max[0], max[1], max[2], max[3], max[0], max[1], max[2], max[3]);
    }

    println!("\n--- Mipmap Hierarchy ---");
    println!("  {:>5} | {:>11} | {:>10} | {:>12} | {:>7} | {:>10}",
        "Level", "Resolution", "Data Size", "Uncompressed", "Ratio", "Offset"
    );
    println!("  {:-<5}-+-{:-<11}-+-{:-<10}-+-{:-<12}-+-{:-<7}-+-{:-<10}", "", "", "", "", "", "");
    for m in &meta.mipmaps {
        let res = format!("{}x{}", m.width, m.height);
        let ratio_str = format!("{:.0}%", m.compression_ratio * 100.0);
        let comp_flag = if m.is_compressed { " (LZO)" } else { "" };
        let size_str = format!("{}{comp_flag}", m.data_size);
        println!("  {:>5} | {:>11} | {:>10} | {:>12} | {:>7} | {:>10}",
            m.level, res, size_str, m.uncompressed_size, ratio_str, m.file_offset
        );
    }

    if !meta.tags.is_empty() {
        println!("\n--- GGAT Tagged Sections ---");
        for t in &meta.tags {
            println!("  [{}] {} ({} bytes):", t.raw_name, t.display_name, t.length);
            println!("      Description: {}", t.description);
            println!("      Hex Data:    {}", t.data_hex);
        }
    }
    println!("============================================================");
}
