//! Fenetre eframe/egui : ecran, cadence, barre d'etat (E23a2) ; menus, raccourcis,
//! ouverture de ROM et glisser-deposer (E23a3) ; fenetres de debogage (E23b2/E23b3) ;
//! sauvegardes batterie .sav (E29a3) ; son et cadence audio (E34c2) ; correctif croix des
//! manettes + fenetre Manettes F3 (E24c) ; configuration des manettes, assistant F4 (E24d).

use crate::pad_config::{self, ConfigSet, PadConfig, RawSource};
use crate::{app, audio, input, sav, viewers};
use eframe::egui;
use puce8_core::nes::Nes;
use puce8_core::util::fnv1a64;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

/// Etat de l'application de bureau.
pub struct Puce8App {
    nes: Option<Nes>,
    rom_name: String,
    /// E29a3 : chemin de la ROM (le .sav est ecrit a cote).
    rom_path: Option<String>,
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
    /// E23b2/E23b3 : fenetres de debogage.
    tiles: viewers::TileViewer,
    tilemap: viewers::TilemapViewer,
    /// E24b2 : manettes (None si gilrs n'a pas pu demarrer) et message d'erreur associe.
    gilrs: Option<gilrs::Gilrs>,
    pad_error: Option<String>,
    /// E24c : etat de chaque manette reconstruit par ses evenements, fenetre de diagnostic (F3),
    /// derniers evenements recus et lignes affichees.
    pad_tracks: HashMap<gilrs::GamepadId, input::PadTrack>,
    pads_window: bool,
    pad_log: VecDeque<String>,
    pad_lines: Vec<String>,
    /// E24d : configurations des manettes (fichier `puce8-manettes.txt`) et assistant (F4).
    pad_configs: ConfigSet,
    config_window: bool,
    assistant: Option<Assistant>,
    config_msg: String,
    pad_list: Vec<(gilrs::GamepadId, String, String)>,
    /// E29a3 : hash FNV de la RAM de batterie au dernier chargement/ecriture, et minuteur (60 s).
    sav_hash: u64,
    sav_timer: Instant,
    /// E34c2 : sortie audio (None : sans son, cadence par l'accumulateur), case "Son", message.
    audio: Option<audio::Audio>,
    sound: bool,
    audio_msg: Option<String>,
}

