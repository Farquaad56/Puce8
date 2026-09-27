//! Fenetres egui de debogage (E23b2 : Tile Viewer ; E23b3 : Tilemap Viewer).
//! Le rendu des vues existe dans `puce8_core::debug` (E18e) : ici seulement les fenetres,
//! les controles et les surimpressions (`overlay.rs`).

use crate::overlay;
use eframe::egui;
use puce8_core::debug;
use puce8_core::nes::Nes;
use puce8_core::ppu::palette::to_rgba;

/// Indices palette (u16) -> couleurs 0x00RRGGBB.
fn to_rgb(px: &[u16]) -> Vec<u32> {
    px.iter().map(|&p| to_rgba(p)).collect()
}

/// Cree ou met a jour une texture NEAREST a partir d'un tampon 0x00RRGGBB.
fn set_texture(
    ctx: &egui::Context,
    slot: &mut Option<egui::TextureHandle>,
    name: &str,
    buf: &[u32],
    w: usize,
    h: usize,
) {
    let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &overlay::to_bytes(buf));
    match slot {
        Some(t) => t.set(img, egui::TextureOptions::NEAREST),
        None => *slot = Some(ctx.load_texture(name, img, egui::TextureOptions::NEAREST)),
    }
}

// ---------------------------------------------------------------- E23b2 : Tile Viewer

/// Tile Viewer : les 2 tables de motifs (256 x 128).
pub struct TileViewer {
    pub open: bool,
    /// None = gris ; Some(0-7) = palette du jeu.
    palette: Option<u8>,
    grid: bool,
    zoom: f32,
    texture: Option<egui::TextureHandle>,
}

impl Default for TileViewer {
    fn default() -> Self {
        TileViewer {
            open: false,
            palette: None,
            grid: false,
            zoom: 3.0,
            texture: None,
        }
    }
}

impl TileViewer {
    /// Recalcule l'image (appele une fois par image emulee, ou en pause). Fermee = aucun calcul.
    pub fn refresh(&mut self, ctx: &egui::Context, nes: &Nes) {
        if !self.open {
            return;
        }
        let (w, h) = (debug::PATTERNS_W, debug::PATTERNS_H);
        let pal = match self.palette {
            None => debug::ViewPalette::Gray,
            Some(p) => debug::ViewPalette::Index(p),
        };
        let mut px = vec![0u16; w * h];
        debug::render_patterns(&nes.bus.ppu, nes.bus.mapper.as_ref(), pal, &mut px);
        let mut buf = to_rgb(&px);
        if self.grid {
            overlay::draw_grid(&mut buf, w, h, 8, 0xFFFFFF, 96);
        }
        set_texture(ctx, &mut self.texture, "tile_viewer", &buf, w, h);
    }

    /// Fenetre flottante (menu Debogage, F1, ou croix).
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;
        egui::Window::new("Tile Viewer")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let texte = match self.palette {
                        None => "Gris".to_string(),
                        Some(p) => format!("Palette {p}"),
                    };
                    egui::ComboBox::from_label("Palette")
                        .selected_text(texte)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.palette, None, "Gris");
                            for p in 0..8u8 {
                                ui.selectable_value(
                                    &mut self.palette,
                                    Some(p),
                                    format!("Palette {p}"),
                                );
                            }
                        });
                    ui.checkbox(&mut self.grid, "Grille 8 px");
                    ui.add(egui::Slider::new(&mut self.zoom, 1.0..=4.0).text("zoom"));
                });
                if let Some(t) = &self.texture {
                    let (w, h) = (debug::PATTERNS_W, debug::PATTERNS_H);
                    let size = egui::vec2(w as f32 * self.zoom, h as f32 * self.zoom);
                    let r = ui.add(
                        egui::Image::new(t)
                            .fit_to_exact_size(size)
                            .sense(egui::Sense::hover()),
                    );
                    if let Some(pos) = r.hover_pos() {
                        let o = r.rect.min;
                        if let Some((x, y)) =
                            overlay::pixel_from_hover((pos.x, pos.y), (o.x, o.y), self.zoom, w, h)
                        {
                            let table = x / 128;
                            let tuile = (y / 8) * 16 + (x % 128) / 8;
                            let addr = table * 0x1000 + tuile * 16;
                            r.on_hover_text(format!("tuile ${tuile:02X} @ ${addr:04X}"));
                        }
                    }
                }
            });
        self.open = open;
    }
}

