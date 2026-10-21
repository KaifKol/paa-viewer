use std::path::PathBuf;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use image::{DynamicImage, GenericImageView};

use crate::paa::{PaaImage, PaaMetadata, PaXType};
use crate::resize::{is_power_of_two, next_power_of_two, resize_pot, scale_by_factor, scale_step, PotMode, ResizeFilter};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ChannelMode {
    Rgba,
    Rgb,
    Red,
    Green,
    Blue,
    Alpha,
}

pub struct PaaViewerApp {
    pub file_path: Option<PathBuf>,
    pub paa_image: Option<PaaImage>,
    pub metadata: Option<PaaMetadata>,
    pub current_mip_level: usize,
    pub current_image: Option<DynamicImage>,
    pub texture: Option<egui::TextureHandle>,

    // Viewport
    pub zoom: f32,
    pub pan: Vec2,
    pub fit_to_window: bool,
    pub channel_mode: ChannelMode,
    pub cursor_info: Option<(u32, u32, [u8; 4])>,
    pub linear_filter: bool,

    // Dialog states
    pub show_resize_modal: bool,
    pub resize_factor_input: String,
    pub resize_filter: ResizeFilter,
    pub status_message: Option<(String, bool)>, // (msg, is_error)

    // Export states
    pub export_pax_type: PaXType,
    pub export_generate_mipmaps: bool,
}

