mod harness;
mod suite;

use std::env;
use std::fs;
use std::process;

fn die(code: i32, msg: &str) -> ! {
    eprintln!("error: {}", msg);
    process::exit(code);
}

fn parse_next(args: &[String], pos: usize) -> String {
    if pos + 1 >= args.len() {
        die(2, "missing value after flag");
    }
    args[pos + 1].clone()
}

fn do_info(args: &[String]) {
    let rom = &args[2];
    let bytes = fs::read(rom).unwrap_or_else(|e| die(2, &format!("cannot read {}: {}", rom, e)));
    let cart = puce8_core::cartridge::Cartridge::from_bytes(&bytes)
        .unwrap_or_else(|e| die(2, &format!("bad ROM: {:?}", e)));
    println!("{}", cart.summary());
}

fn do_run(args: &[String]) {
    // Parse args after "run <rom>"
    let rom = &args[2];
    let bytes = fs::read(rom).unwrap_or_else(|e| die(2, &format!("cannot read {}: {}", rom, e)));
    let _cart = puce8_core::cartridge::Cartridge::from_bytes(&bytes)
        .unwrap_or_else(|e| die(2, &format!("bad ROM: {:?}", e)));

    let rest = &args[3..];
    let mut frames: Option<u32> = None;
    let mut trace_path: Option<String> = None;
    let mut trace_from_cycle: u64 = 0;
    let mut trace_max_lines: usize = 100_000;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--frames" => {
                frames = Some(
                    parse_next(rest, i)
                        .parse()
                        .unwrap_or_else(|_| die(2, "bad --frames")),
                );
                i += 1;
            }
            "--trace" => {
                trace_path = Some(parse_next(rest, i));
                let p = trace_path.as_ref().unwrap();
                if !p.starts_with("out/") {
                    die(2, &format!("trace path must start with out/ : {}", p));
                }
                i += 1;
            }
            "--trace-from-cycle" => {
                trace_from_cycle = parse_next(rest, i)
                    .parse()
                    .unwrap_or_else(|_| die(2, "bad --trace-from-cycle"));
                i += 1;
            }
            "--trace-max-lines" => {
                trace_max_lines = parse_next(rest, i)
                    .parse()
                    .unwrap_or_else(|_| die(2, "bad --trace-max-lines"));
                i += 1;
            }
            _ => die(2, &format!("unknown flag: {}", rest[i])),
        }
        i += 1;
    }

    let frames = frames.unwrap_or(35); // default NES frames for a game run
    let mut nes = puce8_core::nes::Nes::from_rom(&bytes).expect("valid ROM");

    if let Some(ref tp) = trace_path {
        let mut lines: Vec<String> = Vec::new();
        loop {
            nes.run_frame();
            // flush remaining cycles in frame to reach next instruction boundary
            while !nes.cpu.at_instruction_boundary() {
                nes.tick();
            }
            let cyc = nes.bus.cpu_cycles;
            if cyc >= trace_from_cycle {
                use puce8_core::cpu::trace::{capture_nes, format_nestest};
                let state = capture_nes(&nes);
                lines.push(format_nestest(&state));
                if lines.len() >= trace_max_lines {
                    break;
                }
            }
        }
        fs::write(tp, lines.join("\n"))
            .unwrap_or_else(|e| die(2, &format!("cannot write {}: {}", tp, e)));
    } else {
        for _ in 0..frames {
            nes.run_frame();
        }
    }

    let hash = Some(format!("{:x}", puce8_core::util::fnv1a64(&bytes)));
    let r = harness::Resultat {
        rom: rom.to_string(),
        protocole: "run".to_string(),
        resultat: "REUSSI".to_string(),
        code: 0,
        texte: "".to_string(),
        frames,
        cycles: nes.bus.cpu_cycles,
        hash,
    };
    println!("{}", serde_json::to_string(&r).unwrap());
}

fn do_blargg(args: &[String]) {
    let rom = &args[2];
    let bytes = fs::read(rom).unwrap_or_else(|e| die(2, &format!("cannot read {}: {}", rom, e)));
    let _cart = puce8_core::cartridge::Cartridge::from_bytes(&bytes)
        .unwrap_or_else(|e| die(2, &format!("bad ROM: {:?}", e)));

    let rest = &args[3..];
    let mut max_frames: u32 = 6000;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--max-frames" => {
                max_frames = parse_next(rest, i)
                    .parse()
                    .unwrap_or_else(|_| die(2, "bad --max-frames"));
                i += 1;
            }
            _ => die(2, &format!("unknown flag: {}", rest[i])),
        }
        i += 1;
    }

    let mut nes = puce8_core::nes::Nes::from_rom(&bytes).expect("valid ROM");
    let r = harness::run_blargg6000(&mut nes, max_frames);
    println!("{}", serde_json::to_string(&r).unwrap());
    process::exit(r.code);
}

fn do_blarggf8(args: &[String]) {
    let rom = &args[2];
    let bytes = fs::read(rom).unwrap_or_else(|e| die(2, &format!("cannot read {}: {}", rom, e)));
    let _cart = puce8_core::cartridge::Cartridge::from_bytes(&bytes)
        .unwrap_or_else(|e| die(2, &format!("bad ROM: {:?}", e)));

    let rest = &args[3..];
    let mut frames: u32 = 60;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--frames" => {
                frames = parse_next(rest, i)
                    .parse()
                    .unwrap_or_else(|_| die(2, "bad --frames"));
                i += 1;
            }
            _ => die(2, &format!("unknown flag: {}", rest[i])),
        }
        i += 1;
    }

    let mut nes = puce8_core::nes::Nes::from_rom(&bytes).expect("valid ROM");
    // Adresse par defaut $F8 ; la suite utilise le champ `result_addr` du catalogue.
    let r = harness::run_blargg_f8(&mut nes, frames, 0xF8);
    println!("{}", serde_json::to_string(&r).unwrap());
    process::exit(r.code);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: puce8-cli <info|run|blargg|blarggf8> ...");
        process::exit(2);
    }
    match args[1].as_str() {
        "info" => do_info(&args),
        "run" => do_run(&args),
        "blargg" => do_blargg(&args),
        "blarggf8" => do_blarggf8(&args),
        "suite" => {
            let c = suite::do_suite(&args);
            process::exit(c);
        }
        _ => die(2, &format!("unknown command: {}", args[1])),
    }
}
