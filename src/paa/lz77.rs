use super::error::PaaError;

pub fn compress(input: &[u8], out_buf: &mut [u8]) -> Result<usize, PaaError> {
    const MAX_OFFSET: usize = 4095; // 12 bits
    const MIN_MATCH: usize = 3;
    const MAX_MATCH: usize = 18; // 4 bits + 3
    const MAX_CHAIN: usize = 32;
    const HASH_BITS: u32 = 15;
    const HASH_SIZE: usize = 1 << HASH_BITS;
    const NONE: usize = usize::MAX;

    #[inline]
    const fn hash3(a: u8, b: u8, c: u8) -> usize {
        let v = (a as u32) | ((b as u32) << 8) | ((c as u32) << 16);
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    }

    if input.is_empty() {
        return Ok(0);
    }

    let mut pi = 0;
    let mut po = 0;
    let mut checksum: u32 = 0;

    let mut head = vec![NONE; HASH_SIZE];
    let mut prev = vec![NONE; input.len()];

    while pi < input.len() {
        if po >= out_buf.len() {
            return Err(PaaError::CompressionFailed("Output buffer too small".into()));
        }

        let flag_pos = po;
        po += 1;
        let mut flag: u8 = 0;

        for bit in 0..8 {
            if pi >= input.len() {
                break;
            }

            let mut best_match_pos = 0;
            let mut best_match_len = 0;

            if pi + MIN_MATCH <= input.len() {
                let hash = hash3(input[pi], input[pi + 1], input[pi + 2]);
                let search_start = pi.saturating_sub(MAX_OFFSET);

                let mut search_pos = head[hash];
                let mut steps = 0;
                while search_pos != NONE && search_pos >= search_start && steps < MAX_CHAIN {
                    let mut match_len = 0;
                    while match_len < MAX_MATCH
                        && pi + match_len < input.len()
                        && search_pos + match_len < input.len()
                        && input[search_pos + match_len] == input[pi + match_len]
                    {
                        match_len += 1;
                    }

                    if match_len >= MIN_MATCH && match_len > best_match_len {
                        best_match_len = match_len;
                        best_match_pos = pi - search_pos;
                        if match_len == MAX_MATCH {
                            break;
                        }
                    }

                    search_pos = prev[search_pos];
                    steps += 1;
                }

                prev[pi] = head[hash];
                head[hash] = pi;
            }

            if best_match_len >= MIN_MATCH {
                if po + 2 > out_buf.len() {
                    return Err(PaaError::CompressionFailed("Output buffer too small".into()));
                }

                let offset = best_match_pos;
                let length = (best_match_len - 3) as u8;

                out_buf[po] = (offset & 0xFF) as u8;
                po += 1;
                out_buf[po] = (((offset >> 4) & 0xF0) as u8) | (length & 0x0F);
                po += 1;

                for i in 0..best_match_len {
                    checksum = checksum.wrapping_add(u32::from(input[pi + i]));
                }

                pi += best_match_len;
            } else {
                if po >= out_buf.len() {
                    return Err(PaaError::CompressionFailed("Output buffer too small".into()));
                }

                flag |= 1 << bit;
                out_buf[po] = input[pi];
                checksum = checksum.wrapping_add(u32::from(input[pi]));
                po += 1;
                pi += 1;
            }
        }

        out_buf[flag_pos] = flag;
    }

    if po + 4 > out_buf.len() {
        return Err(PaaError::CompressionFailed("Output buffer too small for checksum".into()));
    }

    let checksum_bytes = checksum.to_le_bytes();
    out_buf[po..po + 4].copy_from_slice(&checksum_bytes);
    po += 4;

    Ok(po)
}

pub fn decompress(input: &[u8], out_buf: &mut [u8]) -> Result<usize, PaaError> {
    if input.is_empty() && out_buf.is_empty() {
        return Ok(0);
    }
    let outlen = out_buf.len();
    let mut flag: u8 = 0;
    let mut rpos: usize;
    let mut rlen: u8;
    let mut fl: usize = 0;
    let mut pi: usize = 0;
    let mut data: u8;

    let mut remaining_outlen = outlen;

    'outer: while remaining_outlen > 0 {
        if pi >= input.len() {
            return Err(PaaError::DecompressionFailed("Unexpected end of input data".into()));
        }

        flag = input[pi];
        pi += 1;

        for _ in 0..8 {
            if (flag & 0x01) != 0 {
                if pi >= input.len() {
                    return Err(PaaError::DecompressionFailed("Unexpected end of input data during raw byte read".into()));
                }

                data = input[pi];
                pi += 1;
                out_buf[fl] = data;
                fl += 1;

                remaining_outlen -= 1;
                if remaining_outlen == 0 {
                    break 'outer;
                }
            } else {
                if pi + 1 >= input.len() {
                    return Err(PaaError::DecompressionFailed("Unexpected end of input data during back reference read".into()));
                }

                rpos = input[pi] as usize;
                pi += 1;

                rlen = (input[pi] & 0x0F) + 3;
                rpos += ((input[pi] & 0xF0) as usize) << 4;
                pi += 1;

                let mut skip_backref = false;
                while rpos > fl {
                    out_buf[fl] = 0x20;
                    fl += 1;

                    remaining_outlen -= 1;
                    if remaining_outlen == 0 {
                        break 'outer;
                    }

                    rlen -= 1;
                    if rlen == 0 {
                        skip_backref = true;
                        break;
                    }
                }

                if !skip_backref {
                    rpos = fl - rpos;

                    for _ in 0..rlen {
                        data = out_buf[rpos];
                        out_buf[fl] = data;
                        fl += 1;
                        rpos += 1;

                        remaining_outlen -= 1;
                        if remaining_outlen == 0 {
                            break 'outer;
                        }
                    }
                }
            }

            flag >>= 1;
        }
    }

    if flag & 0xFE != 0 {
        return Err(PaaError::DecompressionFailed("Excess bits in final flag byte".into()));
    }

    if pi + 3 >= input.len() {
        return Err(PaaError::DecompressionFailed("Cannot read checksum: unexpected end of input".into()));
    }

    let stored_checksum = u32::from_le_bytes([input[pi], input[pi + 1], input[pi + 2], input[pi + 3]]);
    let calc_checksum = out_buf.iter().fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b)));
    if stored_checksum != calc_checksum {
        return Err(PaaError::DecompressionFailed(format!(
            "LZ77 checksum mismatch: stored {stored_checksum:#010x}, calculated {calc_checksum:#010x}"
        )));
    }

    Ok(pi + 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz77_roundtrip() {
        let data = b"Hello, Hello, Hello, Hello, Hello, Hello, Hello, Hello! Arma 3 texture PAA test 1234567890";
        let mut compressed = vec![0u8; data.len() * 2 + 32];
        let compressed_size = compress(data, &mut compressed).expect("Compression failed");
        compressed.truncate(compressed_size);

        let mut decompressed = vec![0u8; data.len()];
        decompress(&compressed, &mut decompressed).expect("Decompression failed");

        assert_eq!(data, &decompressed[..]);
    }
}
