/*
 * This file is part of Khipu.
 *
 * Copyright (C) 2026 Alex Palacios
 *
 * Khipu is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Khipu is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with Khipu.  If not, see <https://www.gnu.org/licenses/>.
 */

//! The window a form opens: its blanks asked for in Khipu's own design, and the text that results.
//!
//! Two callers share the fields and the preview. Espanso runs `Khipu.exe --form <trigger in hex>`
//! through the helper (see [`crate::yaml::form`]), and [`run`] is that whole process: a small
//! window of its own, no tray, no mutex, the result written to standard output for espanso to type.
//! The editor's «Probar» shows the same [`fields`] and [`result_preview`] in a dialog, without
//! espanso, so what the editor promises is what the window does.
//!
//! The invariant: **leaving without inserting gives the trigger back.** The typed trigger stays in
//! the text while the window is up, and espanso erases it once this process returns, whatever it
//! printed (measured in a browser textarea, 2026-10-09). Printing nothing would eat it; printing the
//! trigger leaves the text exactly as it was before the form opened.
//!
//! **Not here:** reading the form out of the file (`crate::yaml::form`), and anything that saves.
//! This window never writes `base.yml`.

use crate::app::{accent, secondary_text, win_background_for};
use crate::i18n::Lang;
use crate::settings::Settings;
use crate::ui::controls;
use crate::ui::studio::in_lang;
use crate::yaml::form::{pieces, FieldKind, FormSpec, Piece};
use std::sync::{Arc, Mutex};

/// The window's width in points. Wide enough for a sentence of label and a comfortable field;
/// the height follows the fields.
const WIDTH: f32 = 480.0;
/// Rows a long text field opens with.
const LONG_ROWS: usize = 3;
/// The tallest the preview grows before it scrolls.
const PREVIEW_MAX: f32 = 140.0;

/// What each field holds when the form opens, in the order of [`FormSpec::fields`].
pub fn initial_answers(spec: &FormSpec) -> Vec<String> {
    spec.fields.iter().map(|f| f.initial()).collect()
}

/// The text the form produces with these answers, and with what was last copied.
pub fn filled(spec: &FormSpec, answers: &[String], copied: &str) -> String {
    spec.filled(copied, |name| answer_of(spec, answers, name).to_string())
}

/// What the preview shows where what was last copied goes, when there is nothing real to show.
fn copied_label(lang: Lang) -> &'static str {
    in_lang(
        lang,
        "lo último que hayas copiado",
        "whatever you last copied",
        "ang huli mong kinopya",
        "जो आपने आखिरी बार कॉपी किया",
    )
}

fn answer_of<'a>(spec: &FormSpec, answers: &'a [String], name: &str) -> &'a str {
    spec.fields
        .iter()
        .position(|f| f.name == name)
        .and_then(|i| answers.get(i))
        .map_or("", String::as_str)
}

/// A field's name as its label: the first letter raised, the rest as the user wrote it.
fn label(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn field_id(salt: &str, index: usize) -> egui::Id {
    egui::Id::new((salt, "form-field", index))
}

/// One label and one control per field. Enter in a one-line field goes to the next text field, and
/// from the last one asks to insert; Ctrl+Enter asks to insert from anywhere. Returns whether
/// the user asked to insert.
pub fn fields(
    ui: &mut egui::Ui,
    salt: &str,
    spec: &FormSpec,
    answers: &mut [String],
    focus_first: bool,
) -> bool {
    let mut submit = ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter));
    let typed: Vec<usize> = spec
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind != FieldKind::Choice)
        .map(|(i, _)| i)
        .collect();
    // Enter hands the focus on a frame late. Handed on at once, the next field took it in the
    // same frame, read that same Enter as its own and let go again: no field focused, nothing
    // looking focused, and the window seemingly stuck.
    let pending = egui::Id::new((salt, "form-focus-next"));
    let next_focus = ui.data_mut(|d| d.remove_temp::<usize>(pending));
    if let Some(next) = next_focus {
        ui.memory_mut(|m| m.request_focus(field_id(salt, next)));
    } else if focus_first {
        if let Some(&first) = typed.first() {
            ui.memory_mut(|m| m.request_focus(field_id(salt, first)));
        }
    }
    for (i, field) in spec.fields.iter().enumerate() {
        if i > 0 {
            ui.add_space(controls::GAP_WIDE);
        }
        ui.label(controls::field_label(label(&field.name)));
        ui.add_space(controls::GAP_TIGHT);
        let answer = &mut answers[i];
        match field.kind {
            FieldKind::Short => {
                let response = ui.add(
                    controls::text_field(answer)
                        .id(field_id(salt, i))
                        .desired_width(f32::INFINITY),
                );
                if response.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter)) {
                    match typed.iter().find(|&&next| next > i) {
                        Some(&next) => {
                            ui.data_mut(|d| d.insert_temp(pending, next));
                            ui.ctx().request_repaint();
                        }
                        None => submit = true,
                    }
                }
            }
            FieldKind::Long => {
                ui.add(
                    controls::multiline_field(answer)
                        .id(field_id(salt, i))
                        .desired_rows(LONG_ROWS)
                        .desired_width(f32::INFINITY),
                );
            }
            FieldKind::Choice => choice(ui, salt, i, &field.options, answer),
        }
    }
    submit
}

