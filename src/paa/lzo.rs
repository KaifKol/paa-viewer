#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_ptr_alignment)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::alloc::{alloc, dealloc, Layout};
use std::mem;
use std::slice;
use super::error::PaaError;

pub const fn worst_compress(size: usize) -> usize {
    size + (size / 16) + 64 + 3
}

const LZO1X_MEM_COMPRESS: usize = 8192 * 16;

#[repr(i32)]
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum LzoError {
    Ok = 0,
    Error = -1,
    OutOfMemory = -2,
    NotCompressible = -3,
    InputOverrun = -4,
    OutputOverrun = -5,
    LookbehindOverrun = -6,
    EofNotFound = -7,
    InputNotConsumed = -8,
    NotYetImplemented = -9,
    InvalidArgument = -10,
}

impl std::fmt::Display for LzoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn compress(input: &[u8], output: &mut Vec<u8>) -> Result<(), PaaError> {
    if input.is_empty() {
        output.clear();
        return Ok(());
    }
    unsafe {
        let layout = Layout::from_size_align(LZO1X_MEM_COMPRESS, mem::align_of::<u8>())
            .map_err(|e| PaaError::CompressionFailed(e.to_string()))?;
        let wrkmem = alloc(layout).cast::<std::ffi::c_void>();
        if wrkmem.is_null() {
            return Err(PaaError::CompressionFailed("Failed to allocate working memory".into()));
        }

        let max_dst_len = worst_compress(input.len());
        output.resize(max_dst_len, 0);
        let mut out_len = output.len();

        let err = lzo1x_1_compress(
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            &mut out_len,
            wrkmem,
        );

        output.truncate(out_len);
        dealloc(wrkmem.cast::<u8>(), layout);

        if err == LzoError::Ok as i32 {
            Ok(())
        } else {
            Err(PaaError::CompressionFailed(format!("LZO compression error code: {err}")))
        }
    }
}

pub fn decompress_to_slice<'a>(input: &[u8], output: &'a mut [u8]) -> Result<&'a mut [u8], PaaError> {
    if input.is_empty() {
        return Ok(&mut output[..0]);
    }
    unsafe {
        let mut out_len = output.len();
        let err = lzo1x_decompress_safe(
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            &mut out_len,
        );

        if err == LzoError::Ok as i32 {
            Ok(slice::from_raw_parts_mut(output.as_mut_ptr(), out_len))
        } else {
            Err(PaaError::DecompressionFailed(format!("LZO decompression error code: {err}")))
        }
    }
}

#[inline(always)]
unsafe fn memcpy(dest: *mut u8, src: *const u8, n: usize) {
    std::ptr::copy_nonoverlapping(src, dest, n);
}

#[inline(always)]
unsafe fn memset(dest: *mut u8, c: u8, n: usize) {
    std::ptr::write_bytes(dest, c, n);
}

#[inline(always)]
unsafe fn get_unaligned_le32(p: *const u8) -> u32 {
    let mut bytes = [0u8; 4];
    memcpy(bytes.as_mut_ptr(), p, 4);
    u32::from_le_bytes(bytes)
}

#[inline(always)]
unsafe fn get_unaligned_le16(p: *const u8) -> u16 {
    let mut bytes = [0u8; 2];
    memcpy(bytes.as_mut_ptr(), p, 2);
    u16::from_le_bytes(bytes)
}

#[inline(always)]
unsafe fn put_unaligned(v: u32, p: *mut u8) {
    let bytes = v.to_ne_bytes();
    memcpy(p, bytes.as_ptr(), 4);
}

#[inline(always)]
unsafe fn get_unaligned(p: *const u8) -> u32 {
    let mut bytes = [0u8; 4];
    memcpy(bytes.as_mut_ptr(), p, 4);
    u32::from_ne_bytes(bytes)
}

