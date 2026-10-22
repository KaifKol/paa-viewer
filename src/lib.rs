pub mod cli;
pub mod context_menu;
pub mod convert;
pub mod gui;
pub mod paa;
pub mod resize;
pub mod single_instance;
pub mod thumbnail;

pub use context_menu::ContextMenuManager;
pub use convert::{batch_convert, convert_image, load_image, ConvertOptions, ConvertReport, ResizeOperation, TargetFormat};
pub use paa::{PaaError, PaaImage, PaaMetadata, PaXType};
pub use resize::{is_power_of_two, nearest_power_of_two, next_power_of_two, prev_power_of_two, resize_pot, scale_by_factor, scale_step, PotMode, ResizeFilter};
pub use thumbnail::{render_paa_thumbnail, write_paa_thumbnail};