/// A list's options: side by side as segments while they fit, a drop-down when they do not.
fn choice(ui: &mut egui::Ui, salt: &str, index: usize, options: &[String], answer: &mut String) {
    let options: Vec<&str> = options.iter().map(|o| o.trim()).filter(|o| !o.is_empty()).collect();
    if options.is_empty() {
        ui.label(controls::muted("—", !ui.visuals().dark_mode));
        return;
    }
    let selected = options.iter().position(|o| *o == answer.as_str());
    if options.len() <= 4 && controls::segmented_natural_width(ui, &options) <= ui.available_width() {
        if let Some(picked) = controls::segmented(ui, (salt, "form-choice", index), &options, selected) {
            *answer = options[picked].to_string();
        }
        return;
    }
    egui::ComboBox::from_id_salt((salt, "form-combo", index))
        .selected_text(answer.as_str())
        .truncate()
        .width(ui.available_width())
        .icon(controls::dropdown_chevron)
        .show_ui(ui, |ui| {
            for option in &options {
                if controls::dropdown_choice(ui, option, answer.as_str() == *option) {
                    *answer = option.to_string();
                }
            }
        });
}

/// The text as it will be inserted: what is fixed in the ordinary ink, each answer in the accent,
/// and a blank still empty shown by its name, faintly. What was last copied is in the accent too,
/// or, when `copied` is `None` (the editor's preview), said in words, faintly.
pub fn result_preview(
    ui: &mut egui::Ui,
    spec: &FormSpec,
    answers: &[String],
    copied: Option<&str>,
    lang: Lang,
) {
    controls::inset_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let job = result_job(ui, spec, answers, copied, lang, ui.available_width());
        egui::ScrollArea::vertical()
            .max_height(PREVIEW_MAX)
            .auto_shrink([false, true])
            .show(ui, |ui| ui.label(job));
    });
}

/// The text of [`result_preview`] without its box, `wrap` wide, for the editor's own preview.
pub fn result_job(
    ui: &egui::Ui,
    spec: &FormSpec,
    answers: &[String],
    copied: Option<&str>,
    lang: Lang,
    wrap: f32,
) -> egui::text::LayoutJob {
    let is_light = !ui.visuals().dark_mode;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let ink = ui.visuals().text_color();
    let answer_ink = accent(ui.visuals());
    let faint = secondary_text(is_light);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap;
    let piece = |job: &mut egui::text::LayoutJob, text: &str, color: egui::Color32| {
        job.append(text, 0.0, egui::TextFormat::simple(font.clone(), color));
    };
    for part in pieces(&spec.template, spec.copied.as_deref()) {
        match part {
            Piece::Text(text) => piece(&mut job, text, ink),
            Piece::Blank(name, _) => match answer_of(spec, answers, name) {
                "" => piece(&mut job, &label(name), faint),
                answer => piece(&mut job, answer, answer_ink),
            },
            Piece::Copied(_) => match copied {
                Some(text) => piece(&mut job, text, answer_ink),
                None => piece(&mut job, copied_label(lang), faint),
            },
        }
    }
    job
}

/// The words of the window, which has no [`crate::app::AppState`] to ask.
pub fn insert_label(lang: Lang) -> &'static str {
    in_lang(lang, "Insertar", "Insert", "Ilagay", "डालें")
}

pub fn preview_label(lang: Lang) -> &'static str {
    in_lang(lang, "Así quedará", "How it will read", "Ganito ang lalabas", "ऐसा दिखेगा")
}

