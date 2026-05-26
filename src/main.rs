//! CLI: read a binary PGM signature, write an SVG. Dependency-free.
//!
//!   sigtrace input.pgm output.svg [--thresh N] [--width N] [--fill #RRGGBB]
//!
//! (The library API `trace_signature_gray` is what an app links against; this
//! binary exists for testing/benchmarking against the Python reference.)

use sigtrace::{svg_document, trace_signature_gray, Options};
use std::fs;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: sigtrace input.pgm output.svg [--thresh N] [--width N] [--fill #RRGGBB]");
        exit(2);
    }
    let input = &args[1];
    let output = &args[2];
    let mut opt = Options::default();
    let mut fill = String::from("#111111");
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--thresh" => {
                opt.thresh = args[i + 1].parse().expect("bad --thresh");
                i += 2;
            }
            "--width" => {
                opt.target_w = args[i + 1].parse().expect("bad --width");
                i += 2;
            }
            "--fill" => {
                fill = args[i + 1].clone();
                i += 2;
            }
            "--smooth" => {
                opt.smooth = args[i + 1].parse().expect("bad --smooth");
                i += 2;
            }
            "--fit-err" => {
                opt.fit_err = args[i + 1].parse().expect("bad --fit-err");
                i += 2;
            }
            "--despeckle" => {
                opt.despeckle_k = args[i + 1].parse().expect("bad --despeckle");
                i += 2;
            }
            "--no-auto-contrast" => {
                opt.auto_contrast = false;
                i += 1;
            }
            "--blur-k" => {
                opt.blur_k = args[i + 1].parse().expect("bad --blur-k");
                i += 2;
            }
            other => {
                eprintln!("unknown arg: {other}");
                exit(2);
            }
        }
    }

    let bytes = fs::read(input).unwrap_or_else(|e| {
        eprintln!("read {input}: {e}");
        exit(1);
    });
    let g = sigtrace::pgm::read_p5(&bytes).unwrap_or_else(|e| {
        eprintln!("parse PGM: {e}");
        exit(1);
    });
    let res = trace_signature_gray(&g.px, g.w, g.h, &opt);
    let svg = svg_document(&res, &fill);
    fs::write(output, svg).unwrap_or_else(|e| {
        eprintln!("write {output}: {e}");
        exit(1);
    });
    eprintln!(
        "traced {}x{} -> {} ({} subpaths, {} bytes path)",
        g.w, g.h, output, res.subpaths, res.path_d.len()
    );
}
