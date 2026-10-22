use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER,
    E_UNEXPECTED, ERROR_ALREADY_INITIALIZED, S_FALSE, S_OK,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
    RGBQUAD,
};
use windows::Win32::System::Com::{
    IClassFactory, IClassFactory_Impl, IStream, STREAM_SEEK_CUR, STREAM_SEEK_END, STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTS_ALPHATYPE, WTSAT_ARGB, WTSAT_UNKNOWN,
};
use windows_core::{HRESULT, Interface};

/// COM class identifier for the PAA thumbnail provider:
/// `{69A507B4-551F-4005-92E2-48512EB2F517}`
pub const CLSID_PAA_THUMBNAIL_PROVIDER: windows_core::GUID =
    windows_core::GUID::from_u128(0x69A507B4_551F_4005_92E2_48512EB2F517);

static OBJECT_COUNT: AtomicUsize = AtomicUsize::new(0);
static SERVER_LOCKS: AtomicUsize = AtomicUsize::new(0);

struct StreamHolder(IStream);
unsafe impl Send for StreamHolder {}
unsafe impl Sync for StreamHolder {}

struct IStreamReader<'a>(&'a IStream);

impl<'a> std::io::Read for IStreamReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let to_read = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        let mut bytes_read: u32 = 0;
        let hr = unsafe {
            self.0.Read(
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                to_read,
                Some(&mut bytes_read as *mut u32),
            )
        };
        if hr.is_err() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("IStream::Read failed with HRESULT 0x{:08X}", hr.0),
            ));
        }
        Ok(bytes_read as usize)
    }
}

impl<'a> std::io::Seek for IStreamReader<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let (origin, offset) = match pos {
            std::io::SeekFrom::Start(off) => {
                let off_i64 = i64::try_from(off)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
                (STREAM_SEEK_SET, off_i64)
            }
            std::io::SeekFrom::Current(off) => (STREAM_SEEK_CUR, off),
            std::io::SeekFrom::End(off) => (STREAM_SEEK_END, off),
        };
        let mut new_position: u64 = 0;
        unsafe {
            self.0
                .Seek(offset, origin, Some(&mut new_position as *mut u64))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        }
        Ok(new_position)
    }
}

#[windows_core::implement(IClassFactory)]
struct ClassFactory;

impl ClassFactory {
    fn new() -> Self {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Self
    }

    fn create_instance_impl(
        &self,
        punkouter: Option<&windows_core::IUnknown>,
        riid: *const windows_core::GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        if !ppvobject.is_null() {
            unsafe {
                *ppvobject = std::ptr::null_mut();
            }
        }
        if ppvobject.is_null() || riid.is_null() {
            return Err(windows_core::Error::from_hresult(E_POINTER));
        }
        if punkouter.is_some() {
            return Err(windows_core::Error::from_hresult(CLASS_E_NOAGGREGATION));
        }

        let provider: windows_core::IUnknown = ThumbnailProvider::new().into();
        let hr = unsafe { provider.query(riid, ppvobject) };
        if hr.is_err() {
            Err(windows_core::Error::from_hresult(hr))
        } else {
            Ok(())
        }
    }

    fn lock_server_impl(&self, flock: BOOL) -> windows_core::Result<()> {
        if flock.as_bool() {
            SERVER_LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            let _ = SERVER_LOCKS.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                Some(count.saturating_sub(1))
            });
        }
        Ok(())
    }
}

impl Drop for ClassFactory {
    fn drop(&mut self) {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&windows_core::IUnknown>,
        riid: *const windows_core::GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.create_instance_impl(punkouter, riid, ppvobject)
        }))
        .unwrap_or_else(|_| {
            if !ppvobject.is_null() {
                unsafe {
                    *ppvobject = std::ptr::null_mut();
                }
            }
            Err(windows_core::Error::from_hresult(E_UNEXPECTED))
        })
    }

    fn LockServer(&self, flock: BOOL) -> windows_core::Result<()> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.lock_server_impl(flock)
        }))
        .unwrap_or_else(|_| Err(windows_core::Error::from_hresult(E_UNEXPECTED)))
    }
}

#[windows_core::implement(IThumbnailProvider, IInitializeWithStream)]
struct ThumbnailProvider {
    stream: Mutex<Option<StreamHolder>>,
}

impl ThumbnailProvider {
    fn new() -> Self {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Self {
            stream: Mutex::new(None),
        }
    }

    fn initialize_impl(
        &self,
        pstream: Option<&IStream>,
        _grfmode: u32,
    ) -> windows_core::Result<()> {
        let stream = pstream.ok_or_else(|| windows_core::Error::from_hresult(E_INVALIDARG))?;

        let mut lock = self
            .stream
            .lock()
            .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;

        if lock.is_some() {
            return Err(windows_core::Error::from_hresult(HRESULT::from_win32(
                ERROR_ALREADY_INITIALIZED.0,
            )));
        }

        *lock = Some(StreamHolder(stream.clone()));
        Ok(())
    }

