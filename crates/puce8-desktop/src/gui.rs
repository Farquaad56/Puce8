//! Fenetre eframe/egui : ecran, cadence, barre d'etat (E23a2) ; menus, raccourcis,
//! ouverture de ROM et glisser-deposer (E23a3).

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
    /// E23a3 : fenetre "Ouvrir..." (champ texte du chemin) et derniere erreur de chargement.
    open_dialog: bool,
    open_path: String,
    error: Option<String>,
    /// E23a3 : cases du menu Debogage (les fenetres arrivent en E23b).
    pub show_tile_viewer: bool,
    pub show_tilemap_viewer: bool,
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
            open_dialog: false,
            open_path: String::new(),
            error: None,
            show_tile_viewer: false,
            show_tilemap_viewer: false,
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

    /// Charge une ROM (menu Ouvrir ou glisser-deposer) ; en cas d'erreur, la garde pour l'afficher.
    fn open_rom(&mut self, path: &str) {
        match app::load_rom(path) {
            Ok(nes) => {
                self.nes = Some(nes);
                self.rom_name = app::rom_label(path);
                self.texture = None;
                self.paused = false;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Raccourcis clavier (E23a3) : Echap = quitter, F5 = reset, P = pause.
    /// Ignores si un champ texte egui a le focus (on tape un chemin).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (esc, f5, p) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::P),
            )
        });
        if esc {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if f5 {
            if let Some(nes) = self.nes.as_mut() {
                nes.reset();
            }
        }
        if p {
            self.paused = !self.paused;
        }
    }

    /// Fichiers deposes sur la fenetre : on ouvre le premier.
    fn dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(f) = dropped.first() {
            let path = f.path().to_string_lossy().into_owned();
            self.open_rom(&path);
        }
    }

    /// Barre de menus (E23a3).
    fn menus(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Fichier", |ui| {
                if ui.button("Ouvrir...").clicked() {
                    self.open_dialog = true;
                    ui.close();
                }
                if ui.button("Quitter (Echap)").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Emulation", |ui| {
                let label = if self.paused {
                    "Reprise (P)"
                } else {
                    "Pause (P)"
                };
                if ui.button(label).clicked() {
                    self.paused = !self.paused;
                    ui.close();
                }
                if ui.button("Reset (F5)").clicked() {
                    if let Some(nes) = self.nes.as_mut() {
                        nes.reset();
                    }
                    ui.close();
                }
            });
            ui.menu_button("Affichage", |ui| {
                for s in 1..=5u32 {
                    ui.radio_value(&mut self.scale, s, format!("Echelle x{s}"));
                }
            });
            ui.menu_button("Debogage", |ui| {
                ui.checkbox(&mut self.show_tile_viewer, "Tile Viewer (F1)");
                ui.checkbox(&mut self.show_tilemap_viewer, "Tilemap Viewer (F2)");
            });
        });
    }

    /// Fenetre "Ouvrir..." : chemin tape a la main (pas de dependance de dialogue natif).
    fn open_window(&mut self, ctx: &egui::Context) {
        if !self.open_dialog {
            return;
        }
        let mut open = true;
        let mut valider = false;
        egui::Window::new("Ouvrir une ROM")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Chemin du fichier .nes :");
                let r = ui.text_edit_singleline(&mut self.open_path);
                let entree = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Ouvrir").clicked() || entree {
                    valider = true;
                }
                if let Some(e) = &self.error {
                    ui.colored_label(egui::Color32::RED, e);
                }
            });
        if valider {
            let path = self.open_path.trim().to_string();
            self.open_rom(&path);
            if self.error.is_none() {
                open = false;
            }
        }
        self.open_dialog = open;
    }
}

impl eframe::App for Puce8App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.dropped_files(&ctx);
        let turbo = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(egui::Key::Tab));
        let new_frame = self.step(turbo);
        self.update_texture(&ctx, new_frame);

        egui::Panel::top("menus").show(ui, |ui| self.menus(ui));
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
        self.open_window(&ctx);
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
