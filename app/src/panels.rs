//! Settings and Privacy windows, and the typography controls.

use crate::i18n::Tr;
use crate::style::{self, Palette};
use gl_core::{DisplayMode, Language, Settings, StealthStyle, Store, Theme, TitleBar, TurnStyle};

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(title).font(style::semibold(13.0)));
    ui.add_space(2.0);
}

/// Typography controls for reflowable text. Returns true if anything changed.
pub fn typography(ui: &mut egui::Ui, s: &mut Settings, t: &Tr, fonts: &[String]) -> bool {
    let mut ch = false;
    let ts = &mut s.text;
    egui::Grid::new("typo").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(t.font);
        egui::ComboBox::from_id_salt("font")
            .selected_text(&ts.font)
            .width(170.0)
            .height(320.0)
            .show_ui(ui, |ui| {
                for f in ["Literata", "Inter", "Noto Serif Thai", "Noto Sans Thai"] {
                    ch |= ui.selectable_value(&mut ts.font, f.to_string(), f).changed();
                }
                ui.separator();
                for f in fonts {
                    if ["Literata", "Inter", "Noto Serif Thai", "Noto Sans Thai", "MuseoModerno"].contains(&f.as_str()) {
                        continue;
                    }
                    ch |= ui.selectable_value(&mut ts.font, f.clone(), f).changed();
                }
            });
        ui.end_row();
        ui.label(t.size);
        ch |= ui.add(egui::Slider::new(&mut ts.size, 10.0..=34.0).step_by(0.5)).changed();
        ui.end_row();
        ui.label(t.line_height);
        ch |= ui.add(egui::Slider::new(&mut ts.line_height, 1.0..=2.4).step_by(0.05)).changed();
        ui.end_row();
        ui.label(t.paragraph_spacing);
        ch |= ui
            .add(egui::Slider::new(&mut ts.paragraph_spacing, 0.0..=2.0).step_by(0.1))
            .changed();
        ui.end_row();
        ui.label(t.margins);
        ch |= ui.add(egui::Slider::new(&mut ts.margin, 8.0..=96.0).step_by(2.0)).changed();
        ui.end_row();
    });
    ch |= ui.checkbox(&mut ts.justify, t.justify).changed();
    ch |= ui.checkbox(&mut ts.indent, t.indent).changed();
    ch |= ui.checkbox(&mut ts.join_wrapped_lines, t.join_lines).changed();
    ch
}

fn theme_chips(ui: &mut egui::Ui, s: &mut Settings, t: &Tr) -> bool {
    let mut ch = false;
    for row in Theme::ALL.chunks(4) {
        ui.horizontal(|ui| {
            for th in row {
                let p = style::palette_of(*th, &s.custom_theme);
                let selected = s.theme == *th;
                let (rect, r) = ui.allocate_exact_size(egui::vec2(96.0, 54.0), egui::Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 10.0, p.paper);
                if *th == Theme::Custom {
                    // a small swatch strip shows the tint
                    let strip = egui::Rect::from_min_size(rect.left_bottom() - egui::vec2(-8.0, 8.0), egui::vec2(rect.width() - 16.0, 3.0));
                    let c = s.custom_theme.tint;
                    painter.rect_filled(strip, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                }
                let stroke = if selected {
                    egui::Stroke::new(2.0, ui.visuals().text_color())
                } else if r.hovered() {
                    egui::Stroke::new(1.0, ui.visuals().text_color().gamma_multiply(0.5))
                } else {
                    egui::Stroke::new(1.0, p.line)
                };
                painter.rect_stroke(rect, 10.0, stroke, egui::StrokeKind::Inside);
                painter.text(
                    rect.center() - egui::vec2(0.0, 7.0),
                    egui::Align2::CENTER_CENTER,
                    "Aa",
                    style::serif(16.0),
                    p.ink,
                );
                painter.text(
                    rect.center() + egui::vec2(0.0, 13.0),
                    egui::Align2::CENTER_CENTER,
                    t.theme_names[th.index()],
                    egui::FontId::proportional(10.5),
                    p.muted,
                );
                if r.clicked() {
                    s.theme = *th;
                    ch = true;
                }
            }
        });
    }
    if s.theme == Theme::Custom {
        ui.add_space(4.0);
        egui::Grid::new("custom-theme").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(t.tint);
            ch |= ui.color_edit_button_srgb(&mut s.custom_theme.tint).changed();
            ui.end_row();
            ui.label(t.strength);
            let mut pct = s.custom_theme.strength * 100.0;
            if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").step_by(1.0)).changed() {
                s.custom_theme.strength = pct / 100.0;
                ch = true;
            }
            ui.end_row();
        });
    }
    ch
}