impl Puce8App {
    pub fn new(nes: Option<Nes>, rom_path: Option<String>, scale: u32, audio_on: bool) -> Self {
        // E24b2 : un seul contexte gilrs ; en cas d'echec, clavier seul (pas de panique).
        let (gilrs, pad_error) = match gilrs::Gilrs::new() {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(format!("manettes indisponibles : {e}"))),
        };
        let rom_name = rom_path.as_deref().map(app::rom_label).unwrap_or_default();
        // E34c2 : echec de cpal -> sans son (message dans la barre d'etat), pas de panique.
        let audio = if audio_on { audio::Audio::new() } else { None };
        let audio_msg = if audio_on && audio.is_none() {
            Some("son indisponible".to_string())
        } else {
            None
        };
        let mut s = Puce8App {
            nes,
            rom_name,
            rom_path,
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
            tiles: viewers::TileViewer::default(),
            tilemap: viewers::TilemapViewer::default(),
            gilrs,
            pad_error,
            pad_tracks: HashMap::new(),
            pads_window: false,
            pad_log: VecDeque::new(),
            pad_lines: Vec::new(),
            pad_configs: ConfigSet::load(pad_config::CONFIG_FILE),
            config_window: false,
            assistant: None,
            config_msg: String::new(),
            pad_list: Vec::new(),
            sav_hash: 0,
            sav_timer: Instant::now(),
            audio,
            sound: true,
            audio_msg,
        };
        s.load_sav();
        s.prepare_audio();
        s
    }

    /// E34c2 : l'APU enregistre les echantillons, au taux du peripherique.
    fn prepare_audio(&mut self) {
        if let (Some(nes), Some(a)) = (self.nes.as_mut(), self.audio.as_ref()) {
            nes.bus.apu.record = true;
            nes.bus.apu.set_sample_rate(a.sample_rate);
        }
    }

    /// E29a3 : charge le .sav de la ROM (s'il existe et a la bonne taille).
    fn load_sav(&mut self) {
        let (Some(nes), Some(rom)) = (self.nes.as_mut(), self.rom_path.as_deref()) else {
            return;
        };
        let Some(taille) = nes.battery_ram().map(<[u8]>::len) else {
            return;
        };
        if let Some(data) = sav::load_sav(&sav::sav_path(rom), taille) {
            nes.load_battery_ram(&data);
        }
        self.sav_hash = nes.battery_ram().map_or(0, fnv1a64);
    }

    /// E29a3 : ecrit le .sav (atomique) si la RAM de batterie a change depuis la derniere fois.
    fn save_sav(&mut self) {
        let (Some(nes), Some(rom)) = (self.nes.as_ref(), self.rom_path.as_deref()) else {
            return;
        };
        let Some(ram) = nes.battery_ram() else {
            return;
        };
        let h = fnv1a64(ram);
        if h == self.sav_hash {
            return;
        }
        match sav::write_sav(&sav::sav_path(rom), ram) {
            Ok(()) => self.sav_hash = h,
            Err(e) => eprintln!("sauvegarde impossible : {e}"),
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
        let n = match &self.audio {
            // E34c2 : le tampon audio donne la cadence (environ 50 ms d'avance), sauf en turbo.
            Some(a) if !turbo => {
                let mut n = 0;
                let mut samples = Vec::new();
                while a.queued() < audio::cible(a.sample_rate) && n < app::MAX_CATCH_UP {
                    nes.run_frame();
                    nes.bus.apu.drain_samples(&mut samples);
                    a.push(&samples, self.sound);
                    samples.clear();
                    n += 1;
                }
                self.acc = 0.0;
                n
            }
            // Sans audio (ou turbo) : accumulateur d'E23a ; les echantillons eventuels sont jetes.
            _ => {
                let n = app::frames_to_run(&mut self.acc, dt, turbo);
                for _ in 0..n {
                    nes.run_frame();
                }
                nes.bus.apu.drain_samples(&mut Vec::new());
                n
            }
        };
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
                self.save_sav(); // E29a3 : sauvegarde du jeu precedent
                self.nes = Some(nes);
                self.rom_path = Some(path.to_string());
                self.load_sav();
                self.prepare_audio();
                self.rom_name = app::rom_label(path);
                self.texture = None;
                self.paused = false;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Entrees joueur (E24b2) : clavier (joueur 1) + manettes gilrs (1re -> joueur 1, 2e -> joueur 2).
    /// E24c : la croix est lue de 3 facons (bouton gilrs, evenements suivis, axes DPadX/DPadY),
    /// car la table de correspondance de gilrs ne lie pas toujours les 4 directions.
    fn update_inputs(&mut self, ctx: &egui::Context) {
        let clavier = if ctx.egui_wants_keyboard_input() {
            0 // un champ texte a le focus : on ne joue pas
        } else {
            ctx.input(|i| input::keyboard_buttons(|k| i.key_down(k)))
        };
        let mut pads = [0u8; 2];
        let mut lines = Vec::new();
        let mut connues = Vec::new(); // E24d : (id, cle, nom) des 2 premieres manettes
        if let Some(g) = self.gilrs.as_mut() {
            // Branchement / debranchement a chaud, et suivi de l'etat par les evenements.
            while let Some(ev) = g.next_event() {
                if self.pads_window {
                    self.pad_log
                        .push_back(format!("{:?} {:?}", ev.id, ev.event));
                    while self.pad_log.len() > 12 {
                        self.pad_log.pop_front();
                    }
                }
                // E24d : l'assistant retient la 1re source brute qui bouge sur la manette choisie.
                if let Some(a) = self.assistant.as_mut() {
                    if ev.id == a.pad && a.attente.is_none() && a.etape < 8 {
                        let src = match ev.event {
                            gilrs::EventType::ButtonPressed(_, code) => {
                                Some(RawSource::Button(code.into_u32()))
                            }
                            gilrs::EventType::AxisChanged(_, v, code) => {
                                pad_config::from_axis(code.into_u32(), v)
                            }
                            _ => None,
                        };
                        if let Some(s) = src {
                            a.cfg.sources[a.etape] = Some(s);
                            a.etape += 1;
                            a.attente = Some(s); // attendre le relachement avant l'etape suivante
                        }
                    }
                }
                match ev.event {
                    gilrs::EventType::ButtonPressed(b, _) => {
                        if let Some(p) = from_gilrs(b) {
                            self.pad_tracks
                                .entry(ev.id)
                                .or_default()
                                .set_button(p, true);
                        }
                    }
                    gilrs::EventType::ButtonReleased(b, _) => {
                        if let Some(p) = from_gilrs(b) {
                            self.pad_tracks
                                .entry(ev.id)
                                .or_default()
                                .set_button(p, false);
                        }
                    }
                    gilrs::EventType::AxisChanged(a, v, _) => {
                        if let Some(x) = axis_from_gilrs(a) {
                            self.pad_tracks.entry(ev.id).or_default().axes.set(x, v);
                        }
                    }
                    gilrs::EventType::Disconnected => {
                        self.pad_tracks.remove(&ev.id);
                    }
                    _ => {}
                }
            }
            let mut ids: Vec<gilrs::GamepadId> = g.gamepads().map(|(id, _)| id).collect();
            ids.sort_by_key(|id| usize::from(*id));
            for (n, (slot, id)) in pads.iter_mut().zip(ids).enumerate() {
                let gp = g.gamepad(id);
                // E24d : etat brut (codes du peripherique), pour la configuration et l'assistant.
                let st = gp.state();
                let bruts: HashMap<u32, bool> = st
                    .buttons()
                    .map(|(c, d)| (c.into_u32(), d.is_pressed()))
                    .collect();
                let axes_bruts: HashMap<u32, f32> =
                    st.axes().map(|(c, d)| (c.into_u32(), d.value())).collect();
                let appuye = |c: u32| bruts.get(&c).copied().unwrap_or(false);
                let valeur = |c: u32| axes_bruts.get(&c).copied().unwrap_or(0.0);
                if let Some(a) = self.assistant.as_mut() {
                    if a.pad == id && a.attente.is_some_and(|s| !s.active(&appuye, &valeur)) {
                        a.attente = None;
                    }
                }
                let cle = pad_config::pad_key(gp.uuid(), gp.name());
                connues.push((id, cle.clone(), gp.name().to_string()));
                let t = self.pad_tracks.get(&id).copied().unwrap_or_default();
                let axe = |a: gilrs::Axis, x: input::PadAxis| {
                    input::plus_fort(gp.value(a), t.axes.get(x))
                };
                let axes = input::PadAxes {
                    stick_x: axe(gilrs::Axis::LeftStickX, input::PadAxis::StickX),
                    stick_y: axe(gilrs::Axis::LeftStickY, input::PadAxis::StickY),
                    dpad_x: axe(gilrs::Axis::DPadX, input::PadAxis::DPadX),
                    dpad_y: axe(gilrs::Axis::DPadY, input::PadAxis::DPadY),
                };
                *slot = match self.pad_configs.get(&cle) {
                    // E24d : manette configuree -> codes bruts uniquement.
                    Some(cfg) => cfg.buttons(appuye, valeur),
                    None => {
                        input::pad_buttons(|b| gp.is_pressed(to_gilrs(b)) || t.is_pressed(b), axes)
                    }
                };
                if self.pads_window {
                    let conf = if self.pad_configs.get(&cle).is_some() {
                        "configuree (F4)"
                    } else {
                        "non configuree"
                    };
                    lines.push(format!(
                        "Joueur {} : {} ({:?}, {}) -> {}",
                        n + 1,
                        gp.name(),
                        gp.mapping_source(),
                        conf,
                        input::buttons_text(*slot)
                    ));
                    lines.push(format!(
                        "  stick ({:.2}, {:.2})  croix-axes ({:.2}, {:.2})",
                        axes.stick_x, axes.stick_y, axes.dpad_x, axes.dpad_y
                    ));
                }
            }
        }
        self.pad_list = connues;
        if self.pads_window {
            lines.push(format!("Clavier -> {}", input::buttons_text(clavier)));
            self.pad_lines = lines;
        }
        if let Some(nes) = self.nes.as_mut() {
            nes.set_buttons(0, input::sanitize(input::merge(clavier, pads[0])));
            nes.set_buttons(1, input::sanitize(pads[1]));
        }
    }

    /// E24c : fenetre "Manettes" (F3) : ce que Puce8 recoit de chaque manette.
    fn pads_window(&mut self, ctx: &egui::Context) {
        let mut open = self.pads_window;
        egui::Window::new("Manettes (F3)")
            .open(&mut open)
            .show(ctx, |ui| {
                if self.pad_lines.len() <= 1 {
                    ui.label("Aucune manette detectee.");
                }
                for l in &self.pad_lines {
                    ui.monospace(l);
                }
                ui.separator();
                ui.label("Derniers evenements gilrs :");
                for l in &self.pad_log {
                    ui.monospace(l);
                }
            });
        self.pads_window = open;
    }

    /// E24d : fenetre "Configurer les manettes" (F4) : assistant bouton par bouton.
    fn config_window(&mut self, ctx: &egui::Context) {
        let mut open = self.config_window;
        let mut action: Option<Action> = None;
        egui::Window::new("Configurer les manettes (F4)")
            .open(&mut open)
            .show(ctx, |ui| match &self.assistant {
                None => {
                    if self.pad_list.is_empty() {
                        ui.label("Aucune manette detectee.");
                    }
                    for (n, (id, cle, nom)) in self.pad_list.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let etat = if self.pad_configs.get(cle).is_some() {
                                "configuree"
                            } else {
                                "par defaut"
                            };
                            ui.label(format!("Joueur {} : {nom} ({etat})", n + 1));
                            if ui.button("Configurer").clicked() {
                                action = Some(Action::Commencer(*id, cle.clone(), nom.clone()));
                            }
                            if self.pad_configs.get(cle).is_some()
                                && ui.button("Par defaut").clicked()
                            {
                                action = Some(Action::Oublier(cle.clone()));
                            }
                        });
                    }
                    if !self.config_msg.is_empty() {
                        ui.label(&self.config_msg);
                    }
                }
                Some(a) if a.etape < 8 => {
                    ui.label(format!("Manette : {}", a.nom));
                    ui.heading(format!(
                        "Appuie sur : {}",
                        pad_config::NES_BUTTONS[a.etape].1
                    ));
                    if a.attente.is_some() {
                        ui.label("(relache la touche precedente)");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Passer").clicked() {
                            action = Some(Action::Passer);
                        }
                        if ui.button("Annuler").clicked() {
                            action = Some(Action::Annuler);
                        }
                    });
                }
                Some(a) => {
                    ui.label(format!("Manette : {} - configuration terminee.", a.nom));
                    ui.horizontal(|ui| {
                        if ui.button("Enregistrer").clicked() {
                            action = Some(Action::Enregistrer);
                        }
                        if ui.button("Annuler").clicked() {
                            action = Some(Action::Annuler);
                        }
                    });
                }
            });
        self.config_window = open;
        match action {
            Some(Action::Commencer(pad, cle, nom)) => {
                self.assistant = Some(Assistant {
                    pad,
                    cle,
                    nom,
                    etape: 0,
                    cfg: PadConfig::default(),
                    attente: None,
                });
            }
            Some(Action::Passer) => {
                if let Some(a) = self.assistant.as_mut() {
                    a.etape += 1;
                }
            }
            Some(Action::Annuler) => self.assistant = None,
            Some(Action::Enregistrer) => {
                if let Some(a) = self.assistant.take() {
                    self.pad_configs.set(&a.cle, &a.nom, a.cfg);
                    self.save_pad_configs();
                }
            }
            Some(Action::Oublier(cle)) => {
                self.pad_configs.pads.retain(|(k, _, _)| *k != cle);
                self.save_pad_configs();
            }
            None => {}
        }
        if !self.config_window {
            self.assistant = None; // fenetre fermee : assistant abandonne
        }
    }

    /// E24d : ecrit `puce8-manettes.txt` (message dans la fenetre).
    fn save_pad_configs(&mut self) {
        self.config_msg = match self.pad_configs.save(pad_config::CONFIG_FILE) {
            Ok(()) => format!("Enregistre dans {}", pad_config::CONFIG_FILE),
            Err(e) => format!("Ecriture impossible : {e}"),
        };
    }

    /// Raccourcis clavier (E23a3) : Echap = quitter, F5 = reset, P = pause.
    /// Ignores si un champ texte egui a le focus (on tape un chemin).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (esc, f5, p, f1, f2, f3, f4) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::F5),
                i.key_pressed(egui::Key::P),
                i.key_pressed(egui::Key::F1),
                i.key_pressed(egui::Key::F2),
                i.key_pressed(egui::Key::F3),
                i.key_pressed(egui::Key::F4),
            )
        });
        if f4 {
            self.config_window = !self.config_window; // E24d
        }
        if f3 {
            self.pads_window = !self.pads_window; // E24c
        }
        if f1 {
            self.tiles.open = !self.tiles.open;
        }
        if f2 {
            self.tilemap.open = !self.tilemap.open;
        }
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
                ui.checkbox(&mut self.sound, "Son"); // E34c2 : muet = zeros, meme cadence
            });
            ui.menu_button("Affichage", |ui| {
                for s in 1..=5u32 {
                    ui.radio_value(&mut self.scale, s, format!("Echelle x{s}"));
                }
            });
            ui.menu_button("Debogage", |ui| {
                ui.checkbox(&mut self.tiles.open, "Tile Viewer (F1)");
                ui.checkbox(&mut self.tilemap.open, "Tilemap Viewer (F2)");
                ui.checkbox(&mut self.pads_window, "Manettes (F3)"); // E24c
                ui.checkbox(&mut self.config_window, "Configurer les manettes (F4)");
                // E24d
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
        self.update_inputs(&ctx); // E24b2
        let turbo = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(egui::Key::Tab));
        let new_frame = self.step(turbo);
        if self.sav_timer.elapsed().as_secs() >= 60 {
            self.sav_timer = Instant::now();
            self.save_sav(); // E29a3 : toutes les 60 s, seulement si la RAM a change
        }
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
                if let Some(e) = &self.pad_error {
                    ui.separator();
                    ui.label(e);
                }
                if let Some(e) = &self.audio_msg {
                    ui.separator();
                    ui.label(e);
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
        self.pads_window(&ctx); // E24c
        self.config_window(&ctx); // E24d
        if let Some(nes) = self.nes.as_ref() {
            // Une fois par image emulee (et toujours en pause) ; fenetre fermee = aucun calcul.
            if new_frame || self.paused {
                self.tiles.refresh(&ctx, nes);
                self.tilemap.refresh(&ctx, nes);
            }
            self.tiles.show(&ctx);
            self.tilemap.show(&ctx, nes);
        }
        ctx.request_repaint();
    }
}