struct FormApp {
    title: String,
    spec: FormSpec,
    answers: Vec<String>,
    /// What was last copied when the form opened, for the `{{...}}` that stands for it.
    copied: String,
    lang: Lang,
    is_light: bool,
    outcome: Arc<Mutex<Option<String>>>,
    window: Option<Arc<winit::window::Window>>,
    pacer: crate::pacing::Pacer,
    frames: u32,
    studio: bool,
    /// The most the window may grow to: most of the screen it opened on.
    tallest: f32,
    /// The window that was being typed into when espanso opened this one.
    previous: windows::Win32::Foundation::HWND,
    opened: std::time::Instant,
    /// What arrived behind a Tab or an Enter, kept for the frame after: see
    /// [`crate::ui::keys::hold_after_focus_move`].
    held: Vec<egui::Event>,
    /// Whether the Devanagari face is loaded: for a Hindi interface, a form written in it, or an
    /// answer typed in it.
    devanagari_on: bool,
}

/// How long after opening the window takes the foreground back from the window it was opened
/// over. Long enough for a launcher closing or espanso finishing its keystrokes to hand the
/// activation back there, short enough that a person who chose to click elsewhere is left alone.
const RECLAIM_FOR: std::time::Duration = std::time::Duration::from_millis(1500);

impl FormApp {
    fn close(&self, ctx: &egui::Context, text: Option<String>) {
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = text;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for FormApp {
    /// The focus comes from Windows on every frame, as in the main window: see
    /// [`crate::display::has_keyboard_focus`]. With winit's last notice instead, the field being
    /// typed into could go on looking untouched — no caret, no accent edge — and clicking could not
    /// mend it.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if self.studio {
            crate::app::studio_clicks(ctx, raw_input);
        } else if let Some(focused) = self.window.as_deref().and_then(crate::app::hwnd_of).and_then(crate::display::has_keyboard_focus) {
            raw_input.focused = focused;
        }
        if crate::ui::keys::hold_after_focus_move(&mut raw_input.events, &mut self.held) {
            ctx.request_repaint();
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pacer.wait_for_compositor();
        if self.studio {
            return;
        }
        if let Some(hwnd) = self.window.as_deref().and_then(crate::app::hwnd_of) {
            // Opened from the background, the window can be handed the foreground and then lose it
            // to the window it opened over, which espanso or a closing launcher activates a moment
            // later: on top, still, but greyed and deaf. Taken back while that can still be the
            // reason, and only from that window.
            if self.frames >= 2 && self.opened.elapsed() < RECLAIM_FOR && !self.previous.is_invalid() {
                let foreground = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
                if foreground == self.previous {
                    crate::display::bring_to_front(hwnd);
                }
            }
            crate::display::restore_keyboard_focus(hwnd);
        }
        // A heartbeat, so that whatever Windows changed without a word to winit shows within a
        // quarter of a second rather than at the next keystroke.
        let wait = if self.opened.elapsed() < RECLAIM_FOR { 50 } else { 250 };
        ctx.request_repaint_after(std::time::Duration::from_millis(wait));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frames += 1;
        if !self.devanagari_on && self.answers.iter().any(|a| crate::yaml::model::has_devanagari(a)) {
            self.devanagari_on = true;
            crate::fonts::install(ui.ctx(), true);
            ui.ctx().request_repaint();
        }
        // eframe shows the window after its first frame is painted, so the window can only be
        // brought forward from the second. It was opened by espanso, a process in the background,
        // which Windows does not let take the foreground on a plain request.
        if self.frames == 2 && !self.studio {
            if let Some(hwnd) = self.window.as_deref().and_then(crate::app::hwnd_of) {
                crate::display::bring_to_front(hwnd);
            }
        }
        let ctx = ui.ctx().clone();
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close(&ctx, None);
        }

        let margin = controls::PAGE_MARGIN;
        let background = win_background_for(self.is_light);
        let mut insert = false;
        let mut cancel = false;
        let buttons = egui::Panel::bottom("form-buttons")
            .show_separator_line(false)
            .frame(
                egui::Frame::default()
                    .fill(background)
                    .inner_margin(egui::Margin { left: margin, right: margin, top: 0, bottom: margin }),
            )
            .show(ui, |ui| {
                ui.add_space(controls::GAP_SECTION);
                controls::dialog_buttons(ui, |ui| {
                    let cancel_word = self.lang.strings().cancel;
                    cancel = controls::button(ui, cancel_word, controls::Tone::Normal, true).clicked();
                    ui.add_space(controls::GAP);
                    insert = controls::button(ui, insert_label(self.lang), controls::Tone::Primary, true)
                        .clicked();
                });
            });
        let buttons_height = buttons.response.rect.height();

        let mut content_height = 0.0;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(background)
                    .inner_margin(egui::Margin { left: margin, right: margin, top: margin, bottom: 0 }),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                    let top = ui.min_rect().top();
                    ui.label(controls::h3(&self.title));
                    ui.add_space(controls::GAP_TIGHT);
                    ui.label(controls::muted(
                        in_lang(
                            self.lang,
                            "Completa los campos y pulsa Insertar.",
                            "Fill in the fields and press Insert.",
                            "Punan ang mga field at pindutin ang Ilagay.",
                            "फ़ील्ड भरें और डालें दबाएँ।",
                        ),
                        self.is_light,
                    ));
                    ui.add_space(controls::GAP_SECTION);
                    if fields(ui, "form-window", &self.spec, &mut self.answers, self.frames == 1) {
                        insert = true;
                    }
                    ui.add_space(controls::GAP_SECTION);
                    ui.label(controls::small_muted(preview_label(self.lang), self.is_light));
                    ui.add_space(controls::GAP_TIGHT);
                    result_preview(ui, &self.spec, &self.answers, Some(&self.copied), self.lang);
                    content_height = ui.min_rect().bottom() - top;
                });
            });

        // The window is as tall as what it holds, up to most of the screen; past that it scrolls.
        let wanted = (f32::from(margin) + content_height + buttons_height).min(self.tallest).ceil();
        let current = ctx.content_rect().height();
        if (wanted - current).abs() > 1.0 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(WIDTH, wanted)));
        }

        if insert {
            self.close(&ctx, Some(filled(&self.spec, &self.answers, &self.copied)));
        } else if cancel {
            self.close(&ctx, None);
        }
    }
}