const SETTINGS_W: f32 = 440.0;

pub fn settings_window(ctx: &egui::Context, open: &mut bool, s: &mut Settings, t: &Tr, fonts: &mut Option<Vec<String>>) -> bool {
    let mut ch = false;
    let mut keep = *open;
    let max_h = (ctx.content_rect().height() - 120.0).max(240.0);
    egui::Window::new(t.settings)
        .open(&mut keep)
        .collapsible(false)
        .resizable(false)
        .min_width(SETTINGS_W)
        .max_width(SETTINGS_W)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(max_h)
                .min_scrolled_height(max_h.min(640.0))
                .show(ui, |ui| {
                    ui.set_width(SETTINGS_W - 18.0);
                    section(ui, t.theme);
                    ch |= theme_chips(ui, s, t);

                    section(ui, t.reading);
                    egui::Grid::new("reading").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(t.page_turn);
                        ui.horizontal(|ui| {
                            for (v, l) in [(TurnStyle::Curl, t.curl), (TurnStyle::Slide, t.slide), (TurnStyle::None, t.none)] {
                                ch |= ui.selectable_value(&mut s.turn_style, v, l).changed();
                            }
                        });
                        ui.end_row();
                        ui.label(t.display);
                        ui.horizontal(|ui| {
                            for (v, l) in [
                                (DisplayMode::Single, t.single),
                                (DisplayMode::Spread, t.spread),
                                (DisplayMode::Auto, t.auto),
                            ] {
                                ch |= ui.selectable_value(&mut s.display, v, l).changed();
                            }
                        });
                        ui.end_row();
                        ui.label(t.sound);
                        ui.horizontal(|ui| {
                            ch |= ui.checkbox(&mut s.sound, "").changed();
                            ch |= ui
                                .add_enabled(s.sound, egui::Slider::new(&mut s.volume, 0.0..=1.0).show_value(false))
                                .changed();
                        });
                        ui.end_row();
                    });
                    ch |= ui.checkbox(&mut s.pdf_dark_invert, t.pdf_dark).changed();
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(t.defaults_new_books).small());
                    ui.horizontal(|ui| {
                        ch |= ui.checkbox(&mut s.default_first_blank, t.first_blank).changed();
                        ch |= ui.checkbox(&mut s.default_rtl, t.rtl).changed();
                    });

                    section(ui, t.typography);
                    let list = fonts.get_or_insert_with(gl_formats::engine::font_families).clone();
                    ch |= typography(ui, s, t, &list);

                    section(ui, t.stealth);
                    let st = &mut s.stealth;
                    ch |= ui.checkbox(&mut st.enabled, t.stealth_enabled).changed();
                    ui.add_enabled_ui(st.enabled, |ui| {
                        egui::Grid::new("stealth").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                            ui.label(t.stealth_style);
                            ui.horizontal(|ui| {
                                for (v, l) in [
                                    (StealthStyle::Veil, t.veil),
                                    (StealthStyle::Decoy, t.decoy),
                                    (StealthStyle::Minimize, t.minimize),
                                ] {
                                    ch |= ui.selectable_value(&mut st.style, v, l).changed();
                                }
                            });
                            ui.end_row();
                            ui.label(t.hide_delay);
                            ch |= ui
                                .add(egui::Slider::new(&mut st.hide_delay_ms, 0..=600).suffix(" ms").step_by(50.0))
                                .changed();
                            ui.end_row();
                        });
                        ch |= ui.checkbox(&mut st.reveal_on_click, t.reveal_click).changed();
                        ch |= ui.checkbox(&mut st.on_focus_loss, t.on_focus_loss).changed();
                        ch |= ui.checkbox(&mut st.neutral_title, t.neutral_title).changed();
                    });
                    ch |= ui.checkbox(&mut st.boss_key, t.boss_key).changed();

                    section(ui, t.window);
                    ui.horizontal(|ui| {
                        ui.label(t.title_bar);
                        for (v, l) in [
                            (TitleBar::Theme, t.tb_theme),
                            (TitleBar::System, t.tb_system),
                            (TitleBar::Hidden, t.tb_hidden),
                        ] {
                            ch |= ui.selectable_value(&mut s.title_bar, v, l).changed();
                        }
                    });
                    ch |= ui.checkbox(&mut s.always_on_top, t.always_on_top).changed();

                    section(ui, t.language);
                    ui.horizontal(|ui| {
                        for (v, l) in [
                            (Language::Auto, t.lang_auto),
                            (Language::English, "English"),
                            (Language::Thai, "ไทย"),
                        ] {
                            ch |= ui.selectable_value(&mut s.language, v, l).changed();
                        }
                    });
                    ui.add_space(8.0);
                });
        });
    *open = keep;
    ch
}

