//! Puce8 - frontend de bureau (E23).

use puce8_desktop::{app, gui};
use std::time::Instant;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = app::parse_args(&argv).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    if let Some(n) = args.headless_frames {
        headless(args.rom.as_deref(), n);
        return;
    }
    // E23a2 : fenetre eframe/egui.
    let nes = args.rom.as_deref().map(|p| {
        app::load_rom(p).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1);
        })
    });
    let rom_name = args.rom.clone().unwrap_or_default();
    if let Err(e) = gui::run(nes, rom_name, args.scale) {
        eprintln!("erreur fenetre : {e}");
        std::process::exit(1);
    }
}

/// Mode sans fenetre : N images, affichage des images/s, puis sortie (E23a1, utilise en E37b).
fn headless(rom: Option<&str>, frames: u32) {
    let Some(path) = rom else {
        eprintln!("--headless-frames demande une ROM");
        std::process::exit(1);
    };
    let mut nes = app::load_rom(path).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let debut = Instant::now();
    for _ in 0..frames {
        nes.run_frame();
    }
    let s = debut.elapsed().as_secs_f64();
    println!(
        "{frames} images en {s:.2} s : {:.0} images/s",
        f64::from(frames) / s
    );
}