unsafe fn lzo1x_1_do_compress(
    in_: *const u8,
    in_len: usize,
    out: *mut u8,
    out_len: *mut usize,
    mut ti: usize,
    wrkmem: *mut std::ffi::c_void,
) -> usize {
    let mut current_block;
    let mut ip: *const u8;
    let mut op: *mut u8;
    let in_end: *const u8 = in_.add(in_len);
    let ip_end: *const u8 = in_.add(in_len).sub(20);
    let mut ii: *const u8;
    let dict: *mut u16 = wrkmem.cast::<u16>();
    op = out;
    ip = in_;
    ii = ip;
    ip = ip.add(if ti < 4 { 4 - ti } else { 0 });
    let mut m_pos: *const u8;
    let mut t: usize;
    let mut m_len: usize;
    let mut m_off: usize;
    let mut dv: u32;

    'loop2: loop {
        ip = ip.offset(1 + ((ip as isize - ii as isize) >> 5));
        loop {
            if ip >= ip_end {
                break 'loop2;
            }
            dv = get_unaligned_le32(ip);
            t = ((dv.wrapping_mul(0x1824_429d) >> (32 - 13)) & ((1 << 13) - 1)) as usize;
            m_pos = in_.offset(*dict.add(t) as isize);
            *dict.add(t) = (ip as isize - in_ as isize) as u16;
            if dv != get_unaligned_le32(m_pos) {
                break;
            }
            ii = ii.offset(-(ti as isize));
            ti = 0;
            t = (ip as isize - ii as isize) as usize;
            if t != 0 {
                if t <= 3 {
                    let rhs = t;
                    let lhs = &mut *op.sub(2);
                    *lhs = (*lhs as usize | rhs) as u8;
                    put_unaligned(get_unaligned(ii), op);
                    op = op.add(t);
                } else if t <= 16 {
                    *op = (t - 3) as u8;
                    op = op.add(1);
                    put_unaligned(get_unaligned(ii), op);
                    put_unaligned(get_unaligned(ii.add(4)), op.add(4));
                    put_unaligned(get_unaligned(ii.add(8)), op.add(8));
                    put_unaligned(get_unaligned(ii.add(12)), op.add(12));
                    op = op.add(t);
                } else {
                    if t <= 18 {
                        *op = (t - 3) as u8;
                        op = op.add(1);
                    } else {
                        let mut tt: usize = t - 18;
                        *op = 0;
                        op = op.add(1);
                        while tt > 255 {
                            tt -= 255;
                            *op = 0;
                            op = op.add(1);
                        }
                        *op = tt as u8;
                        op = op.add(1);
                    }
                    loop {
                        put_unaligned(get_unaligned(ii), op);
                        put_unaligned(get_unaligned(ii.add(4)), op.add(4));
                        put_unaligned(get_unaligned(ii.add(8)), op.add(8));
                        put_unaligned(get_unaligned(ii.add(12)), op.add(12));
                        op = op.add(16);
                        ii = ii.add(16);
                        t -= 16;
                        if t < 16 {
                            break;
                        }
                    }
                    if t > 0 {
                        while t > 0 {
                            *op = *ii;
                            op = op.add(1);
                            ii = ii.add(1);
                            t -= 1;
                        }
                    }
                }
            }
            m_len = 4;
            if i32::from(*ip.add(m_len)) == i32::from(*m_pos.add(m_len)) {
                current_block = 22;
            } else {
                current_block = 31;
            }
            loop {
                if current_block == 22 {
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if i32::from(*ip.add(m_len)) != i32::from(*m_pos.add(m_len)) {
                        current_block = 31;
                        continue;
                    }
                    m_len += 1;
                    if ip.add(m_len) >= ip_end {
                        current_block = 31;
                        continue;
                    }
                    if i32::from(*ip.add(m_len)) == i32::from(*m_pos.add(m_len)) {
                        current_block = 22;
                    } else {
                        current_block = 31;
                    }
                } else {
                    m_off = (ip as isize - m_pos as isize) as usize;
                    ip = ip.add(m_len);
                    ii = ip;
                    if m_len <= 8 && m_off <= 0x800 {
                        current_block = 47;
                    } else {
                        current_block = 32;
                    }
                    break;
                }
            }
            if current_block == 32 {
                if m_off <= 0x4000 {
                    m_off -= 1;
                    if m_len <= 33 {
                        *op = (32 | (m_len - 2)) as u8;
                        op = op.add(1);
                    } else {
                        m_len -= 33;
                        *op = 32;
                        op = op.add(1);
                        while m_len > 255 {
                            m_len -= 255;
                            *op = 0;
                            op = op.add(1);
                        }
                        *op = m_len as u8;
                        op = op.add(1);
                    }
                } else {
                    m_off -= 0x4000;
                    if m_len <= 9 {
                        *op = (16 | ((m_off >> 11) & 8) | (m_len - 2)) as u8;
                        op = op.add(1);
                    } else {
                        m_len -= 9;
                        *op = (16 | ((m_off >> 11) & 8)) as u8;
                        op = op.add(1);
                        while m_len > 255 {
                            m_len -= 255;
                            *op = 0;
                            op = op.add(1);
                        }
                        *op = m_len as u8;
                        op = op.add(1);
                    }
                }
                *op = (m_off << 2) as u8;
                op = op.add(1);
                *op = (m_off >> 6) as u8;
                op = op.add(1);
            } else {
                m_off -= 1;
                *op = (((m_len - 1) << 5) | ((m_off & 7) << 2)) as u8;
                op = op.add(1);
                *op = (m_off >> 3) as u8;
                op = op.add(1);
            }
        }
    }
    *out_len = (op as isize - out as isize) as usize;
    (in_end as isize - ii.offset(-(ti as isize)) as isize) as usize
}