// ---------------------------------------------------------------- E23b3 : Tilemap Viewer

/// Tilemap Viewer : les 4 nametables (512 x 480).
pub struct TilemapViewer {
    pub open: bool,
    grid_tiles: bool,
    grid_attr: bool,
    scroll_frame: bool,
    separators: bool,
    zoom: f32,
    texture: Option<egui::TextureHandle>,
    header: String,
}

impl Default for TilemapViewer {
    fn default() -> Self {
        TilemapViewer {
            open: false,
            grid_tiles: false,
            grid_attr: false,
            scroll_frame: true,
            separators: true,
            zoom: 1.0,
            texture: None,
            header: String::new(),
        }
    }
}

impl TilemapViewer {
    /// Recalcule l'image et l'en-tete (une fois par image emulee, ou en pause). Fermee = aucun calcul.
    pub fn refresh(&mut self, ctx: &egui::Context, nes: &Nes) {
        if !self.open {
            return;
        }
        let (w, h) = (debug::NAMETABLES_W, debug::NAMETABLES_H);
        let (ppu, mapper) = (&nes.bus.ppu, nes.bus.mapper.as_ref());
        let mut px = vec![0u16; w * h];
        debug::render_nametables(ppu, mapper, &mut px);
        let mut buf = to_rgb(&px);
        if self.grid_tiles {
            overlay::draw_grid(&mut buf, w, h, 8, 0xFFFFFF, 96);
        }
        if self.grid_attr {
            overlay::draw_grid(&mut buf, w, h, 16, 0xFF0000, 128);
        }
        if self.separators {
            for y in 0..h {
                buf[y * w + 256] = 0xFFFFFF;
            }
            for x in 0..w {
                buf[240 * w + x] = 0xFFFFFF;
            }
        }
        let (sx, sy) = debug::scroll_origin(ppu);
        if self.scroll_frame {
            let pos = (usize::from(sx), usize::from(sy));
            overlay::draw_rect_wrap(&mut buf, (w, h), pos, (256, 240), 0xFFFF00);
        }
        let chr = if ppu.ctrl() & 0x10 != 0 { 1 } else { 0 };
        self.header = format!(
            "{:?} - CHR fond ${chr}000 - scroll ({sx}, {sy})",
            mapper.mirroring()
        );
        set_texture(ctx, &mut self.texture, "tilemap_viewer", &buf, w, h);
    }

    /// Fenetre flottante (menu Debogage, F2, ou croix).
    pub fn show(&mut self, ctx: &egui::Context, nes: &Nes) {
        let mut open = self.open;
        egui::Window::new("Tilemap Viewer")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(&self.header);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.grid_tiles, "Grille tuiles (8 px)");
                    ui.checkbox(&mut self.grid_attr, "Grille attributs (16 px)");
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.scroll_frame, "Cadre de scroll");
                    ui.checkbox(&mut self.separators, "Separateurs de nametables");
                    ui.add(egui::Slider::new(&mut self.zoom, 1.0..=4.0).text("zoom"));
                });
                if let Some(t) = &self.texture {
                    let (w, h) = (debug::NAMETABLES_W, debug::NAMETABLES_H);
                    let size = egui::vec2(w as f32 * self.zoom, h as f32 * self.zoom);
                    egui::ScrollArea::both().show(ui, |ui| {
                        let r = ui.add(
                            egui::Image::new(t)
                                .fit_to_exact_size(size)
                                .sense(egui::Sense::hover()),
                        );
                        if let Some(pos) = r.hover_pos() {
                            let o = r.rect.min;
                            if let Some((x, y)) = overlay::pixel_from_hover(
                                (pos.x, pos.y),
                                (o.x, o.y),
                                self.zoom,
                                w,
                                h,
                            ) {
                                let (ppu, mapper) = (&nes.bus.ppu, nes.bus.mapper.as_ref());
                                let i = debug::tile_at(ppu, mapper, x as u16, y as u16);
                                r.on_hover_text(format!(
                                    "NT ${:04X} tuile ${:02X} pal {}",
                                    i.nt_addr, i.tile, i.palette
                                ));
                            }
                        }
                    });
                }
            });
        self.open = open;
    }
}
