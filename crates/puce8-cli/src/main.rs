mod capture;
mod harness;
mod input_script;
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

/// Chemin de sortie d'une option : doit commencer par out/ (REGLES par. 0).
fn out_path(args: &[String], pos: usize, flag: &str) -> String {
    let p = parse_next(args, pos);
    if !p.starts_with("out/") {
        die(2, &format!("{} path must start with out/ : {}", flag, p));
    }
    p
}

/// Dumps de debogage (E18e4) : table des motifs 256x128, nametables 512x480.
fn dump_views(
    nes: &puce8_core::nes::Nes,
    patterns: Option<String>,
    nametables: Option<String>,
    pal: puce8_core::debug::ViewPalette,
) {
    use puce8_core::debug;
    let ppu = &nes.bus.ppu;
    let mapper = nes.bus.mapper.as_ref();
    if let Some(p) = patterns {
        let mut img = vec![0u16; debug::PATTERNS_W * debug::PATTERNS_H];
        debug::render_patterns(ppu, mapper, pal, &mut img);
        capture::write_png(&p, debug::PATTERNS_W, debug::PATTERNS_H, &img)
            .unwrap_or_else(|e| die(2, &e));
    }
    if let Some(p) = nametables {
        let mut img = vec![0u16; debug::NAMETABLES_W * debug::NAMETABLES_H];
        debug::render_nametables(ppu, mapper, &mut img);
        capture::write_png(&p, debug::NAMETABLES_W, debug::NAMETABLES_H, &img)
            .unwrap_or_else(|e| die(2, &e));
    }
}

/// Dump texte de l'OAM (E21c2) : 64 lignes "NN: Y=.. T=.. A=.. X=.." en hexadecimal.
fn dump_oam(nes: &puce8_core::nes::Nes, path: &str) {
    let oam = &nes.bus.ppu.oam;
    let mut txt = String::new();
    for n in 0..64 {
        let s = &oam[n * 4..n * 4 + 4];
        txt.push_str(&format!(
            "{:02}: Y={:02X} T={:02X} A={:02X} X={:02X}\n",
            n, s[0], s[1], s[2], s[3]
        ));
    }
    fs::write(path, txt).unwrap_or_else(|e| die(2, &format!("cannot write {}: {}", path, e)));
}

fn do_run(args: &[String]) {
    // Parse args after "run <rom>"
    let rom = &args[2];
    let bytes = fs::read(rom).unwrap_or_else(|e| die(2, &format!("cannot read {}: {}", rom, e)));
    let _cart = puce8_core::cartridge::Cartridge::from_bytes(&bytes)
        .unwrap_or_else(|e| die(2, &format!("bad ROM: {:?}", e)));

    let rest = &args[3..];
    let mut frames: Option<u32> = None;
    let mut script: input_script::InputScript = Vec::new(); // E24b3 : --input
    let mut trace_path: Option<String> = None;
    let mut screenshot_path: Option<String> = None;
    let mut print_hash = false;
    let mut trace_from_cycle: u64 = 0;
    let mut trace_max_lines: usize = 100_000;
    let mut dump_patterns: Option<String> = None;
    let mut dump_nametables: Option<String> = None;
    let mut dump_oam_path: Option<String> = None;
    let mut patterns_palette = puce8_core::debug::ViewPalette::Gray;

    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--input" => {
                script = input_script::parse_input_script(&parse_next(rest, i))
                    .unwrap_or_else(|e| die(2, &format!("bad --input: {e}")));
                i += 1;
            }
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
            "--screenshot" => {
                let p = parse_next(rest, i);
                if !p.starts_with("out/") {
                    die(2, &format!("screenshot path must start with out/ : {}", p));
                }
                screenshot_path = Some(p);
                i += 1;
            }
            "--dump-patterns" => {
                dump_patterns = Some(out_path(rest, i, "--dump-patterns"));
                i += 1;
            }
            "--dump-oam" => {
                dump_oam_path = Some(out_path(rest, i, "--dump-oam"));
                i += 1;
            }
            "--dump-nametables" => {
                dump_nametables = Some(out_path(rest, i, "--dump-nametables"));
                i += 1;
            }
            "--patterns-palette" => {
                let p = parse_next(rest, i);
                patterns_palette = match p.as_str() {
                    "gris" => puce8_core::debug::ViewPalette::Gray,
                    n => match n.parse::<u8>() {
                        Ok(k) if k <= 7 => puce8_core::debug::ViewPalette::Index(k),
                        _ => die(2, "bad --patterns-palette (0-7 ou gris)"),
                    },
                };
                i += 1;
            }
            "--print-hash" => {
                print_hash = true;
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
    if trace_path.is_some() && screenshot_path.is_some() {
        die(2, "--trace and --screenshot are incompatible");
    }
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
        input_script::run_frames(&mut nes, frames, &script); // E24b3 : script d'entrees
    }

    let hash = Some(format!("{:x}", puce8_core::util::fnv1a64(&bytes)));
    let frame_hash = if print_hash {
        Some(format!(
            "{:x}",
            puce8_core::util::frame_hash(&nes.bus.ppu.framebuffer)
        ))
    } else {
        None
    };
    if let Some(ref sp) = screenshot_path {
        capture::write_screenshot(sp, &nes.bus.ppu.framebuffer).unwrap_or_else(|e| die(2, &e));
    }
    dump_views(&nes, dump_patterns, dump_nametables, patterns_palette);
    if let Some(p) = dump_oam_path {
        dump_oam(&nes, &p);
    }
    let r = harness::Resultat {
        rom: rom.to_string(),
        protocole: "run".to_string(),
        resultat: "REUSSI".to_string(),
        code: 0,
        texte: "".to_string(),
        frames,
        cycles: nes.bus.cpu_cycles,
        hash,
        frame_hash,
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
