//! Geometry helpers + Schneider's least-squares cubic-Bezier fitting.
//! ("An Algorithm for Automatically Fitting Digitized Curves", Graphics Gems 1990.)
//! Direct port of the validated Python reference.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub fn new(x: f64, y: f64) -> Pt {
        Pt { x, y }
    }
    pub fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
    pub fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
    pub fn scale(self, s: f64) -> Pt {
        Pt::new(self.x * s, self.y * s)
    }
    pub fn dot(self, o: Pt) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn norm(self) -> Pt {
        let n = self.len();
        if n > 1e-12 {
            self.scale(1.0 / n)
        } else {
            self
        }
    }
}

/// A cubic Bezier as 4 control points.
pub type Cubic = [Pt; 4];

// --- closed-polyline helpers ------------------------------------------------

/// Resample a closed polyline to ~uniform arc-length spacing.
pub fn resample_closed(pts: &[Pt], step: f64) -> Vec<Pt> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0);
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        cum.push(cum[i] + b.sub(a).len());
    }
    let total = cum[n];
    if total < 1e-6 {
        return pts.to_vec();
    }
    let count = ((total / step).round() as usize).max(8);
    let mut out = Vec::with_capacity(count);
    let mut seg = 0usize;
    for k in 0..count {
        let s = total * (k as f64) / (count as f64);
        while seg < n && cum[seg + 1] < s {
            seg += 1;
        }
        let seg_i = seg.min(n - 1);
        let denom = cum[seg_i + 1] - cum[seg_i];
        let t = if denom > 1e-12 {
            (s - cum[seg_i]) / denom
        } else {
            0.0
        };
        let a = pts[seg_i];
        let b = pts[(seg_i + 1) % n];
        out.push(a.add(b.sub(a).scale(t)));
    }
    out
}

/// Gaussian smoothing of a closed polyline (wrap-around), matching scipy
/// gaussian_filter1d with mode='wrap' and truncate=4.0.
pub fn smooth_closed(pts: &[Pt], sigma: f64) -> Vec<Pt> {
    let n = pts.len();
    if sigma <= 0.0 || n < 3 {
        return pts.to_vec();
    }
    let radius = (sigma * 4.0).ceil() as isize;
    let mut kernel = Vec::with_capacity((2 * radius + 1) as usize);
    let mut sum = 0.0;
    for d in -radius..=radius {
        let w = (-(d as f64) * (d as f64) / (2.0 * sigma * sigma)).exp();
        kernel.push(w);
        sum += w;
    }
    for w in kernel.iter_mut() {
        *w /= sum;
    }
    let ni = n as isize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = 0.0;
        let mut y = 0.0;
        for (ki, d) in (-radius..=radius).enumerate() {
            let idx = (((i as isize + d) % ni) + ni) % ni; // wrap
            let p = pts[idx as usize];
            x += p.x * kernel[ki];
            y += p.y * kernel[ki];
        }
        out.push(Pt::new(x, y));
    }
    out
}

/// Turning angle (degrees) at each vertex over +/-k neighbours (closed).
pub fn turn_angles(pts: &[Pt], k: usize) -> Vec<f64> {
    let n = pts.len();
    let mut out = vec![0.0; n];
    if n < 2 * k + 1 {
        return out;
    }
    let ni = n as isize;
    for i in 0..n {
        let prev = pts[((i as isize - k as isize) % ni + ni) as usize % n];
        let next = pts[(i + k) % n];
        let a = pts[i].sub(prev);
        let b = next.sub(pts[i]);
        let denom = (a.len() * b.len()).max(1e-9);
        let cos = (a.dot(b) / denom).clamp(-1.0, 1.0);
        out[i] = cos.acos().to_degrees();
    }
    out
}

// --- Schneider fitting ------------------------------------------------------

fn q(ctrl: &Cubic, t: f64) -> Pt {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    Pt::new(
        a * ctrl[0].x + b * ctrl[1].x + c * ctrl[2].x + d * ctrl[3].x,
        a * ctrl[0].y + b * ctrl[1].y + c * ctrl[2].y + d * ctrl[3].y,
    )
}

fn q_deriv1(ctrl: &Cubic, t: f64) -> Pt {
    let d0 = ctrl[1].sub(ctrl[0]).scale(3.0);
    let d1 = ctrl[2].sub(ctrl[1]).scale(3.0);
    let d2 = ctrl[3].sub(ctrl[2]).scale(3.0);
    let mt = 1.0 - t;
    d0.scale(mt * mt).add(d1.scale(2.0 * mt * t)).add(d2.scale(t * t))
}

fn q_deriv2(ctrl: &Cubic, t: f64) -> Pt {
    let d0 = ctrl[2].sub(ctrl[1].scale(2.0)).add(ctrl[0]).scale(6.0);
    let d1 = ctrl[3].sub(ctrl[2].scale(2.0)).add(ctrl[1]).scale(6.0);
    d0.scale(1.0 - t).add(d1.scale(t))
}

fn chord_param(pts: &[Pt]) -> Vec<f64> {
    let n = pts.len();
    let mut u = vec![0.0; n];
    for i in 1..n {
        u[i] = u[i - 1] + pts[i].sub(pts[i - 1]).len();
    }
    let total = u[n - 1];
    if total < 1e-9 {
        for i in 0..n {
            u[i] = i as f64 / (n - 1) as f64;
        }
    } else {
        for v in u.iter_mut() {
            *v /= total;
        }
    }
    u
}