/// The whole `--form` process: find the expansion, ask for its blanks, print the result.
///
/// Prints the trigger back on every way out that is not «Insertar» — cancelled, closed, or a form
/// that is no longer in the file — so espanso puts back what it erased. Prints nothing only when
/// the argument is not a trigger at all.
pub fn run(layout: &crate::install::Layout, settings: &Settings, hex: &str) {
    let Some(trigger) = crate::yaml::form::decode_trigger(hex) else {
        return;
    };
    let studio = cfg!(feature = "preview") && std::env::var_os("EM_PREVIEW_BACKGROUND").is_some();
    let path = layout.espanso_config().join("match").join("base.yml");
    let found = crate::yaml::io::load(&path).ok().and_then(|file| {
        file.entries.iter().find_map(|entry| match entry {
            crate::yaml::model::MatchEntry::Simple(m) if m.trigger == trigger => {
                m.as_form().map(|spec| (m.label.clone(), spec))
            }
            _ => None,
        })
    });
    let Some((name, spec)) = found else {
        emit(&trigger, studio);
        return;
    };

    // Whoever was being typed into, so the result goes back there and not to whatever Windows
    // picks once this window is gone.
    let previous = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };

    let lang = settings.lang;
    let theme_mode = settings.theme_mode;
    let outcome = Arc::new(Mutex::new(None));
    let title = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| trigger.clone());
    let answers = initial_answers(&spec);
    // Read once, as the form opens: whatever is copied while it is up is not what was meant.
    let copied = if spec.copied.is_some() { crate::paste::copied_text() } else { String::new() };
    // Not only for a Hindi interface: a form written in Hindi, opened with Khipu in Spanish, drew
    // its words as boxes.
    let devanagari = lang == Lang::Hi || needs_devanagari(&title, &spec, &copied);

    // The monitor being typed on, not the primary one: the form opens where the eyes already are.
    let monitor = (!previous.is_invalid() && !studio)
        .then(|| crate::display::monitor_of_window(previous))
        .flatten();
    let tallest = match monitor {
        Some((work, scale)) => crate::display::WorkArea::from_rect(work, scale),
        None => crate::display::work_area(crate::display::system_scale()),
    }
    .map_or(720.0, |a| a.height * 0.85);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(crate::install::app_name())
        .with_inner_size([WIDTH, 320.0])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_minimize_button(false)
        .with_window_level(egui::WindowLevel::AlwaysOnTop);
    if let Some(icon) = crate::icons::window_fallback() {
        viewport = viewport.with_icon(icon);
    }
    if studio {
        viewport = viewport.with_active(false).with_window_level(egui::WindowLevel::Normal);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: monitor.is_none(),
        persist_window: false,
        // Never written: nothing here asks eframe to keep anything. Named so that eframe does not
        // go looking for a folder of its own outside Khipu's.
        persistence_path: Some(layout.manager_dir().join("form_state.ron")),
        glow_options: eframe::egui_glow::GlowConfiguration { vsync: false, ..Default::default() },
        ..Default::default()
    };

    let shared = outcome.clone();
    let _ = eframe::run_native(
        "Khipu",
        options,
        Box::new(move |cc| {
            let window = cc.winit_window().cloned();
            // Centred on that monitor in its own pixels, before the window is first shown, so a
            // second screen at another scale gets no conversion through the primary's.
            if let (Some(window), Some((work, _))) = (window.as_deref(), monitor) {
                let size = window.outer_size();
                let x = work.left + (work.right - work.left - size.width as i32) / 2;
                let y = work.top + (work.bottom - work.top - size.height as i32) / 2;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
            }
            let is_light = dress(cc, window.as_deref(), theme_mode, devanagari);
            Ok(Box::new(FormApp {
                title,
                spec,
                answers,
                copied,
                lang,
                is_light,
                outcome: shared,
                window,
                pacer: Default::default(),
                frames: 0,
                studio,
                tallest,
                previous,
                opened: std::time::Instant::now(),
                held: Vec::new(),
                devanagari_on: devanagari,
            }))
        }),
    );

    let text = outcome.lock().ok().and_then(|mut o| o.take());
    if !studio && !previous.is_invalid() {
        crate::display::bring_to_front(previous);
        let _ = crate::display::wait_until_foreground(
            previous.0 as isize,
            std::time::Duration::from_millis(60),
            std::time::Duration::from_millis(1500),
        );
    }
    emit(text.as_deref().unwrap_or(&trigger), studio);
}