unsafe fn lzo1x_1_compress(
    in_: *const u8,
    in_len: usize,
    out: *mut u8,
    out_len: *mut usize,
    wrkmem: *mut std::ffi::c_void,
) -> i32 {
    let mut current_block;
    let mut ip: *const u8 = in_;
    let mut op: *mut u8 = out;
    let mut l: usize = in_len;
    let mut t: usize = 0;
    loop {
        if l <= 20 {
            break;
        }
        let ll: usize = if l <= 0xc000 { l } else { 0xc000 };
        let ll_end: usize = ip as usize + ll;
        if ll_end + ((t + ll) >> 5) <= ll_end {
            break;
        }
        memset(wrkmem.cast::<u8>(), 0, (1 << 13) * mem::size_of::<u16>());
        t = lzo1x_1_do_compress(ip, ll, op, out_len, t, wrkmem);
        ip = ip.add(ll);
        op = op.add(*out_len);
        l -= ll;
    }
    t += l;
    if t > 0 {
        let mut ii: *const u8 = in_.add(in_len).offset(-(t as isize));
        if std::ptr::eq(op, out) && t <= 238 {
            *op = (17 + t) as u8;
            op = op.add(1);
        } else if t <= 3 {
            let rhs = t;
            let lhs = &mut *op.sub(2);
            *lhs = (*lhs as usize | rhs) as u8;
        } else if t <= 18 {
            *op = (t - 3) as u8;
            op = op.add(1);
        } else {
            let mut tt: usize = t - 18;
            *op = 0;
            op = op.add(1);
            while tt > 255 {
                tt -= 255;
                *op = 0;
                op = op.add(1);
            }
            *op = tt as u8;
            op = op.add(1);
        }
        if t >= 16 {
            current_block = 16;
        } else {
            current_block = 18;
        }
        loop {
            if current_block == 16 {
                put_unaligned(get_unaligned(ii), op);
                put_unaligned(get_unaligned(ii.add(4)), op.add(4));
                put_unaligned(get_unaligned(ii.add(8)), op.add(8));
                put_unaligned(get_unaligned(ii.add(12)), op.add(12));
                op = op.add(16);
                ii = ii.add(16);
                t -= 16;
                if t >= 16 {
                    current_block = 16;
                } else {
                    current_block = 18;
                }
            } else if t > 0 {
                current_block = 19;
                break;
            } else {
                current_block = 21;
                break;
            }
        }
        if current_block != 21 {
            while t > 0 {
                *op = *ii;
                op = op.add(1);
                ii = ii.add(1);
                t -= 1;
            }
        }
    }
    *op = 17;
    op = op.add(1);
    *op = 0;
    op = op.add(1);
    *op = 0;
    op = op.add(1);
    *out_len = (op as isize - out as isize) as usize;
    0
}