fn generate_bezier(pts: &[Pt], u: &[f64], t_left: Pt, t_right: Pt) -> Cubic {
    let p0 = pts[0];
    let p1 = *pts.last().unwrap();
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..pts.len() {
        let t = u[i];
        let mt = 1.0 - t;
        let b0 = mt * mt * mt;
        let b1 = 3.0 * mt * mt * t;
        let b2 = 3.0 * mt * t * t;
        let b3 = t * t * t;
        let a0 = t_left.scale(b1);
        let a1 = t_right.scale(b2);
        let tmp = pts[i].sub(p0.scale(b0 + b1).add(p1.scale(b2 + b3)));
        c00 += a0.dot(a0);
        c01 += a0.dot(a1);
        c11 += a1.dot(a1);
        x0 += a0.dot(tmp);
        x1 += a1.dot(tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = p1.sub(p0).len();
    let (mut al, mut ar);
    if det.abs() < 1e-12 {
        al = seg / 3.0;
        ar = seg / 3.0;
    } else {
        al = (x0 * c11 - x1 * c01) / det;
        ar = (c00 * x1 - c01 * x0) / det;
    }
    if al < 1e-6 * seg || ar < 1e-6 * seg {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    [p0, p0.add(t_left.scale(al)), p1.add(t_right.scale(ar)), p1]
}

fn reparameterize(pts: &[Pt], ctrl: &Cubic, u: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; u.len()];
    for i in 0..u.len() {
        let qu = q(ctrl, u[i]);
        let q1 = q_deriv1(ctrl, u[i]);
        let q2 = q_deriv2(ctrl, u[i]);
        let diff = qu.sub(pts[i]);
        let num = diff.dot(q1);
        let den = q1.dot(q1) + diff.dot(q2);
        out[i] = if den.abs() < 1e-12 {
            u[i]
        } else {
            (u[i] - num / den).clamp(0.0, 1.0)
        };
    }
    out
}

fn max_error(pts: &[Pt], ctrl: &Cubic, u: &[f64]) -> (f64, usize) {
    let mut max = 0.0;
    let mut idx = 0;
    for i in 0..pts.len() {
        let d = q(ctrl, u[i]).sub(pts[i]);
        let e = d.dot(d);
        if e > max {
            max = e;
            idx = i;
        }
    }
    (max, idx)
}

fn fit_cubic(pts: &[Pt], t_left: Pt, t_right: Pt, max_err2: f64, out: &mut Vec<Cubic>, depth: u32) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    if n == 2 {
        let d = pts[1].sub(pts[0]).len() / 3.0;
        out.push([pts[0], pts[0].add(t_left.scale(d)), pts[1].add(t_right.scale(d)), pts[1]]);
        return;
    }
    let mut u = chord_param(pts);
    let mut ctrl = generate_bezier(pts, &u, t_left, t_right);
    let (mut err2, mut split) = max_error(pts, &ctrl, &u);
    if err2 < max_err2 {
        out.push(ctrl);
        return;
    }
    if err2 < max_err2 * 16.0 && depth < 24 {
        for _ in 0..6 {
            u = reparameterize(pts, &ctrl, &u);
            ctrl = generate_bezier(pts, &u, t_left, t_right);
            let (e, s) = max_error(pts, &ctrl, &u);
            err2 = e;
            split = s;
            if err2 < max_err2 {
                out.push(ctrl);
                return;
            }
        }
    }
    if depth >= 24 {
        out.push(ctrl);
        return;
    }
    let split = split.clamp(1, n - 2);
    let center = pts[split - 1].sub(pts[split + 1]).norm();
    fit_cubic(&pts[..=split], t_left, center, max_err2, out, depth + 1);
    fit_cubic(&pts[split..], center.scale(-1.0), t_right, max_err2, out, depth + 1);
}

/// Fit an OPEN polyline with a chain of cubic beziers.
pub fn fit_open(pts: &[Pt], fit_err: f64) -> Vec<Cubic> {
    let mut out = Vec::new();
    if pts.len() < 2 {
        return out;
    }
    let t_left = pts[1].sub(pts[0]).norm();
    let n = pts.len();
    let t_right = pts[n - 2].sub(pts[n - 1]).norm();
    fit_cubic(pts, t_left, t_right, fit_err * fit_err, &mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_square_preserves_perimeter() {
        let sq = [Pt::new(0.0, 0.0), Pt::new(10.0, 0.0), Pt::new(10.0, 10.0), Pt::new(0.0, 10.0)];
        let rs = resample_closed(&sq, 1.0);
        let n = rs.len();
        let mut per = 0.0;
        for i in 0..n {
            per += rs[(i + 1) % n].sub(rs[i]).len();
        }
        assert!((per - 40.0).abs() < 0.5, "perimeter {per}");
    }

    #[test]
    fn fits_a_known_cubic_tightly() {
        // sample a smooth cubic, fit it, expect ~1 segment within tolerance
        let ctrl = [Pt::new(0.0, 0.0), Pt::new(30.0, 90.0), Pt::new(70.0, -40.0), Pt::new(100.0, 50.0)];
        let mut pts = Vec::new();
        for i in 0..=40 {
            let t = i as f64 / 40.0;
            pts.push(q(&ctrl, t));
        }
        let cubics = fit_open(&pts, 1.0);
        assert!(!cubics.is_empty(), "no segments produced");
        // densely sample the fitted chain, then check every original point is
        // close to it (fit tolerance was 1px, so expect well under 1.5px)
        let mut curve = Vec::new();
        for c in &cubics {
            for i in 0..=60 {
                curve.push(q(c, i as f64 / 60.0));
            }
        }
        let mut maxd: f64 = 0.0;
        for p in &pts {
            let mut best = f64::MAX;
            for on in &curve {
                best = best.min(p.sub(*on).len());
            }
            maxd = maxd.max(best);
        }
        assert!(maxd < 1.5, "max deviation {maxd}");
    }
}