/// E29a3 : fermeture de la fenetre (Echap, croix, menu Quitter) -> derniere sauvegarde.
impl Drop for Puce8App {
    fn drop(&mut self) {
        self.save_sav();
    }
}

/// Bouton de manette independant de gilrs (input.rs) -> bouton gilrs.
fn to_gilrs(b: input::PadButton) -> gilrs::Button {
    use input::PadButton as P;
    match b {
        P::East => gilrs::Button::East,
        P::South => gilrs::Button::South,
        P::West => gilrs::Button::West,
        P::Select => gilrs::Button::Select,
        P::Start => gilrs::Button::Start,
        P::DPadUp => gilrs::Button::DPadUp,
        P::DPadDown => gilrs::Button::DPadDown,
        P::DPadLeft => gilrs::Button::DPadLeft,
        P::DPadRight => gilrs::Button::DPadRight,
    }
}

/// E24d : assistant de configuration d'une manette.
struct Assistant {
    pad: gilrs::GamepadId,
    cle: String,
    nom: String,
    /// Bouton NES attendu (0-7, ordre de `NES_BUTTONS`) ; 8 = termine.
    etape: usize,
    cfg: PadConfig,
    /// Source qui vient d'etre retenue : on attend son relachement avant l'etape suivante.
    attente: Option<RawSource>,
}

