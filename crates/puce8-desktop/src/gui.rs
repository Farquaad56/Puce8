//! Fenetre eframe/egui (E23a2) : ecran, cadence, barre d'etat.

use crate::app;
use eframe::egui;
use puce8_core::nes::Nes;
use std::time::Instant;

/// Etat de l'application de bureau.
pub struct Puce8App {
    nes: Option<Nes>,
    rom_name: String,
    scale: u32,
    acc: f64,
    last: Instant,
    texture: Option<egui::TextureHandle>,
    /// Images emulees depuis le debut de la seconde en cours, et moyenne de la seconde precedente.
    fps_count: u32,
    fps_start: Instant,
    fps: u32,
    paused: bool,
}

impl Puce8App {
    pub fn new(nes: Option<Nes>, rom_name: String, scale: u32) -> Self {
        Puce8App {
            nes,
            rom_name,
            scale,
            acc: 0.0,
            last: Instant::now(),
            texture: None,
            fps_count: 0,
            fps_start: Instant::now(),
            fps: 0,
            paused: false,
        }
    }

    /// Joue les images dues (accumulateur) ; renvoie vrai si au moins une image a ete produite.
    fn step(&mut self, turbo: bool) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        let Some(nes) = self.nes.as_mut() else {
            return false;
        };
        if self.paused {
            self.acc = 0.0;
            return false;
        }
        let n = app::frames_to_run(&mut self.acc, dt, turbo);
        for _ in 0..n {
            nes.run_frame();
        }
        self.fps_count += n;
        if self.fps_start.elapsed().as_secs_f64() >= 1.0 {
            self.fps = self.fps_count;
            self.fps_count = 0;
            self.fps_start = Instant::now();
        }
        n > 0
    }

    /// Met a jour la texture de l'ecran (seulement si une nouvelle image a ete produite).
    fn update_texture(&mut self, ctx: &egui::Context, new_frame: bool) {
        let Some(nes) = self.nes.as_ref() else {
            return;
        };
        if self.texture.is_some() && !new_frame {
            return;
        }
        let bytes = app::rgba_bytes(nes.bus.ppu.frame_buffer());
        let img = egui::ColorImage::from_rgba_unmultiplied([256, 240], &bytes);
        match self.texture.as_mut() {
            Some(t) => t.set(img, egui::TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture("ecran", img, egui::TextureOptions::NEAREST))
            }
        }
    }
}

impl eframe::App for Puce8App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let turbo = ctx.input(|i| i.key_down(egui::Key::Tab));
        let new_frame = self.step(turbo);
        self.update_texture(&ctx, new_frame);

        egui::Panel::bottom("etat").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.rom_name);
                ui.separator();
                ui.label(format!("{} images/s", self.fps));
                if self.paused {
                    ui.separator();
                    ui.label("PAUSE");
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| match &self.texture {
                Some(t) => {
                    let s = self.scale as f32;
                    ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(256.0 * s, 240.0 * s)));
                }
                None => {
                    ui.label("Glissez une ROM ici");
                }
            });
        });
        ctx.request_repaint();
    }
}

/// Ouvre la fenetre principale (bloquant jusqu'a la fermeture).
pub fn run(nes: Option<Nes>, rom_name: String, scale: u32) -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Puce8")
            .with_inner_size([768.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Puce8",
        options,
        Box::new(move |_cc| Ok(Box::new(Puce8App::new(nes, rom_name, scale)))),
    )
}