impl PaaViewerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        let mut app = Self {
            file_path: None,
            paa_image: None,
            metadata: None,
            current_mip_level: 0,
            current_image: None,
            texture: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_to_window: true,
            channel_mode: ChannelMode::Rgba,
            cursor_info: None,
            linear_filter: false,
            show_resize_modal: false,
            resize_factor_input: "0.5".to_string(),
            resize_filter: ResizeFilter::Lanczos3,
            status_message: None,
            export_pax_type: PaXType::DXT5,
            export_generate_mipmaps: true,
        };

        if let Some(path) = initial_file {
            app.load_file(path, &_cc.egui_ctx);
        }

        app
    }

    pub fn load_file(&mut self, path: PathBuf, ctx: &egui::Context) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if ext == "paa" || ext == "pac" {
            match std::fs::File::open(&path) {
                Ok(file) => {
                    let mut reader = std::io::BufReader::new(file);
                    match PaaImage::read(&mut reader) {
                        Ok(paa) => {
                            self.metadata = Some(paa.metadata());
                            match paa.to_dynamic_image(0) {
                                Ok(img) => {
                                    self.file_path = Some(path);
                                    self.current_mip_level = 0;
                                    self.current_image = Some(img);
                                    self.paa_image = Some(paa);
                                    self.fit_to_window = true;
                                    self.update_texture(ctx);
                                    self.status_message = Some(("Loaded PAA file successfully".to_string(), false));
                                }
                                Err(e) => {
                                    self.status_message = Some((format!("Failed to decode PAA mipmap 0: {e}"), true));
                                }
                            }
                        }
                        Err(e) => {
                            self.status_message = Some((format!("Failed to read PAA: {e}"), true));
                        }
                    }
                }
                Err(e) => {
                    self.status_message = Some((format!("Failed to open file: {e}"), true));
                }
            }
        } else {
            match image::open(&path) {
                Ok(img) => {
                    self.file_path = Some(path);
                    self.paa_image = None;
                    self.metadata = None;
                    self.current_mip_level = 0;
                    self.current_image = Some(img);
                    self.fit_to_window = true;
                    self.update_texture(ctx);
                    self.status_message = Some(("Loaded image successfully".to_string(), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Failed to load image: {e}"), true));
                }
            }
        }
    }

    pub fn select_mipmap_level(&mut self, level: usize, ctx: &egui::Context) {
        if let Some(paa) = &self.paa_image {
            if level < paa.maps.len() {
                match paa.to_dynamic_image(level) {
                    Ok(img) => {
                        self.current_mip_level = level;
                        self.current_image = Some(img);
                        self.update_texture(ctx);
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Failed to decode mipmap level {level}: {e}"), true));
                    }
                }
            }
        }
    }

    pub fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(img) = &self.current_image {
            let (w, h) = img.dimensions();
            let rgba = img.to_rgba8();

            let pixels: Vec<Color32> = rgba
                .pixels()
                .map(|p| match self.channel_mode {
                    ChannelMode::Rgba => Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]),
                    ChannelMode::Rgb => Color32::from_rgb(p[0], p[1], p[2]),
                    ChannelMode::Red => Color32::from_rgb(p[0], 0, 0),
                    ChannelMode::Green => Color32::from_rgb(0, p[1], 0),
                    ChannelMode::Blue => Color32::from_rgb(0, 0, p[2]),
                    ChannelMode::Alpha => Color32::from_gray(p[3]),
                })
                .collect();

            let color_image = egui::ColorImage {
                size: [w as usize, h as usize],
                pixels,
            };

            let options = if self.linear_filter {
                egui::TextureOptions::LINEAR
            } else {
                egui::TextureOptions::NEAREST
            };

            self.texture = Some(ctx.load_texture("view_texture", color_image, options));
        } else {
            self.texture = None;
        }
    }

    pub fn export_current_image(&mut self, path: PathBuf) {
        if let Some(img) = &self.current_image {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();

            let res = if ext == "paa" || ext == "pac" {
                match PaaImage::from_dynamic_image(img, self.export_pax_type, self.export_generate_mipmaps) {
                    Ok(paa) => match std::fs::File::create(&path) {
                        Ok(file) => {
                            let mut writer = std::io::BufWriter::new(file);
                            paa.write(&mut writer).map_err(|e| e.to_string())
                        }
                        Err(e) => Err(e.to_string()),
                    },
                    Err(e) => Err(e.to_string()),
                }
            } else {
                let fmt = match ext.as_str() {
                    "png" => image::ImageFormat::Png,
                    "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                    "tga" => image::ImageFormat::Tga,
                    "bmp" => image::ImageFormat::Bmp,
                    "webp" => image::ImageFormat::WebP,
                    _ => image::ImageFormat::Png,
                };
                img.save_with_format(&path, fmt).map_err(|e| e.to_string())
            };

            match res {
                Ok(()) => {
                    self.status_message = Some((format!("Exported successfully to {}", path.display()), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Export error: {e}"), true));
                }
            }
        }
    }

    pub fn apply_resize(&mut self, new_img: DynamicImage, ctx: &egui::Context) {
        // If current image was a PAA, regenerate PAA with new dimensions
        if let Some(paa) = &self.paa_image {
            let format = paa.format;
            match PaaImage::from_dynamic_image(&new_img, format, true) {
                Ok(new_paa) => {
                    self.metadata = Some(new_paa.metadata());
                    self.paa_image = Some(new_paa);
                    self.current_mip_level = 0;
                    self.current_image = Some(new_img);
                    self.update_texture(ctx);
                    self.status_message = Some(("Resized image and generated new mipmaps".into(), false));
                }
                Err(e) => {
                    self.status_message = Some((format!("Resize error: {e}"), true));
                }
            }
        } else {
            self.current_image = Some(new_img);
            self.update_texture(ctx);
            self.status_message = Some(("Resized image successfully".into(), false));
        }
    }
}

impl eframe::App for PaaViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle Drag & Drop of files
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped_files.first() {
            if let Some(path) = &file.path {
                self.load_file(path.clone(), ctx);
            }
        }

        // Top Control Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("PAA Viewer & Converter");
                ui.separator();

                if ui.button("📁 Open File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images & Textures", &["paa", "pac", "png", "jpg", "jpeg", "tga", "bmp", "webp"])
                        .add_filter("Bohemia PAA (*.paa)", &["paa", "pac"])
                        .add_filter("PNG Images (*.png)", &["png"])
                        .pick_file()
                    {
                        self.load_file(path, ctx);
                    }
                }

                if ui.button("💾 Export As...").clicked() {
                    if self.current_image.is_some() {
                        let mut dialog = rfd::FileDialog::new()
                            .add_filter("Bohemia PAA (*.paa)", &["paa"])
                            .add_filter("PNG Image (*.png)", &["png"])
                            .add_filter("JPEG Image (*.jpg)", &["jpg", "jpeg"])
                            .add_filter("TGA Image (*.tga)", &["tga"])
                            .add_filter("WebP Image (*.webp)", &["webp"]);

                        if let Some(orig) = &self.file_path {
                            if let Some(stem) = orig.file_stem() {
                                dialog = dialog.set_file_name(stem.to_string_lossy());
                            }
                        }

                        if let Some(path) = dialog.save_file() {
                            self.export_current_image(path);
                        }
                    }
                }

                ui.separator();

                // Quick resize actions
                ui.label("Scale:");
                if ui.button("÷2 (x0.5)").on_hover_text("Half resolution").clicked() {
                    if let Some(img) = self.current_image.clone() {
                        let resized = scale_step(&img, -1, self.resize_filter);
                        self.apply_resize(resized, ctx);
                    }
                }

                if ui.button("x2 (Double)").on_hover_text("Double resolution").clicked() {
                    if let Some(img) = self.current_image.clone() {
                        let resized = scale_step(&img, 1, self.resize_filter);
                        self.apply_resize(resized, ctx);
                    }
                }

                if ui.button("⚙ Resize Dialog...").clicked() {
                    self.show_resize_modal = true;
                }

                ui.separator();

                // Channel toggles
                ui.label("Channels:");
                let mut mode = self.channel_mode;
                ui.selectable_value(&mut mode, ChannelMode::Rgba, "RGBA");
                ui.selectable_value(&mut mode, ChannelMode::Rgb, "RGB");
                ui.selectable_value(&mut mode, ChannelMode::Red, "R");
                ui.selectable_value(&mut mode, ChannelMode::Green, "G");
                ui.selectable_value(&mut mode, ChannelMode::Blue, "B");
                ui.selectable_value(&mut mode, ChannelMode::Alpha, "A");
                if mode != self.channel_mode {
                    self.channel_mode = mode;
                    self.update_texture(ctx);
                }

                ui.separator();

                // View settings
                if ui.button("1:1").on_hover_text("Reset zoom to 100%").clicked() {
                    self.zoom = 1.0;
                    self.pan = Vec2::ZERO;
                    self.fit_to_window = false;
                }

                if ui.button("Fit").on_hover_text("Fit image inside view window").clicked() {
                    self.fit_to_window = true;
                    self.pan = Vec2::ZERO;
                }

                if ui.checkbox(&mut self.linear_filter, "Bilinear").on_hover_text("Toggle linear texture filtering").changed() {
                    self.update_texture(ctx);
                }
            });
        });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((msg, is_err)) = &self.status_message {
                    let color = if *is_err { Color32::RED } else { Color32::GREEN };
                    ui.colored_label(color, msg);
                    ui.separator();
                }

                if let Some((x, y, rgba)) = self.cursor_info {
                    ui.label(format!("Cursor: ({x}, {y})"));
                    ui.label(format!("RGBA: [{}, {}, {}, {}]", rgba[0], rgba[1], rgba[2], rgba[3]));
                    let hex = format!("#{:02X}{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2], rgba[3]);
                    ui.label(hex);
                    // Color swatch
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(rect, CornerRadius::same(2), Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]));
                    ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0_f32, Color32::GRAY), StrokeKind::Inside);
                    ui.separator();
                }

                if let Some(img) = &self.current_image {
                    let (w, h) = img.dimensions();
                    ui.label(format!("Current: {w}x{h}"));
                    ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                } else {
                    ui.label("No file loaded. Drag & drop a PAA/PNG file here or click 'Open File'");
                }
            });
        });

        // Right Info Sidebar
        egui::SidePanel::right("info_panel")
            .min_width(280.0)
            .default_width(320.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Texture Info");
                    ui.separator();

                    if let Some(path) = &self.file_path {
                        ui.label(format!("File: {}", path.file_name().and_then(|s| s.to_str()).unwrap_or("?")));
                        if let Ok(meta) = std::fs::metadata(path) {
                            let kb = meta.len() as f64 / 1024.0;
                            ui.label(format!("File Size: {:.1} KB ({} bytes)", kb, meta.len()));
                        }
                    }

                    let mut selected_mip = None;
                    if let Some(meta) = &self.metadata {
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(format!("Format: {}", meta.format_name)).strong());
                            ui.label(format!("Base Dimensions: {}x{}", meta.width, meta.height));
                            ui.label(format!("Aspect Ratio: {} ({:.2})", meta.aspect_ratio, meta.aspect_ratio_num));

                            if meta.is_power_of_two {
                                ui.colored_label(Color32::from_rgb(0, 200, 100), "✔ Power of Two (Arma compatible)");
                            } else {
                                ui.colored_label(Color32::from_rgb(255, 180, 0), "⚠ Non-POT: Dimensions not powers of 2!");
                            }

                            ui.label(format!("Mipmap Count: {}", meta.mipmap_count));
                            let comp_kb = meta.total_data_size as f64 / 1024.0;
                            ui.label(format!("Total Mip Data: {:.1} KB", comp_kb));
                        });

                        // Colors and tags
                        ui.add_space(8.0);
                        ui.heading("Color & Tags");
                        ui.separator();

                        ui.group(|ui| {
                            if let Some(avg) = meta.average_color {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
                                    ui.painter().rect_filled(rect, CornerRadius::same(3), Color32::from_rgba_unmultiplied(avg[0], avg[1], avg[2], avg[3]));
                                    ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
                                    ui.label(format!("Avg Color: [{}, {}, {}, {}]", avg[0], avg[1], avg[2], avg[3]));
                                });
                            }

                            if let Some(max) = meta.max_color {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::hover());
                                    ui.painter().rect_filled(rect, CornerRadius::same(3), Color32::from_rgba_unmultiplied(max[0], max[1], max[2], max[3]));
                                    ui.painter().rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0_f32, Color32::WHITE), StrokeKind::Inside);
                                    ui.label(format!("Max Color: [{}, {}, {}, {}]", max[0], max[1], max[2], max[3]));
                                });
                            }

                            ui.label(format!("Transparency Flag: {}", if meta.has_alpha { "Yes (Alpha < 255)" } else { "No (Opaque)" }));
                        });

                        // Tag sections
                        if !meta.tags.is_empty() {
                            ui.add_space(8.0);
                            ui.collapsing("All GGAT Tags", |ui| {
                                for tag in &meta.tags {
                                    ui.group(|ui| {
                                        ui.strong(&tag.display_name);
                                        ui.label(&tag.description);
                                        ui.label(format!("Size: {} bytes", tag.length));
                                        ui.label(egui::RichText::new(&tag.data_hex).monospace().size(11.0));
                                    });
                                }
                            });
                        }

                        // Mipmap selector list
                        ui.add_space(8.0);
                        ui.heading("Mipmap Levels");
                        ui.separator();

                        for (i, mip) in meta.mipmaps.iter().enumerate() {
                            let is_selected = self.current_mip_level == i;
                            let btn_label = format!(
                                "Level {}: {}x{} ({} bytes, {:.0}%)",
                                mip.level,
                                mip.width,
                                mip.height,
                                mip.data_size,
                                mip.compression_ratio * 100.0
                            );

                            if ui.selectable_label(is_selected, btn_label).clicked() {
                                selected_mip = Some(i);
                            }
                        }
                    }
                    if let Some(i) = selected_mip {
                        self.select_mipmap_level(i, ctx);
                    }
                    if self.metadata.is_none() {
                        if let Some(img) = &self.current_image {
                            let (w, h) = img.dimensions();
                            let is_pot = is_power_of_two(w) && is_power_of_two(h);

                            ui.group(|ui| {
                                ui.label("Format: Standard Image (PNG/JPG/etc)");
                                ui.label(format!("Dimensions: {w}x{h}"));
                                if is_pot {
                                    ui.colored_label(Color32::from_rgb(0, 200, 100), "✔ Power of Two");
                                } else {
                                    ui.colored_label(Color32::from_rgb(255, 180, 0), "⚠ Non-POT: Not a power of two");
                                }
                            });
                        }
                    }

                    // Quick POT Resize Buttons
                    ui.add_space(12.0);
                    ui.heading("Power of Two Alignment");
                    ui.separator();

                    if let Some(img) = self.current_image.clone() {
                        let (w, h) = img.dimensions();
                        let next_w = next_power_of_two(w);
                        let next_h = next_power_of_two(h);

                        if ui.button(format!("Next POT ({}x{})", next_w, next_h)).clicked() {
                            let resized = resize_pot(&img, PotMode::Next, self.resize_filter);
                            self.apply_resize(resized, ctx);
                        }

                        ui.horizontal(|ui| {
                            for &preset in &[256, 512, 1024, 2048] {
                                if ui.button(format!("{preset}")).clicked() {
                                    let resized = resize_pot(
                                        &img,
                                        PotMode::Stretch { target_w: preset, target_h: preset },
                                        self.resize_filter,
                                    );
                                    self.apply_resize(resized, ctx);
                                }
                            }
                        });
                    }

                    // PAA Export settings
                    ui.add_space(12.0);
                    ui.heading("PAA Export Settings");
                    ui.separator();
                    ui.group(|ui| {
                        ui.label("Format:");
                        egui::ComboBox::from_id_salt("pax_format_combo")
                            .selected_text(format!("{:?}", self.export_pax_type))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.export_pax_type, PaXType::DXT1, "DXT1 (Opaque / 1-bit Alpha)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::DXT3, "DXT3 (Explicit Alpha)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::DXT5, "DXT5 (Smooth Interpolated Alpha)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::ARGB8, "ARGB8888 (Uncompressed 32-bit)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::ARGB5, "ARGB1555 (16-bit 1-bit Alpha)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::ARGB4, "ARGB4444 (16-bit 4-bit Alpha)");
                                ui.selectable_value(&mut self.export_pax_type, PaXType::GRAYA, "GRAYA (Grayscale)");
                            });

                        ui.checkbox(&mut self.export_generate_mipmaps, "Generate Mipmaps");
                    });

                    // System Integration & Context Menu
                    ui.add_space(12.0);
                    ui.heading("System Integration");
                    ui.separator();
                    ui.group(|ui| {
                        let is_reg = crate::context_menu::ContextMenuManager::is_installed();
                        if is_reg {
                            ui.colored_label(Color32::from_rgb(0, 200, 100), "✔ Registered in context menu");
                            if ui.button("Remove from Context Menu").clicked() {
                                match crate::context_menu::ContextMenuManager::uninstall() {
                                    Ok(msg) => self.status_message = Some((msg, false)),
                                    Err(err) => self.status_message = Some((err, true)),
                                }
                            }
                        } else {
                            ui.label("Not registered in context menu");
                            if ui.button("Register in Context Menu").clicked() {
                                match crate::context_menu::ContextMenuManager::install() {
                                    Ok(msg) => self.status_message = Some((msg, false)),
                                    Err(err) => self.status_message = Some((err, true)),
                                }
                            }
                        }

                        if ui.button("Export .reg for Windows").on_hover_text("Export Windows registry file to register context menu on Windows").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Registry Script (*.reg)", &["reg"])
                                .set_file_name("paa-viewer-context-menu.reg")
                                .save_file()
                            {
                                let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("paa-viewer.exe"));
                                let reg = crate::context_menu::ContextMenuManager::generate_windows_reg(&exe);
                                if let Err(e) = std::fs::write(&path, reg) {
                                    self.status_message = Some((format!("Failed to write .reg file: {e}"), true));
                                } else {
                                    self.status_message = Some((format!("Exported .reg file to {}", path.display()), false));
                                }
                            }
                        }
                    });
                });
            });

        // Center Viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let viewport_rect = response.rect;

            if let Some(texture) = &self.texture {
                let img_size = texture.size_vec2();

                // Compute fit zoom if requested
                if self.fit_to_window {
                    let padding = 32.0;
                    let avail_w = (viewport_rect.width() - padding).max(10.0);
                    let avail_h = (viewport_rect.height() - padding).max(10.0);
                    let scale_w = avail_w / img_size.x;
                    let scale_h = avail_h / img_size.y;
                    self.zoom = scale_w.min(scale_h).min(20.0).max(0.01);
                }

                // Handle zoom with scroll wheel
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 && response.hovered() {
                    let zoom_factor = if scroll > 0.0 { 1.15 } else { 0.85 };
                    let old_zoom = self.zoom;
                    self.zoom = (self.zoom * zoom_factor).clamp(0.02, 50.0);
                    self.fit_to_window = false;

                    // Zoom towards mouse cursor
                    if let Some(pointer_pos) = response.hover_pos() {
                        let center = viewport_rect.center() + self.pan;
                        let pointer_rel = pointer_pos - center;
                        self.pan -= pointer_rel * (self.zoom / old_zoom - 1.0);
                    }
                }

                // Handle panning with drag
                if response.dragged() {
                    self.pan += response.drag_delta();
                    self.fit_to_window = false;
                }

                let render_size = img_size * self.zoom;
                let center = viewport_rect.center() + self.pan;
                let image_rect = Rect::from_center_size(center, render_size);

                // Draw checkerboard background for transparent textures
                draw_checkerboard(&painter, image_rect);

                // Draw Texture
                let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
                painter.image(texture.id(), image_rect, uv, Color32::WHITE);

                // Draw subtle border around image
                painter.rect_stroke(image_rect, CornerRadius::ZERO, Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)), StrokeKind::Outside);

                // Calculate cursor pixel coordinates
                if let Some(mouse_pos) = response.hover_pos() {
                    if image_rect.contains(mouse_pos) {
                        let rel_x = (mouse_pos.x - image_rect.min.x) / image_rect.width();
                        let rel_y = (mouse_pos.y - image_rect.min.y) / image_rect.height();

                        let px = (rel_x * img_size.x).floor() as u32;
                        let py = (rel_y * img_size.y).floor() as u32;

                        if let Some(img) = &self.current_image {
                            let (w, h) = img.dimensions();
                            if px < w && py < h {
                                let pixel = img.get_pixel(px, py);
                                self.cursor_info = Some((px, py, [pixel[0], pixel[1], pixel[2], pixel[3]]));
                            }
                        }
                    } else {
                        self.cursor_info = None;
                    }
                } else {
                    self.cursor_info = None;
                }
            } else {
                // Empty state text
                painter.text(
                    viewport_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Drop a PAA or Image file here, or click 'Open File' in top bar",
                    egui::FontId::proportional(18.0),
                    Color32::from_rgb(140, 140, 140),
                );
            }
        });

        // Resize Dialog Window
        if self.show_resize_modal {
            let mut show_modal = self.show_resize_modal;
            let mut pending_resize = None;
            let mut close_modal = false;
            let mut factor_input = self.resize_factor_input.clone();
            let mut filter = self.resize_filter;
            let current_img = self.current_image.clone();

            egui::Window::new("Resize Image")
                .collapsible(false)
                .resizable(false)
                .open(&mut show_modal)
                .show(ctx, |ui| {
                    ui.label("Scale by factor (multiplier):");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut factor_input);
                        if ui.button("Apply Factor").clicked() {
                            if let Ok(factor) = factor_input.trim().parse::<f32>() {
                                if factor > 0.0 {
                                    if let Some(img) = current_img {
                                        let resized = scale_by_factor(&img, factor, filter);
                                        pending_resize = Some(resized);
                                        close_modal = true;
                                    }
                                }
                            }
                        }
                    });

                    ui.separator();
                    ui.label("Filter:");
                    egui::ComboBox::from_id_salt("modal_filter_combo")
                        .selected_text(format!("{filter:?}"))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut filter, ResizeFilter::Lanczos3, "Lanczos3 (High Quality)");
                            ui.selectable_value(&mut filter, ResizeFilter::Triangle, "Triangle (Bilinear)");
                            ui.selectable_value(&mut filter, ResizeFilter::Nearest, "Nearest Neighbor");
                        });
                });

            self.resize_factor_input = factor_input;
            self.resize_filter = filter;
            self.show_resize_modal = show_modal && !close_modal;

            if let Some(resized) = pending_resize {
                self.apply_resize(resized, ctx);
            }
        }
    }
}

fn draw_checkerboard(painter: &egui::Painter, rect: Rect) {
    let check_size = 12.0;
    let col1 = Color32::from_rgb(40, 40, 40);
    let col2 = Color32::from_rgb(55, 55, 55);

    painter.rect_filled(rect, CornerRadius::ZERO, col1);

    let start_x = rect.min.x;
    let start_y = rect.min.y;
    let end_x = rect.max.x;
    let end_y = rect.max.y;

    let cols = ((end_x - start_x) / check_size).ceil() as usize;
    let rows = ((end_y - start_y) / check_size).ceil() as usize;

    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 1 {
                let cell_min = Pos2::new(
                    (start_x + x as f32 * check_size).min(end_x),
                    (start_y + y as f32 * check_size).min(end_y),
                );
                let cell_max = Pos2::new(
                    (cell_min.x + check_size).min(end_x),
                    (cell_min.y + check_size).min(end_y),
                );
                painter.rect_filled(Rect::from_min_max(cell_min, cell_max), CornerRadius::ZERO, col2);
            }
        }
    }
}
