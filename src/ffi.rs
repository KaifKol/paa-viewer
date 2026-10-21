use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::BufReader;
use std::os::raw::c_char;
use std::path::Path;
use crate::cli::build_convert_options;
use crate::convert::convert_image;
use crate::paa::{PaaImage, PaaMetadata};

pub struct PaaFfiHandle {
    pub paa: Option<PaaImage>,
    pub metadata: Option<PaaMetadata>,
    pub dynamic_img: Option<image::DynamicImage>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_load(path: *const c_char) -> *mut PaaFfiHandle {
    if path.is_null() {
        return std::ptr::null_mut();
    }

    let c_str = unsafe { CStr::from_ptr(path) };
    let path_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let p = Path::new(path_str);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if ext == "paa" || ext == "pac" {
        let f = match File::open(p) {
            Ok(file) => file,
            Err(_) => return std::ptr::null_mut(),
        };
        let mut reader = BufReader::new(f);
        let paa = match PaaImage::read(&mut reader) {
            Ok(img) => img,
            Err(_) => return std::ptr::null_mut(),
        };

        let meta = paa.metadata();
        let dyn_img = paa.to_dynamic_image(0).ok();

        Box::into_raw(Box::new(PaaFfiHandle {
            paa: Some(paa),
            metadata: Some(meta),
            dynamic_img: dyn_img,
        }))
    } else {
        let dyn_img = match image::open(p) {
            Ok(img) => img,
            Err(_) => return std::ptr::null_mut(),
        };

        Box::into_raw(Box::new(PaaFfiHandle {
            paa: None,
            metadata: None,
            dynamic_img: Some(dyn_img),
        }))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_free(handle: *mut PaaFfiHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_width(handle: *mut PaaFfiHandle) -> u32 {
    if handle.is_null() {
        return 0;
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        meta.width
    } else if let Some(img) = &h.dynamic_img {
        img.width()
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_height(handle: *mut PaaFfiHandle) -> u32 {
    if handle.is_null() {
        return 0;
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        meta.height
    } else if let Some(img) = &h.dynamic_img {
        img.height()
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_is_pot(handle: *mut PaaFfiHandle) -> bool {
    if handle.is_null() {
        return false;
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        meta.is_power_of_two
    } else if let Some(img) = &h.dynamic_img {
        crate::resize::is_power_of_two(img.width()) && crate::resize::is_power_of_two(img.height())
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_format_name(handle: *mut PaaFfiHandle) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let h = unsafe { &*handle };
    let name = if let Some(meta) = &h.metadata {
        meta.format_name.clone()
    } else {
        "Standard Image".to_string()
    };

    match CString::new(name) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_mipmap_count(handle: *mut PaaFfiHandle) -> u32 {
    if handle.is_null() {
        return 0;
    }
    let h = unsafe { &*handle };
    if let Some(paa) = &h.paa {
        paa.maps.len() as u32
    } else if h.dynamic_img.is_some() {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_mipmap_info(
    handle: *mut PaaFfiHandle,
    index: u32,
    out_width: *mut u32,
    out_height: *mut u32,
    out_comp_size: *mut usize,
    out_uncomp_size: *mut usize,
    out_is_compressed: *mut bool,
) -> bool {
    if handle.is_null() {
        return false;
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        if let Some(mip) = meta.mipmaps.get(index as usize) {
            unsafe {
                if !out_width.is_null() { *out_width = mip.width; }
                if !out_height.is_null() { *out_height = mip.height; }
                if !out_comp_size.is_null() { *out_comp_size = mip.data_size; }
                if !out_uncomp_size.is_null() { *out_uncomp_size = mip.uncompressed_size; }
                if !out_is_compressed.is_null() { *out_is_compressed = mip.is_compressed; }
            }
            return true;
        }
    } else if let Some(img) = &h.dynamic_img {
        if index == 0 {
            unsafe {
                if !out_width.is_null() { *out_width = img.width(); }
                if !out_height.is_null() { *out_height = img.height(); }
                if !out_comp_size.is_null() { *out_comp_size = (img.width() * img.height() * 4) as usize; }
                if !out_uncomp_size.is_null() { *out_uncomp_size = (img.width() * img.height() * 4) as usize; }
                if !out_is_compressed.is_null() { *out_is_compressed = false; }
            }
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_decode_mipmap_rgba(
    handle: *mut PaaFfiHandle,
    index: u32,
    out_buffer: *mut u8,
    buffer_len: usize,
) -> bool {
    if handle.is_null() || out_buffer.is_null() {
        return false;
    }
    let h = unsafe { &*handle };
    if let Some(paa) = &h.paa {
        match paa.to_dynamic_image(index as usize) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let raw = rgba.as_raw();
                if raw.len() > buffer_len {
                    return false;
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(raw.as_ptr(), out_buffer, raw.len());
                }
                true
            }
            Err(_) => false,
        }
    } else if let Some(img) = &h.dynamic_img {
        if index == 0 {
            let rgba = img.to_rgba8();
            let raw = rgba.as_raw();
            if raw.len() > buffer_len {
                return false;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(raw.as_ptr(), out_buffer, raw.len());
            }
            true
        } else {
            false
        }
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_colors(
    handle: *mut PaaFfiHandle,
    out_avg_rgba: *mut u8,
    out_max_rgba: *mut u8,
    out_has_alpha: *mut bool,
) -> bool {
    if handle.is_null() {
        return false;
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        unsafe {
            if !out_avg_rgba.is_null() {
                if let Some(avg) = meta.average_color {
                    std::ptr::copy_nonoverlapping(avg.as_ptr(), out_avg_rgba, 4);
                } else {
                    std::ptr::write_bytes(out_avg_rgba, 0, 4);
                }
            }
            if !out_max_rgba.is_null() {
                if let Some(max) = meta.max_color {
                    std::ptr::copy_nonoverlapping(max.as_ptr(), out_max_rgba, 4);
                } else {
                    std::ptr::write_bytes(out_max_rgba, 255, 4);
                }
            }
            if !out_has_alpha.is_null() {
                *out_has_alpha = meta.has_alpha;
            }
        }
        true
    } else {
        false
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_get_json_metadata(handle: *mut PaaFfiHandle) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let h = unsafe { &*handle };
    if let Some(meta) = &h.metadata {
        if let Ok(json) = serde_json::to_string(meta) {
            if let Ok(c) = CString::new(json) {
                return c.into_raw();
            }
        }
    }
    std::ptr::null_mut()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            drop(CString::from_raw(s));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paa_ffi_convert(
    input_path: *const c_char,
    output_path: *const c_char,
    format_name: *const c_char,
    resize_str: *const c_char,
    generate_mipmaps: bool,
) -> *mut c_char {
    if input_path.is_null() || output_path.is_null() {
        return match CString::new("Null input or output path") {
            Ok(c) => c.into_raw(),
            Err(_) => std::ptr::null_mut(),
        };
    }

    let in_str = match unsafe { CStr::from_ptr(input_path).to_str() } {
        Ok(s) => s,
        Err(e) => return CString::new(format!("Invalid input path UTF-8: {e}")).unwrap().into_raw(),
    };
    let out_str = match unsafe { CStr::from_ptr(output_path).to_str() } {
        Ok(s) => s,
        Err(e) => return CString::new(format!("Invalid output path UTF-8: {e}")).unwrap().into_raw(),
    };

    let fmt_opt = if !format_name.is_null() {
        unsafe { CStr::from_ptr(format_name).to_str().ok() }
    } else {
        None
    };

    let rsz_opt = if !resize_str.is_null() {
        unsafe { CStr::from_ptr(resize_str).to_str().ok() }
    } else {
        None
    };

    let pax_arg = fmt_opt.and_then(|s| match s.to_ascii_lowercase().as_str() {
        "dxt1" => Some(crate::cli::PaaFormatArg::Dxt1),
        "dxt3" => Some(crate::cli::PaaFormatArg::Dxt3),
        "dxt5" => Some(crate::cli::PaaFormatArg::Dxt5),
        "argb8888" => Some(crate::cli::PaaFormatArg::Argb8888),
        "argb1555" => Some(crate::cli::PaaFormatArg::Argb1555),
        "argb4444" => Some(crate::cli::PaaFormatArg::Argb4444),
        "graya" => Some(crate::cli::PaaFormatArg::Graya),
        _ => None,
    });

    let opts = match build_convert_options(
        pax_arg,
        rsz_opt,
        crate::cli::FilterArg::Lanczos,
        !generate_mipmaps,
    ) {
        Ok(o) => o,
        Err(e) => return CString::new(format!("Options error: {e}")).unwrap().into_raw(),
    };

    match convert_image(Path::new(in_str), Path::new(out_str), &opts) {
        Ok(_) => std::ptr::null_mut(), // Success: returns null
        Err(e) => CString::new(e).unwrap().into_raw(),
    }
}
