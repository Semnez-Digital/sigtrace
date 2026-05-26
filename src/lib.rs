//! sigtrace -- a signature-specialised bitmap->vector tracer for low-quality
//! eID JPEGs. Zero dependencies; pure std Rust so it builds for macOS, iOS,
//! Android, Linux and Windows.
//!
//! Pipeline (deliberately minimal -- validated to beat fancier variants on a
//! corpus of degraded modern signatures):
//!   bicubic upscale -> fixed threshold -> despeckle -> pixel-edge contour
//!   -> resample/smooth -> Schneider cubic-bezier fit -> SVG path.

pub mod blur;
pub mod contour;
pub mod fit;
pub mod pgm;
pub mod resize;

use fit::Pt;

#[derive(Clone, Copy)]
pub struct Options {
    pub target_w: usize,   // working-resolution width
    pub thresh: u8,        // ink = gray < thresh
    pub step: f64,         // contour resample spacing (working px)
    pub smooth: f64,       // contour smoothing sigma
    pub fit_err: f64,      // bezier fit tolerance (working px)
    pub min_len: f64,      // drop loops with smaller perimeter
    pub corner_k: usize,   // neighbour offset for seam selection
    pub despeckle_k: f64,  // drop components < despeckle_k * scale^2 px
    pub auto_contrast: bool, // rescue light/low-contrast inputs (see preprocess)
    pub dark_ink_max: u8,  // if the darkest ~2% of pixels is lighter than this,
                           // there is no real dark ink -> contrast-stretch first
    pub blur_k: f64,       // pre-threshold Gaussian blur sigma = blur_k * upscale
                           // factor; removes the source-pixel "staircase" tremor
}

impl Default for Options {
    fn default() -> Self {
        Options {
            target_w: 3000, // higher working res => finer threshold-crossing => smoother thin strokes
            thresh: 145,
            step: 1.5,
            smooth: 1.5,    // more contour smoothing tames thin-stroke wobble
            fit_err: 2.0,
            min_len: 24.0,
            corner_k: 4,
            despeckle_k: 4.0,
            auto_contrast: true,
            dark_ink_max: 130,
            blur_k: 0.45, // sigma ~0.45*s smooths the threshold staircase
        }
    }
}

pub struct TraceResult {
    pub path_d: String,
    pub width: usize,
    pub height: usize,
    pub subpaths: usize,
}

/// Upscale + (optional) contrast rescue + threshold + despeckle.
/// Returns (ink mask 0/255, nw, nh).
fn preprocess(gray: &[u8], w: usize, h: usize, opt: &Options) -> (Vec<u8>, usize, usize) {
    let s = ((opt.target_w as f64 / w as f64).round() as usize).max(1);
    let (nw, nh) = (w * s, h * s);
    let mut up = resize::bicubic_gray(gray, w, h, nw, nh);
    if opt.auto_contrast {
        rescue_contrast(&mut up, opt.dark_ink_max);
    }
    if opt.blur_k > 0.0 {
        up = blur::gaussian(&up, nw, nh, opt.blur_k * s as f64);
    }
    let mut ink: Vec<u8> = up.iter().map(|&v| if v < opt.thresh { 255 } else { 0 }).collect();
    despeckle(&mut ink, nw, nh, (opt.despeckle_k * (s * s) as f64) as usize);
    (ink, nw, nh)
}

fn percentile(hist: &[u32; 256], total: u32, frac: f64) -> u8 {
    let target = (frac * total as f64) as u32;
    let mut cum = 0u32;
    for (v, &c) in hist.iter().enumerate() {
        cum += c;
        if cum >= target {
            return v as u8;
        }
    }
    255
}

/// Normal eID ink is dark-on-white, so the fixed threshold is left alone (best on
/// the corpus). But a faint/low-contrast input (e.g. faded ink) would have NO dark
/// pixels and trace to nothing. So: if the darkest ~2% of pixels is still lighter
/// than `dark_ink_max`, there is no real dark ink -> linearly stretch [p2,p98] to
/// [0,255] so the strokes become dark and threshold normally. The guard means
/// full-contrast inputs are untouched (no regression).
fn rescue_contrast(img: &mut [u8], dark_ink_max: u8) {
    let mut hist = [0u32; 256];
    for &v in img.iter() {
        hist[v as usize] += 1;
    }
    let total = img.len() as u32;
    let lo = percentile(&hist, total, 0.02);
    if lo <= dark_ink_max {
        return; // genuine dark ink present -> don't touch it
    }
    let hi = percentile(&hist, total, 0.98);
    if hi <= lo {
        return; // degenerate / blank
    }
    let scale = 255.0 / (hi as f32 - lo as f32);
    for v in img.iter_mut() {
        *v = (((*v as f32 - lo as f32) * scale).round()).clamp(0.0, 255.0) as u8;
    }
}