unsafe fn lzo1x_decompress_safe(
    in_: *const u8,
    in_len: usize,
    out: *mut u8,
    out_len: *mut usize,
) -> i32 {
    let mut current_block;
    let mut op: *mut u8;
    let mut ip: *const u8;
    let mut t: usize;
    let mut next: usize;
    let mut state: usize = 0;
    let mut m_pos: *const u8;
    let ip_end: *const u8 = in_.add(in_len);
    let op_end: *mut u8 = out.add(*out_len);
    op = out;
    ip = in_;

    if in_len >= 3 {
        if i32::from(*ip) > 17 {
            t = (i32::from(*ip) - 17) as usize;
            ip = ip.add(1);
            if t < 4 {
                next = t;
                state = next;
                t = next;
                if ((ip_end as isize - ip as isize) as usize) < t + 3 {
                    current_block = 64;
                } else if ((op_end as isize - op as isize) as usize) < t {
                    current_block = 63;
                } else {
                    while t > 0 {
                        *op = *ip;
                        op = op.add(1);
                        ip = ip.add(1);
                        t -= 1;
                    }
                    current_block = 11;
                }
            } else if ((op_end as isize - op as isize) as usize) < t {
                current_block = 63;
            } else if ((ip_end as isize - ip as isize) as usize) < t + 3 {
                current_block = 64;
            } else {
                while t > 0 {
                    *op = *ip;
                    op = op.add(1);
                    ip = ip.add(1);
                    t -= 1;
                }
                state = 4;
                current_block = 11;
            }
        } else {
            current_block = 11;
        }

        if current_block != 64 {
            'loop11: loop {
                if current_block == 11 {
                    t = *ip as usize;
                    ip = ip.add(1);
                    if t < 16 {
                        if state == 0 {
                            if t == 0 {
                                let mut offset: usize;
                                let ip_last: *const u8 = ip;
                                while *ip == 0 {
                                    ip = ip.add(1);
                                    if ((ip_end as isize - ip as isize) as usize) < 1 {
                                        current_block = 64;
                                        break 'loop11;
                                    }
                                }
                                offset = (ip as isize - ip_last as isize) as usize;
                                if offset > usize::MAX / 255 - 2 {
                                    current_block = 60;
                                    break;
                                }
                                offset = (offset << 8) - offset;
                                t += offset + 15 + (*ip as usize);
                                ip = ip.add(1);
                            }
                            t += 3;
                            if ((op_end as isize - op as isize) as usize) < t {
                                current_block = 63;
                                continue;
                            }
                            if ((ip_end as isize - ip as isize) as usize) < t + 3 {
                                current_block = 64;
                                break;
                            }
                            while t > 0 {
                                *op = *ip;
                                op = op.add(1);
                                ip = ip.add(1);
                                t -= 1;
                            }
                            state = 4;
                            current_block = 11;
                            continue;
                        } else if state != 4 {
                            next = t & 3;
                            m_pos = op.wrapping_sub(1);
                            m_pos = m_pos.wrapping_offset(-((t >> 2) as isize));
                            m_pos = m_pos.wrapping_offset(-((i32::from(*ip) << 2) as isize));
                            ip = ip.add(1);
                            if m_pos < out {
                                current_block = 48;
                                break;
                            }
                            if ((op_end as isize - op as isize) as usize) < 2 {
                                current_block = 63;
                                continue;
                            }
                            *op = *m_pos;
                            *op.add(1) = *m_pos.add(1);
                            op = op.add(2);
                        } else {
                            next = t & 3;
                            m_pos = op.wrapping_offset(-(1 + 0x800));
                            m_pos = m_pos.wrapping_offset(-((t >> 2) as isize));
                            m_pos = m_pos.wrapping_offset(-((i32::from(*ip) << 2) as isize));
                            ip = ip.add(1);
                            t = 3;
                            current_block = 36;
                        }
                    } else if t >= 64 {
                        next = t & 3;
                        m_pos = op.wrapping_sub(1);
                        m_pos = m_pos.wrapping_offset(-(((t >> 2) & 7) as isize));
                        m_pos = m_pos.wrapping_offset(-((i32::from(*ip) << 3) as isize));
                        ip = ip.add(1);
                        t = (t >> 5) - 1 + 2;
                        current_block = 36;
                    } else if t >= 32 {
                        t = (t & 31) + 2;
                        if t == 2 {
                            let mut offset: usize;
                            let ip_last: *const u8 = ip;
                            while *ip == 0 {
                                ip = ip.add(1);
                                if ((ip_end as isize - ip as isize) as usize) < 1 {
                                    current_block = 64;
                                    break 'loop11;
                                }
                            }
                            offset = (ip as isize - ip_last as isize) as usize;
                            if offset > usize::MAX / 255 - 2 {
                                current_block = 30;
                                break;
                            }
                            offset = (offset << 8) - offset;
                            t += offset + 31 + (*ip as usize);
                            ip = ip.add(1);
                            if ((ip_end as isize - ip as isize) as usize) < 2 {
                                current_block = 64;
                                break;
                            }
                        }
                        m_pos = op.wrapping_sub(1);
                        next = get_unaligned_le16(ip) as usize;
                        ip = ip.add(2);
                        m_pos = m_pos.wrapping_offset(-((next >> 2) as isize));
                        next &= 3;
                        current_block = 36;
                    } else {
                        m_pos = op;
                        m_pos = m_pos.wrapping_offset(-(((t & 8) << 11) as isize));
                        t = (t & 7) + 2;
                        if t == 2 {
                            let mut offset: usize;
                            let ip_last: *const u8 = ip;
                            while *ip == 0 {
                                ip = ip.add(1);
                                if ((ip_end as isize - ip as isize) as usize) < 1 {
                                    current_block = 64;
                                    break 'loop11;
                                }
                            }
                            offset = (ip as isize - ip_last as isize) as usize;
                            if offset > usize::MAX / 255 - 2 {
                                current_block = 22;
                                break;
                            }
                            offset = (offset << 8) - offset;
                            t += offset + 7 + (*ip as usize);
                            ip = ip.add(1);
                            if ((ip_end as isize - ip as isize) as usize) < 2 {
                                current_block = 64;
                                break;
                            }
                        }
                        next = get_unaligned_le16(ip) as usize;
                        ip = ip.add(2);
                        m_pos = m_pos.wrapping_offset(-((next >> 2) as isize));
                        next &= 3;
                        if std::ptr::eq(m_pos, op) {
                            current_block = 21;
                            break;
                        }
                        m_pos = m_pos.wrapping_sub(0x4000);
                        current_block = 36;
                    }

                    if current_block == 36 {
                        if m_pos < out {
                            current_block = 48;
                            break;
                        }
                        let oe: *mut u8 = op.wrapping_add(t);
                        if ((op_end as isize - op as isize) as usize) < t {
                            current_block = 63;
                            continue;
                        }
                        *op = *m_pos;
                        *op.add(1) = *m_pos.add(1);
                        op = op.add(2);
                        m_pos = m_pos.add(2);
                        while op < oe {
                            *op = *m_pos;
                            op = op.add(1);
                            m_pos = m_pos.add(1);
                        }
                    }
                    state = next;
                    t = next;
                    if ((ip_end as isize - ip as isize) as usize) < t + 3 {
                        current_block = 64;
                        break;
                    }
                    if ((op_end as isize - op as isize) as usize) < t {
                        current_block = 63;
                        continue;
                    }
                    while t > 0 {
                        *op = *ip;
                        op = op.add(1);
                        ip = ip.add(1);
                        t -= 1;
                    }
                    current_block = 11;
                } else {
                    *out_len = (op as isize - out as isize) as usize;
                    return LzoError::OutputOverrun as i32;
                }
            }

            if current_block == 64 {
            } else if current_block == 21 {
                *out_len = (op as isize - out as isize) as usize;
                return if t != 3 {
                    -1
                } else if std::ptr::eq(ip, ip_end) {
                    0
                } else if ip < ip_end {
                    -8
                } else {
                    -4
                };
            } else if current_block == 22 || current_block == 30 {
                return -1;
            } else if current_block == 48 {
                *out_len = (op as isize - out as isize) as usize;
                return -6;
            } else {
                return -1;
            }
        }
    }
    *out_len = (op as isize - out as isize) as usize;
    -4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lzo_roundtrip() {
        let original = b"Arma 3 PAA texture test string with repeating patterns 1234567890 1234567890 hello world!";
        let mut compressed = Vec::new();
        compress(original, &mut compressed).expect("compress failed");
        assert!(!compressed.is_empty());

        let mut decompressed = vec![0u8; original.len()];
        let dec_slice = decompress_to_slice(&compressed, &mut decompressed).expect("decompress failed");
        assert_eq!(dec_slice, original);
    }
}