    fn get_thumbnail_impl(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> windows_core::Result<()> {
        if !phbmp.is_null() {
            unsafe {
                *phbmp = HBITMAP::default();
            }
        }
        if !pdwalpha.is_null() {
            unsafe {
                *pdwalpha = WTSAT_UNKNOWN;
            }
        }

        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(windows_core::Error::from_hresult(E_POINTER));
        }

        if cx == 0 {
            return Err(windows_core::Error::from_hresult(E_INVALIDARG));
        }

        let stream = {
            let lock = self
                .stream
                .lock()
                .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;
            match lock.as_ref() {
                Some(holder) => holder.0.clone(),
                None => {
                    return Err(windows_core::Error::from_hresult(E_UNEXPECTED));
                }
            }
        };

        unsafe {
            stream
                .Seek(0, STREAM_SEEK_SET, None)
                .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;
        }

        let stream_reader = IStreamReader(&stream);
        let mut reader = std::io::BufReader::new(stream_reader);
        let rgba_image = paa_viewer::thumbnail::render_paa_thumbnail(&mut reader, cx)
            .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;

        let width = rgba_image.width();
        let height = rgba_image.height();
        if width == 0 || height == 0 {
            return Err(windows_core::Error::from_hresult(E_FAIL));
        }

        let width_i32 =
            i32::try_from(width).map_err(|_| windows_core::Error::from_hresult(E_INVALIDARG))?;
        let height_i32 =
            i32::try_from(height).map_err(|_| windows_core::Error::from_hresult(E_INVALIDARG))?;

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width_i32,
                biHeight: -height_i32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default()],
        };

        let mut bits_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let hbitmap =
            unsafe { CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0) }
                .map_err(|_| windows_core::Error::from_hresult(E_FAIL))?;

        if bits_ptr.is_null() {
            unsafe {
                let _ = DeleteObject(hbitmap);
            }
            return Err(windows_core::Error::from_hresult(E_FAIL));
        }

        let total_pixels = match (width as usize).checked_mul(height as usize) {
            Some(p) => p,
            None => {
                unsafe {
                    let _ = DeleteObject(hbitmap);
                }
                return Err(windows_core::Error::from_hresult(E_FAIL));
            }
        };

        let total_bytes = match total_pixels.checked_mul(4) {
            Some(b) => b,
            None => {
                unsafe {
                    let _ = DeleteObject(hbitmap);
                }
                return Err(windows_core::Error::from_hresult(E_FAIL));
            }
        };

        let raw_src = rgba_image.as_raw();
        if raw_src.len() < total_bytes {
            unsafe {
                let _ = DeleteObject(hbitmap);
            }
            return Err(windows_core::Error::from_hresult(E_FAIL));
        }

        let dest_slice =
            unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u8, total_bytes) };

        for i in 0..total_pixels {
            let offset = i * 4;
            let r = raw_src[offset] as u32;
            let g = raw_src[offset + 1] as u32;
            let b = raw_src[offset + 2] as u32;
            let a = raw_src[offset + 3] as u32;

            if a == 0 {
                dest_slice[offset] = 0;
                dest_slice[offset + 1] = 0;
                dest_slice[offset + 2] = 0;
                dest_slice[offset + 3] = 0;
            } else if a == 255 {
                dest_slice[offset] = b as u8;
                dest_slice[offset + 1] = g as u8;
                dest_slice[offset + 2] = r as u8;
                dest_slice[offset + 3] = 255;
            } else {
                let pr = ((r * a + 127) / 255) as u8;
                let pg = ((g * a + 127) / 255) as u8;
                let pb = ((b * a + 127) / 255) as u8;

                dest_slice[offset] = pb;
                dest_slice[offset + 1] = pg;
                dest_slice[offset + 2] = pr;
                dest_slice[offset + 3] = a as u8;
            }
        }

        unsafe {
            *phbmp = hbitmap;
            *pdwalpha = WTSAT_ARGB;
        }

        Ok(())
    }
}

impl Drop for ThumbnailProvider {
    fn drop(&mut self) {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IInitializeWithStream_Impl for ThumbnailProvider_Impl {
    fn Initialize(&self, pstream: Option<&IStream>, grfmode: u32) -> windows_core::Result<()> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.initialize_impl(pstream, grfmode)
        }))
        .unwrap_or_else(|_| Err(windows_core::Error::from_hresult(E_UNEXPECTED)))
    }
}

impl IThumbnailProvider_Impl for ThumbnailProvider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> windows_core::Result<()> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.get_thumbnail_impl(cx, phbmp, pdwalpha)
        }))
        .unwrap_or_else(|_| {
            if !phbmp.is_null() {
                unsafe {
                    *phbmp = HBITMAP::default();
                }
            }
            if !pdwalpha.is_null() {
                unsafe {
                    *pdwalpha = WTSAT_UNKNOWN;
                }
            }
            Err(windows_core::Error::from_hresult(E_UNEXPECTED))
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const windows_core::GUID,
    riid: *const windows_core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if !ppv.is_null() {
        unsafe {
            *ppv = std::ptr::null_mut();
        }
    }
    match std::panic::catch_unwind(|| unsafe { dll_get_class_object_impl(rclsid, riid, ppv) }) {
        Ok(hr) => hr,
        Err(_) => E_UNEXPECTED,
    }
}

unsafe fn dll_get_class_object_impl(
    rclsid: *const windows_core::GUID,
    riid: *const windows_core::GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if ppv.is_null() || rclsid.is_null() || riid.is_null() {
        return E_POINTER;
    }

    if unsafe { *rclsid } != CLSID_PAA_THUMBNAIL_PROVIDER {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory: IClassFactory = ClassFactory::new().into();
    unsafe { factory.query(riid, ppv) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    match std::panic::catch_unwind(dll_can_unload_now_impl) {
        Ok(hr) => hr,
        Err(_) => E_UNEXPECTED,
    }
}

fn dll_can_unload_now_impl() -> HRESULT {
    if OBJECT_COUNT.load(Ordering::SeqCst) == 0 && SERVER_LOCKS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}