/// The window in Khipu's clothes: the same palette, type and Mica bar as the main window, from the
/// same functions, so the two can never drift apart.
fn dress(
    cc: &eframe::CreationContext<'_>,
    window: Option<&winit::window::Window>,
    theme_mode: crate::theme::ThemeMode,
    devanagari: bool,
) -> bool {
    use crate::app::{apply_layout_style, apply_motion, apply_theme, contrast_is_light, refresh_contrast, set_palette_for};
    refresh_contrast();
    let is_light = contrast_is_light().unwrap_or(theme_mode.is_light());
    if let Some(window) = window {
        crate::icons::apply_window(window);
    }
    let work_area = window
        .and_then(|w| crate::display::work_area(w.scale_factor() as f32))
        .or_else(|| crate::display::work_area(crate::display::system_scale()));
    apply_layout_style(&cc.egui_ctx, work_area.map_or(1.0, |a| crate::display::text_scale(a.height)));
    if cfg!(feature = "preview") {
        if let Some(zoom) = std::env::var("EM_PREVIEW_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()) {
            cc.egui_ctx.set_zoom_factor(zoom.clamp(1.0, 4.0));
        }
    }
    crate::theme::refresh_animations();
    apply_motion(&cc.egui_ctx);
    set_palette_for(theme_mode);
    apply_theme(&cc.egui_ctx, window, is_light);
    if let Some(window) = window {
        use winit::platform::windows::{BackdropType, WindowExtWindows as _};
        window.set_system_backdrop(BackdropType::MainWindow);
    }
    crate::fonts::install(&cc.egui_ctx, devanagari);
    is_light
}

/// Whether the form shows words in Devanagari: its name, its text, or what its blanks offer.
fn needs_devanagari(title: &str, spec: &FormSpec, copied: &str) -> bool {
    use crate::yaml::model::has_devanagari;
    has_devanagari(title) || spec.has_devanagari() || has_devanagari(copied)
}

/// The result, to standard output, where espanso reads it. The preview build also leaves it beside
/// the exe, since nothing reads its output there.
fn emit(text: &str, studio: bool) {
    use std::io::Write as _;
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
    if studio {
        if let Some(file) = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("preview-form-output.txt"))) {
            let _ = std::fs::write(file, text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::form::FormField;

    #[test]
    fn the_answers_go_where_their_names_were() {
        let spec = FormSpec {
            template: "Hola [[cliente]], [[tono]]. Adiós, [[cliente]].".into(),
            fields: vec![
                FormField { name: "cliente".into(), ..Default::default() },
                FormField {
                    name: "tono".into(),
                    kind: FieldKind::Choice,
                    options: vec!["Formal".into(), "Cercano".into()],
                    ..Default::default()
                },
            ],
            copied: None,
        };
        let mut answers = initial_answers(&spec);
        assert_eq!(answers, ["", "Formal"]);
        answers[0] = "Ana".into();
        assert_eq!(filled(&spec, &answers, ""), "Hola Ana, Formal. Adiós, Ana.");
        assert_eq!(label("cliente"), "Cliente");
        assert_eq!(label("ñandú"), "Ñandú");
    }
}