/// Remove 8-connected ink components smaller than `min_size` pixels.
fn despeckle(ink: &mut [u8], w: usize, h: usize, min_size: usize) {
    let n = w * h;
    let mut visited = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut comp: Vec<usize> = Vec::new();
    for start in 0..n {
        if ink[start] == 0 || visited[start] {
            continue;
        }
        comp.clear();
        stack.push(start);
        visited[start] = true;
        while let Some(p) = stack.pop() {
            comp.push(p);
            let (x, y) = ((p % w) as isize, (p / w) as isize);
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                        continue;
                    }
                    let q = ny as usize * w + nx as usize;
                    if ink[q] != 0 && !visited[q] {
                        visited[q] = true;
                        stack.push(q);
                    }
                }
            }
        }
        if comp.len() < min_size {
            for &p in &comp {
                ink[p] = 0;
            }
        }
    }
}

fn perimeter(loop_pts: &[Pt]) -> f64 {
    let n = loop_pts.len();
    let mut s = 0.0;
    for i in 0..n {
        s += loop_pts[(i + 1) % n].sub(loop_pts[i]).len();
    }
    s
}

/// Full pipeline: grayscale (ink dark) -> SVG bezier path data.
pub fn trace_signature_gray(gray: &[u8], w: usize, h: usize, opt: &Options) -> TraceResult {
    let (ink, nw, nh) = preprocess(gray, w, h, opt);
    let loops = contour::trace_loops(&ink, nw, nh);

    let mut d = String::new();
    let mut subpaths = 0;
    for lp in &loops {
        if lp.len() < 4 || perimeter(lp) < opt.min_len {
            continue;
        }
        let rs = fit::resample_closed(lp, opt.step);
        let sm = fit::smooth_closed(&rs, opt.smooth);
        let n = sm.len();
        if n < 4 {
            continue;
        }
        // open the closed loop at the sharpest-curvature seam
        let ang = fit::turn_angles(&sm, opt.corner_k);
        let mut seam = 0usize;
        let mut best = -1.0;
        for (i, &a) in ang.iter().enumerate() {
            if a > best {
                best = a;
                seam = i;
            }
        }
        let mut seg: Vec<Pt> = Vec::with_capacity(n + 1);
        for k in 0..=n {
            seg.push(sm[(seam + k) % n]);
        }
        let cubics = fit::fit_open(&seg, opt.fit_err);
        if cubics.is_empty() {
            continue;
        }
        let p0 = cubics[0][0];
        d.push_str(&format!("M{:.2} {:.2}", p0.x, p0.y));
        for c in &cubics {
            d.push_str(&format!(
                "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                c[1].x, c[1].y, c[2].x, c[2].y, c[3].x, c[3].y
            ));
        }
        d.push('Z');
        subpaths += 1;
    }
    TraceResult { path_d: d, width: nw, height: nh, subpaths }
}

/// Wrap a trace result in a standalone SVG document.
pub fn svg_document(res: &TraceResult, fill: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
         viewBox=\"0 0 {} {}\"><path fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/></svg>",
        res.width, res.height, res.width, res.height, fill, res.path_d
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light_block_image(w: usize, h: usize, ink: u8) -> Vec<u8> {
        let mut img = vec![255u8; w * h]; // white paper
        for y in h / 3..2 * h / 3 {
            for x in w / 6..5 * w / 6 {
                img[y * w + x] = ink; // a faint stroke block
            }
        }
        img
    }

    #[test]
    fn auto_contrast_rescues_light_ink() {
        let (w, h) = (60usize, 60usize);
        let img = light_block_image(w, h, 200); // ink lighter than thresh(145)
        let mut opt = Options { target_w: 120, despeckle_k: 0.5, ..Options::default() };

        let rescued = trace_signature_gray(&img, w, h, &opt);
        assert!(rescued.subpaths >= 1, "auto-contrast should recover light ink");

        opt.auto_contrast = false;
        let missed = trace_signature_gray(&img, w, h, &opt);
        assert_eq!(missed.subpaths, 0, "without it, light ink (200 > thresh) is missed");
    }

    #[test]
    fn dark_ink_is_left_untouched() {
        // a normal dark-ink image must NOT be altered by the guard
        let (w, h) = (60usize, 60usize);
        let img = light_block_image(w, h, 20); // genuine dark ink
        let opt = Options { target_w: 120, despeckle_k: 0.5, ..Options::default() };
        let with = trace_signature_gray(&img, w, h, &opt);
        let without = trace_signature_gray(&img, w, h,
            &Options { auto_contrast: false, ..opt });
        assert_eq!(with.subpaths, without.subpaths, "dark ink path must be unchanged");
        assert!(with.subpaths >= 1);
    }
}