pub enum PrivacyAction {
    None,
    ClearCovers,
    ForgetBooks,
    WipeAll,
    SettingsChanged,
}

fn human(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1048576.0)
    }
}

pub fn privacy_window(
    ctx: &egui::Context,
    open: &mut bool,
    store: &Store,
    s: &mut Settings,
    t: &Tr,
    pal: Palette,
    confirm: &mut bool,
) -> PrivacyAction {
    let mut act = PrivacyAction::None;
    let mut keep = *open;
    egui::Window::new(t.privacy)
        .open(&mut keep)
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(t.privacy_intro);
            ui.add_space(6.0);
            egui::Frame::new().fill(pal.bg).corner_radius(10).inner_margin(12).show(ui, |ui| {
                ui.label(egui::RichText::new(t.privacy_network).color(pal.ink));
            });
            section(ui, t.data_folder);
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(store.root().display().to_string()).monospace().size(12.0)).truncate());
                if ui.button(t.open_folder).clicked() {
                    let _ = opener::open(store.root());
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    t.portable,
                    if store.is_portable() { t.portable_on } else { t.portable_off }
                ))
                .small()
                .color(pal.muted),
            );
            section(ui, t.stored_files);
            let files = store.list_files();
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                if files.is_empty() {
                    ui.label(egui::RichText::new(t.nothing_stored).color(pal.muted));
                }
                egui::Grid::new("files").num_columns(2).striped(true).show(ui, |ui| {
                    for f in &files {
                        let rel = f.path.strip_prefix(store.root()).unwrap_or(&f.path).display().to_string();
                        ui.label(egui::RichText::new(rel).monospace().size(12.0));
                        ui.label(egui::RichText::new(human(f.bytes)).color(pal.muted));
                        ui.end_row();
                    }
                });
            });
            ui.add_space(8.0);
            if ui.checkbox(&mut s.save_covers, t.save_covers).changed() {
                act = PrivacyAction::SettingsChanged;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(t.clear_covers).clicked() {
                    act = PrivacyAction::ClearCovers;
                }
                if ui.button(t.forget_books).clicked() {
                    act = PrivacyAction::ForgetBooks;
                }
                if *confirm {
                    if ui
                        .button(egui::RichText::new(t.confirm).color(egui::Color32::from_rgb(0xC0, 0x39, 0x2B)))
                        .clicked()
                    {
                        act = PrivacyAction::WipeAll;
                        *confirm = false;
                    }
                    if ui.button(t.cancel).clicked() {
                        *confirm = false;
                    }
                } else if ui.button(t.wipe_all).clicked() {
                    *confirm = true;
                }
            });
        });
    *open = keep;
    act
}
