//! Pixel-EDGE boundary tracing.
//!
//! Instead of OpenCV-style pixel-centre contours (which inset ~0.5px and thin
//! strokes), we trace the boundary along the lattice *between* pixels. Every
//! foreground cell contributes a unit directed edge on each side that faces
//! background, oriented so the foreground stays on the right of travel. The
//! edges link head-to-tail into closed loops (outer boundaries and holes alike),
//! giving the same pixel-edge convention as potrace -- correct stroke width with
//! no edge-grow hack. Holes come out with opposite winding; render fill-rule
//! evenodd.

use crate::fit::Pt;
use std::collections::HashMap;

type V = (i32, i32);

fn cw(d: V) -> V {
    (-d.1, d.0)
}
fn ccw(d: V) -> V {
    (d.1, -d.0)
}
fn rev(d: V) -> V {
    (-d.0, -d.1)
}

/// Trace all boundary loops of the ink mask. `ink[y*w+x] != 0` => foreground.
/// Returns loops as collapsed integer corner points (collinear runs removed).
pub fn trace_loops(ink: &[u8], w: usize, h: usize) -> Vec<Vec<Pt>> {
    let fg = |x: i32, y: i32| -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < w
            && (y as usize) < h
            && ink[y as usize * w + x as usize] != 0
    };

    // build directed lattice edges (start -> end)
    let mut adj: HashMap<V, Vec<V>> = HashMap::new();
    let push = |a: V, b: V, m: &mut HashMap<V, Vec<V>>| {
        m.entry(a).or_default().push(b);
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !fg(x, y) {
                continue;
            }
            if !fg(x, y - 1) {
                push((x, y), (x + 1, y), &mut adj);
            } // top -> right
            if !fg(x + 1, y) {
                push((x + 1, y), (x + 1, y + 1), &mut adj);
            } // right -> down
            if !fg(x, y + 1) {
                push((x + 1, y + 1), (x, y + 1), &mut adj);
            } // bottom -> left
            if !fg(x - 1, y) {
                push((x, y + 1), (x, y), &mut adj);
            } // left -> up
        }
    }

    // deterministic-ish iteration: collect candidate start vertices
    let mut starts: Vec<V> = adj.keys().copied().collect();
    starts.sort();

    let mut loops: Vec<Vec<Pt>> = Vec::new();
    for &s0 in &starts {
        // consume any remaining edges that start at s0
        while let Some(first_end) = adj.get_mut(&s0).and_then(|v| v.pop()) {
            let mut path: Vec<V> = vec![s0, first_end];
            let mut din = (first_end.0 - s0.0, first_end.1 - s0.1);
            let mut cur = first_end;
            let start = s0;
            while cur != start {
                // priority: sharpest right turn first (keeps fg on the right)
                let prefs = [cw(din), din, ccw(din), rev(din)];
                let mut chosen: Option<V> = None;
                if let Some(vec) = adj.get_mut(&cur) {
                    for d in prefs {
                        let target = (cur.0 + d.0, cur.1 + d.1);
                        if let Some(pos) = vec.iter().position(|&e| e == target) {
                            vec.swap_remove(pos);
                            chosen = Some(target);
                            break;
                        }
                    }
                    if chosen.is_none() {
                        chosen = vec.pop();
                    }
                }
                let next = match chosen {
                    Some(n) => n,
                    None => break, // dangling (shouldn't happen for a closed boundary)
                };
                din = (next.0 - cur.0, next.1 - cur.1);
                cur = next;
                path.push(cur);
            }
            loops.push(collapse(&path));
        }
    }
    loops
}

/// Remove collinear interior points of a closed integer path; keep corners.
fn collapse(path: &[V]) -> Vec<Pt> {
    let n = path.len();
    if n < 4 {
        return path
            .iter()
            .map(|&(x, y)| Pt::new(x as f64, y as f64))
            .collect();
    }
    // path starts and ends at the same vertex (cur returned to start); drop dup
    let pts: Vec<V> = if path[0] == path[n - 1] {
        path[..n - 1].to_vec()
    } else {
        path.to_vec()
    };
    let m = pts.len();
    let mut out = Vec::with_capacity(m);
    for i in 0..m {
        let prev = pts[(i + m - 1) % m];
        let cur = pts[i];
        let next = pts[(i + 1) % m];
        let d0 = (cur.0 - prev.0, cur.1 - prev.1);
        let d1 = (next.0 - cur.0, next.1 - cur.1);
        if d0 != d1 {
            out.push(Pt::new(cur.0 as f64, cur.1 as f64)); // direction changed: corner
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask(w: usize, h: usize, fg: &[(usize, usize)]) -> Vec<u8> {
        let mut m = vec![0u8; w * h];
        for &(x, y) in fg {
            m[y * w + x] = 255;
        }
        m
    }

    #[test]
    fn solid_square_is_one_loop() {
        // 4x4 ink block inside an 8x8 frame
        let mut fg = Vec::new();
        for y in 2..6 {
            for x in 2..6 {
                fg.push((x, y));
            }
        }
        let loops = trace_loops(&mask(8, 8, &fg), 8, 8);
        assert_eq!(loops.len(), 1);
        // pixel-edge boundary => 4 corners at the block edges
        assert_eq!(loops[0].len(), 4);
        let xs: Vec<f64> = loops[0].iter().map(|p| p.x).collect();
        assert!(xs.iter().cloned().fold(f64::MAX, f64::min) == 2.0);
        assert!(xs.iter().cloned().fold(f64::MIN, f64::max) == 6.0);
    }

    #[test]
    fn square_with_hole_is_two_loops() {
        // 6x6 ink ring (outer 6x6 minus inner 2x2) => outer + hole boundary
        let mut fg = Vec::new();
        for y in 1..7 {
            for x in 1..7 {
                let hole = (3..5).contains(&x) && (3..5).contains(&y);
                if !hole {
                    fg.push((x, y));
                }
            }
        }
        let loops = trace_loops(&mask(8, 8, &fg), 8, 8);
        assert_eq!(loops.len(), 2);
    }
}
