//! Separable bicubic upscale for single-channel (grayscale) images.
//! Uses the a = -0.75 cubic-convolution kernel (same as OpenCV INTER_CUBIC) and
//! OpenCV's pixel-centre coordinate mapping, so it matches the Python reference.

fn cubic(d: f64) -> f64 {
    const A: f64 = -0.75;
    let d = d.abs();
    if d <= 1.0 {
        (A + 2.0) * d * d * d - (A + 3.0) * d * d + 1.0
    } else if d < 2.0 {
        A * d * d * d - 5.0 * A * d * d + 8.0 * A * d - 4.0 * A
    } else {
        0.0
    }
}

fn clampi(v: isize, lo: isize, hi: isize) -> isize {
    v.max(lo).min(hi)
}

/// One axis pass: resample `src` (len = old, stride between samples = `stride`,
/// `count` lines of length `old`) along that axis to `new` samples.
/// We just implement the full 2D resize directly for clarity.
pub fn bicubic_gray(src: &[u8], w: usize, h: usize, nw: usize, nh: usize) -> Vec<u8> {
    // horizontal pass: w -> nw, keep h rows (work in f32)
    let sx = w as f64 / nw as f64;
    let sy = h as f64 / nh as f64;

    // precompute horizontal taps
    let mut hx = vec![0isize; nw * 4];
    let mut hw = vec![0f64; nw * 4];
    for ox in 0..nw {
        let fx = (ox as f64 + 0.5) * sx - 0.5;
        let ix = fx.floor();
        let t = fx - ix;
        for k in 0..4 {
            let xx = ix as isize - 1 + k as isize;
            hx[ox * 4 + k] = clampi(xx, 0, w as isize - 1);
            hw[ox * 4 + k] = cubic(t - (k as f64 - 1.0));
        }
    }

    let mut tmp = vec![0f32; nw * h];
    for y in 0..h {
        let row = &src[y * w..y * w + w];
        for ox in 0..nw {
            let mut acc = 0.0;
            for k in 0..4 {
                acc += hw[ox * 4 + k] * row[hx[ox * 4 + k] as usize] as f64;
            }
            tmp[y * nw + ox] = acc as f32;
        }
    }

    // vertical pass: h -> nh
    let mut vy = vec![0isize; nh * 4];
    let mut vw = vec![0f64; nh * 4];
    for oy in 0..nh {
        let fy = (oy as f64 + 0.5) * sy - 0.5;
        let iy = fy.floor();
        let t = fy - iy;
        for k in 0..4 {
            let yy = iy as isize - 1 + k as isize;
            vy[oy * 4 + k] = clampi(yy, 0, h as isize - 1);
            vw[oy * 4 + k] = cubic(t - (k as f64 - 1.0));
        }
    }

    let mut out = vec![0u8; nw * nh];
    for oy in 0..nh {
        for ox in 0..nw {
            let mut acc = 0.0;
            for k in 0..4 {
                acc += vw[oy * 4 + k] * tmp[vy[oy * 4 + k] as usize * nw + ox] as f64;
            }
            out[oy * nw + ox] = acc.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}
