//! Separable Gaussian blur on a grayscale buffer (edge-clamped).
//! Used to smooth the threshold-crossing staircase that low-res JPEG input
//! produces: the stroke edge wiggles once per source pixel after upscaling, and
//! a symmetric pre-threshold blur removes that wiggle WITHOUT shifting edges
//! inward (unlike smoothing the traced contour, which shrinks convex curves).

pub fn gaussian(img: &[u8], w: usize, h: usize, sigma: f64) -> Vec<u8> {
    if sigma <= 0.05 {
        return img.to_vec();
    }
    let radius = (sigma * 3.0).ceil() as isize;
    let mut kernel = vec![0f64; (2 * radius + 1) as usize];
    let mut sum = 0.0;
    for (i, d) in (-radius..=radius).enumerate() {
        let v = (-(d as f64) * (d as f64) / (2.0 * sigma * sigma)).exp();
        kernel[i] = v;
        sum += v;
    }
    for k in kernel.iter_mut() {
        *k /= sum;
    }

    let clamp = |v: isize, hi: usize| -> usize { v.max(0).min(hi as isize - 1) as usize };

    // horizontal pass
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (i, d) in (-radius..=radius).enumerate() {
                acc += kernel[i] * img[y * w + clamp(x as isize + d, w)] as f64;
            }
            tmp[y * w + x] = acc as f32;
        }
    }
    // vertical pass
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (i, d) in (-radius..=radius).enumerate() {
                acc += kernel[i] * tmp[clamp(y as isize + d, h) * w + x] as f64;
            }
            out[y * w + x] = acc.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}
