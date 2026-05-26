//! CLI: read a binary PGM signature, write an SVG. Dependency-free.
//!
//!   sigtrace input.pgm output.svg [--thresh N] [--width N] [--fill #RRGGBB]
//!
//! (The library API `trace_signature_gray` is what an app links against; this
//! binary exists for testing/benchmarking against the Python reference.)

use sigtrace::{svg_document, trace_signature_gray_checked, Options};
use std::fs;
use std::process::exit;
use std::str::FromStr;

fn usage() -> ! {
    eprintln!("usage: sigtrace input.pgm output.svg [--thresh N] [--width N] [--fill #RRGGBB]");
    exit(2);
}

fn value<'a>(args: &'a [String], i: usize, flag: &str) -> &'a str {
    args.get(i + 1).map(String::as_str).unwrap_or_else(|| {
        eprintln!("missing value for {flag}");
        usage();
    })
}

fn parse_value<T: FromStr>(args: &[String], i: usize, flag: &str) -> T {
    value(args, i, flag).parse().unwrap_or_else(|_| {
        eprintln!("bad value for {flag}");
        usage();
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        usage();
    }
    let input = &args[1];
    let output = &args[2];
    let mut opt = Options::default();
    let mut fill = String::from("#111111");
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--thresh" => {
                opt.thresh = parse_value(&args, i, "--thresh");
                i += 2;
            }
            "--width" => {
                opt.target_w = parse_value(&args, i, "--width");
                i += 2;
            }
            "--fill" => {
                fill = value(&args, i, "--fill").to_string();
                i += 2;
            }
            "--smooth" => {
                opt.smooth = parse_value(&args, i, "--smooth");
                i += 2;
            }
            "--fit-err" => {
                opt.fit_err = parse_value(&args, i, "--fit-err");
                i += 2;
            }
            "--despeckle" => {
                opt.despeckle_k = parse_value(&args, i, "--despeckle");
                i += 2;
            }
            "--no-auto-contrast" => {
                opt.auto_contrast = false;
                i += 1;
            }
            "--blur-k" => {
                opt.blur_k = parse_value(&args, i, "--blur-k");
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
    let res = trace_signature_gray_checked(&g.px, g.w, g.h, &opt).unwrap_or_else(|e| {
        eprintln!("trace: {e}");
        exit(1);
    });
    let svg = svg_document(&res, &fill);
    fs::write(output, svg).unwrap_or_else(|e| {
        eprintln!("write {output}: {e}");
        exit(1);
    });
    eprintln!(
        "traced {}x{} -> {} ({} subpaths, {} bytes path)",
        g.w,
        g.h,
        output,
        res.subpaths,
        res.path_d.len()
    );
}