/// E24d : boutons de la fenetre de configuration.
enum Action {
    Commencer(gilrs::GamepadId, String, String),
    Passer,
    Annuler,
    Enregistrer,
    Oublier(String),
}

/// E24c : bouton gilrs -> bouton de manette de Puce8 (None : bouton inutile a la NES).
fn from_gilrs(b: gilrs::Button) -> Option<input::PadButton> {
    use input::PadButton as P;
    Some(match b {
        gilrs::Button::East => P::East,
        gilrs::Button::South => P::South,
        gilrs::Button::West => P::West,
        gilrs::Button::Select => P::Select,
        gilrs::Button::Start => P::Start,
        gilrs::Button::DPadUp => P::DPadUp,
        gilrs::Button::DPadDown => P::DPadDown,
        gilrs::Button::DPadLeft => P::DPadLeft,
        gilrs::Button::DPadRight => P::DPadRight,
        _ => return None,
    })
}

/// E24c : axe gilrs -> axe utile a la NES.
fn axis_from_gilrs(a: gilrs::Axis) -> Option<input::PadAxis> {
    Some(match a {
        gilrs::Axis::LeftStickX => input::PadAxis::StickX,
        gilrs::Axis::LeftStickY => input::PadAxis::StickY,
        gilrs::Axis::DPadX => input::PadAxis::DPadX,
        gilrs::Axis::DPadY => input::PadAxis::DPadY,
        _ => return None,
    })
}

/// Ouvre la fenetre principale (bloquant jusqu'a la fermeture).
pub fn run(
    nes: Option<Nes>,
    rom_path: Option<String>,
    scale: u32,
    audio_on: bool,
) -> eframe::Result {
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
        Box::new(move |_cc| Ok(Box::new(Puce8App::new(nes, rom_path, scale, audio_on)))),
    )
}
