//! Minimal binary PGM (P5) read/write -- used by the CLI and tests so the whole
//! crate stays dependency-free. JPEG decoding is the host app's job.

pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

/// Parse a binary P5 PGM (8-bit). Tolerates comments (# ...) in the header.
pub fn read_p5(bytes: &[u8]) -> Result<Gray, String> {
    let mut pos = 0usize;
    let mut tok = || -> Option<String> {
        // skip whitespace and comment lines
        loop {
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'#' {
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
            } else {
                break;
            }
        }
        let start = pos;
        while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos > start {
            Some(String::from_utf8_lossy(&bytes[start..pos]).to_string())
        } else {
            None
        }
    };
    let magic = tok().ok_or("empty")?;
    if magic != "P5" {
        return Err(format!("not P5: {magic}"));
    }
    let w: usize = tok().ok_or("no width")?.parse().map_err(|_| "bad width")?;
    let h: usize = tok()
        .ok_or("no height")?
        .parse()
        .map_err(|_| "bad height")?;
    let maxv: usize = tok()
        .ok_or("no maxval")?
        .parse()
        .map_err(|_| "bad maxval")?;
    if maxv != 255 {
        return Err("only maxval 255 supported".into());
    }
    if w == 0 || h == 0 {
        return Err("image dimensions must be non-zero".into());
    }
    if pos >= bytes.len() || !bytes[pos].is_ascii_whitespace() {
        return Err("missing raster separator".into());
    }
    pos += 1; // single whitespace after maxval, then raster
    let need = w.checked_mul(h).ok_or("image dimensions overflow")?;
    let end = pos.checked_add(need).ok_or("raster length overflow")?;
    if bytes.len() < end {
        return Err("truncated raster".into());
    }
    Ok(Gray {
        w,
        h,
        px: bytes[pos..end].to_vec(),
    })
}

pub fn write_p5(g: &Gray) -> Vec<u8> {
    let mut out = format!("P5\n{} {}\n255\n", g.w, g.h).into_bytes();
    out.extend_from_slice(&g.px);
    out
}
