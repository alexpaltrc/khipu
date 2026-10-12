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

//! Task-oriented quick guide. Short, scannable steps describe the current interface.
//! Read-only except for explicit navigation; all examples are illustrative, never saved.
//!
//! **Organised by what the reader came to do**, not by feature: one lead card for the only thing a
//! first-time reader needs (make an expansion), then one collapsed heading per task. Eight questions
//! that fit on a screen beat three open cards that do not, because the reader is here with a
//! question already in mind and is scanning for it.
//!
//! What goes in a body must be true of the interface *as it stands*. This screen is the easiest one
//! in the app to leave behind: nothing breaks when it lies, so nothing tells us it has started to.
//! Where a sentence names something the user will look for — a settings group, the pause button —
//! it is filled in from [`crate::i18n`] rather than retyped here, so renaming it in one place does
//! not leave four translations pointing at a name that no longer exists.

use crate::app::{AppState, View};
use super::{controls, studio};
use super::studio::text;
use super::keys;
use super::glyphs::{self, Glyph};

/// One task: a heading you can open, with the steps inside.
///
/// Closed by default and unindented, so the answer starts at the same left edge as the question.
/// The rule goes *between* topics, never above the first or below the last: a line with nothing
/// under it reads as an item that failed to draw.
fn topic(ui: &mut egui::Ui, first: bool, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    if !first {
        // The separator brings its own few pixels either side; adding a full gap on top made eight
        // closed headings take a screen and a half.
        ui.add_space(controls::GAP_TIGHT);
        ui.separator();
        ui.add_space(controls::GAP_TIGHT);
    }
    // Drawn here rather than by egui's CollapsingHeader, whose triangle is nobody's icon family: the
    // chevron is the one Windows puts on an expander, pointing where the answer will open.
    let id = ui.make_persistent_id(("topic", title));
    let mut open = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), controls::FIELD_HEIGHT),
        egui::Sense::click(),
    );
    controls::subtle_wash(ui, &response, true);
    if ui.is_rect_visible(rect) {
        let ink = ui.visuals().text_color();
        let icon = egui::Rect::from_min_size(
            egui::pos2(rect.left() + controls::GAP, rect.center().y - CHEVRON * 0.5),
            egui::Vec2::splat(CHEVRON),
        );
        let glyph = if open.is_open() { Glyph::ChevronDown } else { Glyph::ChevronRight };
        glyphs::paint(ui, icon, glyph, CHEVRON, ink);
        let words = controls::widget_line(ui, egui::RichText::new(title).strong(), egui::TextStyle::Body);
        let at = egui::pos2(icon.right() + controls::GAP_ROW, rect.center().y - words.size().y * 0.5);
        ui.painter()
            .with_clip_rect(rect.intersect(ui.clip_rect()))
            .galley(at, words, ink);
        controls::focus_ring(ui, &response, f32::from(controls::RADIUS_CONTROL));
    }
    if response.clicked() {
        open.toggle(ui);
    }
    open.show_body_unindented(ui, |ui| {
        ui.add_space(controls::GAP);
        contents(ui);
        ui.add_space(controls::GAP_TIGHT);
    });
}

/// The expander's chevron: smaller than a command's icon, because it marks a heading rather than
/// being a button of its own.
const CHEVRON: f32 = 12.0;

/// A paragraph of a topic, in the reading colour. The guide is the one screen made entirely of
/// prose; setting all of it in the supporting grey would make the whole page look like a footnote.
fn para(ui: &mut egui::Ui, body: &str) {
    ui.add(egui::Label::new(body).wrap());
}

/// A named part inside a topic that holds more than one answer.
///
/// The name is in Semibold, the way Windows' own Settings heads a group: set in the reading weight it
/// read as one more sentence, and nobody could tell where one answer ended and the next began. The
/// wider gap above does the rest — a heading belongs to what follows it, not to what came before.
fn part(ui: &mut egui::Ui, first: bool, title: &str, body: &str) {
    ui.add_space(if first { controls::GAP_TIGHT } else { controls::GAP_SECTION });
    ui.add(egui::Label::new(controls::h3(title)).wrap());
    ui.add_space(controls::GAP_TIGHT);
    para(ui, body);
}

/// The way to the hidden theme: ↑ ↑ ↓ ↓ ← → ← →, one key after another. Arrows only, so it can be
/// typed on any keyboard in any layout; no letters, which would mean a different thing on each.
const SECRET: [egui::Key; 8] = {
    use egui::Key::{ArrowDown as D, ArrowLeft as L, ArrowRight as R, ArrowUp as U};
    [U, U, D, D, L, R, L, R]
};

/// How much of [`SECRET`] the keys pressed so far have spelt, after `key`.
///
/// A wrong key does not throw away what came before it when that still starts the sequence: a third
/// ↑ after ↑ ↑ leaves ↑ ↑, not nothing, so whoever is trying is never made to begin again for a key
/// that was right after all. Anything that is not a plain arrow begins again from nothing.
fn advance(progress: usize, key: egui::Key) -> usize {
    let mut typed: Vec<egui::Key> = SECRET[..progress].to_vec();
    typed.push(key);
    // The longest tail of what was typed that is also a beginning of the sequence.
    (1..=typed.len().min(SECRET.len()))
        .rev()
        .find(|&n| typed[typed.len() - n..] == SECRET[..n])
        .unwrap_or(0)
}

/// Follows the keys pressed while this screen is up, and hands the hidden theme over when they
/// spell [`SECRET`]. Reads the keys without taking them: whatever else they do here, they still do.
fn listen_for_secret(ui: &egui::Ui, state: &mut AppState) {
    if state.settings_open {
        return;
    }
    let id = egui::Id::new("tips-secret");
    let mut progress = ui.ctx().data(|d| d.get_temp::<usize>(id)).unwrap_or(0);
    let keys: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => {
                    Some((*key, *modifiers))
                }
                _ => None,
            })
            .collect()
    });
    if keys.is_empty() {
        return;
    }
    for (key, modifiers) in keys {
        progress = if modifiers.is_none() { advance(progress, key) } else { 0 };
        if progress == SECRET.len() {
            progress = 0;
            state.find_khipu_theme();
            break;
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, progress));
}

pub fn show(ui: &mut egui::Ui, state: &mut AppState) {
    listen_for_secret(ui, state);
    let t = state.t();
    // The name the «…» menu gave it, so the way in and the page agree about what this is.
    let title = t.tips_button_tip;
    // Drawn above the scroll area on purpose: the way back must not be something you have to scroll
    // to the end of the guide to find.
    if controls::page_header(ui, t.back, title, state.settings.lang) {
        state.view = View::List;
    }
    controls::page_scroll(ui, "quick-guide", |ui| {
            // Prose sets badly in a very wide column; past this the line becomes hard to track back.
            // A cap, not a width: `set_max_width` takes the number as given.
            ui.set_max_width(ui.available_width().min(820.0));
            lead(ui, state);
            ui.add_space(28.0);
            tasks(ui, state);
            // Where the work underneath comes from, once, at the end of the page that explains it.
            ui.add_space(28.0);
            let is_light = !ui.visuals().dark_mode;
            ui.add(egui::Label::new(controls::small_muted(t.credits_espanso, is_light)).wrap());
            // The window's own gutter is drawn around the scroll area, not inside it.
            ui.add_space(controls::GAP_STACK);
        });
}

/// The one thing somebody opening this screen for the first time is here for, shown rather than
/// described: a trigger, an arrow, and the text it turns into.
fn lead(ui: &mut egui::Ui, state: &mut AppState) {
    let t = state.t();
    let is_light = !ui.visuals().dark_mode;
    let heading = text(
        state,
        "Escribe menos, di lo mismo",
        "Type less, say the same",
        "Mas kaunting tipa, parehong sinasabi",
        "कम लिखें, वही कहें",
    );
    let body = text(
        state,
        "Eliges un activador corto, escribes el texto una sola vez y, a partir de ahí, Khipu lo completa por ti en cualquier programa.",
        "You pick a short trigger, write the text once, and from then on Khipu completes it for you in any program.",
        "Pumili ka ng maikling trigger, isulat ang teksto nang minsanan, at mula noon ay kukumpletuhin ito ng Khipu para sa iyo sa kahit anong programa.",
        "एक छोटा ट्रिगर चुनें, टेक्स्ट एक बार लिखें, और उसके बाद Khipu उसे किसी भी प्रोग्राम में आपके लिए पूरा कर देगा।",
    );
    // With the user's own prefix: the example is what they would really type.
    let prefix = state.settings.prefix.trim().to_owned();
    let trigger = format!("{prefix}{}", text(state, "hola", "hi", "kumusta", "namaste"));
    let typed_caption = text(state, "Activador", "Trigger", "Trigger", "ट्रिगर");
    let shown_caption = text(state, "Aparece", "Appears", "Lalabas", "दिखता है");
    let result = text(
        state,
        "Hola, ¿en qué puedo ayudarte?",
        "Hi, how can I help you?",
        "Kumusta, paano kita matutulungan?",
        "नमस्ते, मैं आपकी कैसे मदद कर सकता हूँ?",
    );
    let note = crate::i18n::fill(
        text(
            state,
            "Lo que escribes es el activador, y lo que aparece en su lugar es el texto: juntos forman una expansión. El «{prefix}» del principio es el prefijo, y evita que una palabra corriente dispare una expansión. Se cambia en {settings}.",
            "What you type is the trigger, and what appears in its place is the text: together they make an expansion. The “{prefix}” at the start is the prefix, and it keeps an ordinary word from firing an expansion. You change it in {settings}.",
            "Ang tinitipa mo ang trigger, at ang lumalabas kapalit nito ang teksto: magkasama silang bumubuo ng isang expansion. Ang “{prefix}” sa simula ang prefix, at pinipigilan nitong ma-trigger ng karaniwang salita ang isang expansion. Mababago ito sa {settings}.",
            "आप जो लिखते हैं वह ट्रिगर है, और उसकी जगह जो दिखता है वह टेक्स्ट: दोनों मिलकर एक विस्तार बनाते हैं। शुरुआत का “{prefix}” उपसर्ग है, जो किसी आम शब्द को विस्तार चलाने से रोकता है। इसे {settings} में बदला जाता है।",
        ),
        &[("prefix", prefix.as_str()), ("settings", t.settings_title)],
    );

    let mut creating = false;
    ui.vertical(|ui| {
        ui.add(egui::Label::new(controls::h3(heading)).wrap());
        ui.add_space(controls::GAP_TIGHT);
        ui.add(egui::Label::new(controls::muted(body, is_light)).wrap());
        ui.add_space(controls::GAP_ROW);
        controls::inset_frame(ui).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Each half named above itself, so the words in the note below point at something.
            let written = controls::code(trigger, ui.visuals());
            let arrow = controls::muted("→", is_light);
            // The trigger and the arrow sit in a box one text line tall, centred, so they share the
            // middle of the first line of what appears rather than riding above it.
            let line = ui.text_style_height(&egui::TextStyle::Body);
            let on_the_line = |ui: &mut egui::Ui, text: egui::RichText| {
                let width = controls::widget_line(ui, text.clone(), egui::TextStyle::Body).size().x;
                ui.allocate_ui_with_layout(
                    egui::vec2(width, line),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| ui.label(text),
                );
            };
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label(controls::small_muted(typed_caption, is_light));
                    on_the_line(ui, written);
                });
                ui.vertical(|ui| {
                    ui.label(controls::small_muted(" ", is_light));
                    on_the_line(ui, arrow);
                });
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width());
                    ui.label(controls::small_muted(shown_caption, is_light));
                    ui.add(egui::Label::new(result).wrap());
                });
            });
        });
        ui.add_space(controls::GAP_ROW);
        ui.add(egui::Label::new(controls::small_muted(note, is_light)).wrap());
        ui.add_space(controls::GAP_ROW);
        creating = controls::primary_button(ui, t.new_expansion).clicked();
    });
    // Acted on after the frame closes: [`studio::create_expansion`] moves the whole screen out from
    // under us, and doing that half way through painting this card is how a frame ends up drawn
    // against a view that no longer exists.
    if creating {
        studio::create_expansion(ui.ctx(), state);
    }
}

/// Everything else, one heading per task: the everyday ones first, the occasional ones after, and
/// the table of keys last, as the reference it is.
fn tasks(ui: &mut egui::Ui, state: &AppState) {
    let t = state.t();
    let keys = state.search_shortcut.label(state.settings.lang);
    ui.vertical(|ui| {
        topic(
            ui,
            true,
            text(state, "Encontrar y editar", "Find and edit", "Hanapin at i-edit", "खोजें और संपादित करें"),
            |ui| {
                para(ui, text(
                    state,
                    "Busca por el activador, por su nombre o por el texto que inserta; Ctrl+F lleva el cursor al buscador. Abre una fila para leer el texto completo: Editar y Eliminar están dentro de ese detalle, no en cada fila.",
                    "Search by the trigger, by its name or by the text it inserts; Ctrl+F puts the cursor in the search box. Open a row to read the whole text: Edit and Delete live inside that detail, not on every row.",
                    "Maghanap gamit ang trigger, ang pangalan nito o ang tekstong ipinapasok nito; dinadala ng Ctrl+F ang cursor sa paghahanap. Buksan ang isang row para mabasa ang buong teksto: nasa loob ng detalye ang I-edit at Tanggalin, hindi sa bawat row.",
                    "ट्रिगर से, उसके नाम से या उसके डाले जाने वाले टेक्स्ट से खोजें; Ctrl+F कर्सर को खोज बॉक्स में ले जाता है। पूरा टेक्स्ट पढ़ने के लिए पंक्ति खोलें: संपादित करें और हटाएँ उसी विवरण के अंदर हैं, हर पंक्ति पर नहीं।",
                ));
            },
        );

        // Second, because after making and finding expansions it is what they are used through.
        let mut search_body = crate::i18n::fill(t.tip_search_body, &[("keys", &keys)]);
        // Only while the combination is ours: with espanso's own bar in charge, Settings has no box
        // to change it in, and the guide must not send anyone looking for one.
        if state.hotkey.is_some() {
            search_body.push(' ');
            search_body.push_str(&crate::i18n::fill(
                text(
                    state,
                    "La combinación se cambia en {settings}.",
                    "You change the combination in {settings}.",
                    "Napapalitan ang kombinasyon sa {settings}.",
                    "संयोजन {settings} में बदला जाता है।",
                ),
                &[("settings", t.settings_title)],
            ));
        }
        topic(
            ui,
            false,
            text(state, "Buscar desde cualquier aplicación", "Search from any application", "Maghanap mula sa kahit anong app", "किसी भी ऐप से खोजें"),
            |ui| para(ui, &search_body),
        );

        let folders_body = crate::i18n::fill(
            text(
                state,
                "El selector {all}, junto al buscador, muestra solo una carpeta o {none}. Para mover varias expansiones de una vez, elige {select} en el menú «…»; para mover una sola, arrástrala hasta una carpeta o usa el menú de su detalle. En el mismo selector están {new} y {options}, donde puedes cambiarle el nombre o el prefijo, exportarla o eliminarla.",
                "The {all} picker, beside the search box, shows just one folder or {none}. To move several expansions at once, choose {select} in the «…» menu; to move one, drag it onto a folder or use the menu in its detail. The same picker holds {new} and {options}, where you can rename a folder, change its prefix, export it or delete it.",
                "Ang {all} na pagpipilian, katabi ng paghahanap, ay nagpapakita ng isang folder lang o {none}. Para maglipat ng ilang expansion nang sabay, piliin ang {select} sa menu na «…»; para sa isa, i-drag ito sa isang folder o gamitin ang menu sa detalye nito. Nasa parehong pagpipilian ang {new} at {options}, kung saan mapapalitan ang pangalan o prefix, mai-e-export o matatanggal ang folder.",
                "खोज के बगल वाला {all} चयनकर्ता केवल एक फ़ोल्डर या {none} दिखाता है। एक साथ कई विस्तार ले जाने के लिए «…» मेनू में {select} चुनें; एक को ले जाने के लिए उसे फ़ोल्डर पर खींचें या उसके विवरण का मेनू इस्तेमाल करें। उसी चयनकर्ता में {new} और {options} हैं, जहाँ फ़ोल्डर का नाम या उपसर्ग बदल सकते हैं, निर्यात या हटा सकते हैं।",
            ),
            &[
                ("all", text(state, "Todas las carpetas", "All folders", "Lahat ng folder", "सभी फ़ोल्डर")),
                ("none", t.no_folder),
                ("select", text(state, "Seleccionar varias", "Select several", "Pumili ng ilan", "कई चुनें")),
                ("new", t.add_new_folder.trim_start_matches(['+', ' ']).trim_end_matches('…')),
                ("options", text(state, "Opciones de carpeta", "Folder options", "Mga opsyon ng folder", "फ़ोल्डर विकल्प")),
            ],
        );
        topic(
            ui,
            false,
            text(state, "Organizar en carpetas", "Organize into folders", "Ayusin sa mga folder", "फ़ोल्डर में व्यवस्थित करें"),
            |ui| {
                para(ui, &folders_body);
                ui.add_space(controls::GAP);
                // Said plainly, because the confirmation is the only other place it is said and by
                // then the user is already deciding.
                para(ui, text(
                    state,
                    "Eliminar una carpeta elimina también las expansiones que contiene; la confirmación te dice cuántas son.",
                    "Deleting a folder also deletes the expansions inside it; the confirmation tells you how many.",
                    "Ang pagtanggal ng folder ay tinatanggal din ang mga expansion sa loob nito; sinasabi ng kumpirmasyon kung ilan.",
                    "फ़ोल्डर हटाने पर उसके अंदर के विस्तार भी हट जाते हैं; पुष्टि बताती है कि कितने।",
                ));
            },
        );

        // A use of the app rather than a part of it, and the one people who keep lists ask for: a
        // shared start to every trigger turns a folder into something you can look things up in.
        // Letters, not the folder's prefix — a prefix is symbols only, and the search box treats a
        // run of letters inside a trigger like any other.
        let example = format!("{}dddparacetamol", state.settings.prefix);
        let list_body = crate::i18n::fill(
            text(
                state,
                "Para lo que consultas a menudo —medicamentos, clientes, direcciones—, empieza todos sus activadores con las mismas letras: ddd y el nombre, como en {example}. Escribe ddd en el buscador de la biblioteca o en el de {keys} y aparecen todos; sigue con las primeras letras del nombre y queda solo el que buscas.",
                "For things you look up often — medicines, clients, addresses — start all their triggers with the same letters: ddd and the name, as in {example}. Type ddd in the library's search box or in the {keys} one and they all appear; go on with the first letters of the name and only the one you want is left.",
                "Para sa mga madalas mong hanapin — gamot, kliyente, address — simulan ang lahat ng trigger nila sa parehong mga letra: ddd at ang pangalan, gaya ng {example}. I-type ang ddd sa paghahanap ng aklatan o sa {keys} at lalabas silang lahat; ituloy ang unang mga letra ng pangalan at matitira na lang ang hinahanap mo.",
                "जो चीज़ें आप अक्सर देखते हैं — दवाइयाँ, ग्राहक, पते — उनके सभी ट्रिगर एक जैसे अक्षरों से शुरू करें: ddd और फिर नाम, जैसे {example}। लाइब्रेरी की खोज में या {keys} वाली खोज में ddd लिखें और सभी दिख जाएँगे; नाम के शुरुआती अक्षर लिखते जाएँ और केवल वही बचेगा जो आप चाहते हैं।",
            ),
            &[("example", &example), ("keys", &keys)],
        );
        topic(
            ui,
            false,
            text(state, "Armar una lista de consulta", "Build a lookup list", "Gumawa ng listahang madaling hanapin", "खोजने लायक सूची बनाएँ"),
            |ui| {
                para(ui, &list_body);
                ui.add_space(controls::GAP);
                para(ui, text(
                    state,
                    "Si además los guardas en una carpeta propia, el selector de carpetas te los muestra todos juntos.",
                    "If you also keep them in a folder of their own, the folder picker shows them all together.",
                    "Kung ilalagay mo rin sila sa sariling folder, ipapakita silang lahat nang magkakasama ng pagpipilian ng folder.",
                    "अगर आप उन्हें अपने अलग फ़ोल्डर में भी रखें, तो फ़ोल्डर चयनकर्ता उन्हें एक साथ दिखाता है।",
                ));
            },
        );

        topic(
            ui,
            false,
            text(state, "Usar fechas", "Use dates", "Gumamit ng petsa", "तारीख़ का उपयोग"),
            |ui| {
                para(ui, text(
                    state,
                    "Al crear una expansión elige Fecha en lugar de Texto. Hay cuatro formatos listos y uno que armas por bloques, con su idioma y su zona horaria. La vista previa es un ejemplo: la fecha se calcula cada vez que escribes el activador, no cuando lo guardas.",
                    "When you create an expansion, choose Date instead of Text. There are four ready-made formats and one you build from blocks, with its own language and time zone. The preview is an example: the date is worked out each time you type the trigger, not when you save it.",
                    "Kapag gumagawa ng expansion, piliin ang Petsa sa halip na Teksto. May apat na handang format at isa na binubuo mo sa mga block, may sariling wika at time zone. Halimbawa lang ang preview: kinakalkula ang petsa sa tuwing iti-type mo ang trigger, hindi kapag nag-save ka.",
                    "विस्तार बनाते समय टेक्स्ट के बजाय तारीख़ चुनें। चार तैयार प्रारूप हैं और एक जिसे आप ब्लॉक से बनाते हैं, अपनी भाषा और समय क्षेत्र के साथ। पूर्वावलोकन एक उदाहरण है: तारीख़ हर बार ट्रिगर लिखने पर बनती है, सहेजते समय नहीं।",
                ));
            },
        );

        topic(
            ui,
            false,
            text(state, "Apagar expansiones", "Turn expansions off", "Patayin ang mga expansion", "विस्तार बंद करना"),
            |ui| {
                para(ui, text(
                    state,
                    "Para un texto que usas de vez en cuando y no necesita activador, abre la expansión y apaga su interruptor de Estado. Apagada no se expande ni aparece en el lanzador, pero sigue en la lista, más tenue y al final de su carpeta: puedes copiar su texto desde ahí o volver a encenderla. Desde las opciones de una carpeta se apaga la carpeta entera.",
                    "For a text you use now and then that needs no trigger, open the expansion and turn off its Status switch. Off, it neither expands nor shows in the launcher, but it stays in the list, dimmer and at the end of its folder: you can copy its text from there or turn it back on. A folder's options turn off the whole folder.",
                    "Para sa tekstong paminsan-minsan mo lang ginagamit at hindi kailangan ng trigger, buksan ang expansion at patayin ang switch ng Katayuan nito. Kapag nakapatay, hindi ito nag-e-expand at wala sa launcher, pero nananatili ito sa listahan, mas malabo at nasa dulo ng folder nito: puwede mong kopyahin ang teksto nito roon o buksan itong muli. Sa mga opsyon ng isang folder, mapapatay ang buong folder.",
                    "ऐसे टेक्स्ट के लिए जिसे आप कभी-कभी उपयोग करते हैं और जिसे ट्रिगर की ज़रूरत नहीं, विस्तार खोलें और उसका स्थिति स्विच बंद करें। बंद होने पर वह न विस्तार करता है, न लॉन्चर में दिखता है, पर सूची में रहता है, हल्का और अपने फ़ोल्डर के अंत में: वहाँ से उसका टेक्स्ट कॉपी कर सकते हैं या उसे फिर चालू कर सकते हैं। फ़ोल्डर के विकल्पों से पूरा फ़ोल्डर बंद होता है।",
                ));
            },
        );

        topic(ui, false, t.tip_undo_title, |ui| para(ui, t.tip_undo_body));

        let pause_body = crate::i18n::fill(
            text(
                state,
                "El botón {pause} de la biblioteca detiene las expansiones sin cerrar nada, y el mismo botón las vuelve a activar. El punto de estado, a la izquierda del título, dice si las expansiones están activas, en pausa o sin responder.",
                "The {pause} button in the library stops expansions without closing anything, and the same button turns them back on. The status dot to the left of the title says whether expansions are on, paused or not responding.",
                "Pinapatigil ng {pause} na button sa aklatan ang mga expansion nang walang isinasara, at ibinabalik din ito ng parehong button. Sinasabi ng status dot sa kaliwa ng pamagat kung aktibo, naka-pause o hindi tumutugon ang mga expansion.",
                "लाइब्रेरी का {pause} बटन बिना कुछ बंद किए विस्तार रोक देता है, और वही बटन उन्हें फिर चालू करता है। शीर्षक के बाईं ओर का स्थिति बिंदु बताता है कि विस्तार सक्रिय हैं, रुके हुए हैं या जवाब नहीं दे रहे।",
            ),
            &[("pause", t.tray_pause)],
        );
        topic(
            ui,
            false,
            text(state, "Pausar y usar la bandeja", "Pause and use the tray", "Mag-pause at gamitin ang tray", "रोकें और ट्रे का उपयोग"),
            |ui| {
                para(ui, &pause_body);
                part(ui, false, t.tip_pause_title, t.tip_pause_body);
                part(ui, false, t.tip_where_title, t.tip_where_body);
                part(ui, false, t.tip_pin_title, t.tip_pin_body);
                ui.add_space(controls::GAP);
                tray_illustration(ui);
            },
        );

        let share_body = crate::i18n::fill(
            text(
                state,
                "En {settings} → {transfer} eliges qué carpetas exportar a un archivo; también puedes exportar una sola desde {options}. Quien lo reciba lo importa desde {settings}: se conservan las carpetas y los activadores que ya tenga se omiten, nunca se reemplazan.",
                "In {settings} → {transfer} you choose which folders to export to a file; you can also export just one from {options}. Whoever receives it imports it from {settings}: folders are kept, and triggers they already have are skipped, never replaced.",
                "Sa {settings} → {transfer}, pipiliin mo kung aling mga folder ang ie-export sa isang file; puwede ring isa lang mula sa {options}. Ini-import ito ng tatanggap mula sa {settings}: mananatili ang mga folder, at lalaktawan ang mga trigger na mayroon na siya — hindi pinapalitan.",
                "{settings} → {transfer} में आप चुनते हैं कि कौन-से फ़ोल्डर फ़ाइल में निर्यात करने हैं; {options} से केवल एक भी निर्यात कर सकते हैं। पाने वाला उसे {settings} से आयात करता है: फ़ोल्डर बने रहते हैं, और जो ट्रिगर उसके पास पहले से हैं वे छोड़ दिए जाते हैं, बदले नहीं जाते।",
            ),
            &[
                ("settings", t.settings_title),
                ("transfer", t.transfer_section),
                ("options", text(state, "Opciones de carpeta", "Folder options", "Mga opsyon ng folder", "फ़ोल्डर विकल्प")),
            ],
        );
        topic(
            ui,
            false,
            text(state, "Importar y exportar", "Import and export", "Mag-import at mag-export", "आयात और निर्यात"),
            |ui| para(ui, &share_body),
        );

        // What each setting does, at more length than the one line the dialog gives it: under the
        // same name and in the same order, for whoever asks.
        topic(ui, false, t.settings_title, |ui| {
            part(ui, true, t.autostart_checkbox, t.autostart_hint);
            part(ui, false, t.language_section, t.language_hint);
            part(ui, false, t.appearance_section, t.theme_hint);
            part(ui, false, t.prefix_section, t.prefix_hint);
            part(ui, false, super::settings_view::apply_title(state), t.apply_prefix_hint);
            // Only where Settings shows it: see the same test in the search topic above.
            if state.hotkey.is_some() {
                let default = crate::hotkey::DEFAULT.label(state.settings.lang);
                let hint = crate::i18n::fill(
                    text(
                        state,
                        "Abre el buscador desde cualquier programa. Haz clic en el recuadro y presiona la combinación nueva; el botón de al lado vuelve a {keys}.",
                        "Opens the search box from any program. Click the box and press the new combination; the button beside it goes back to {keys}.",
                        "Binubuksan nito ang search box mula sa kahit anong programa. I-click ang kahon at pindutin ang bagong kombinasyon; ibinabalik ng button sa tabi nito sa {keys}.",
                        "किसी भी प्रोग्राम से खोज बॉक्स खोलता है। बॉक्स पर क्लिक करें और नया संयोजन दबाएँ; बगल वाला बटन {keys} पर वापस ले जाता है।",
                    ),
                    &[("keys", &default)],
                );
                part(ui, false, t.launcher_shortcut_section, &hint);
            }
        });

        topic(
            ui,
            false,
            text(state, "Atajos de teclado", "Keyboard shortcuts", "Mga keyboard shortcut", "कीबोर्ड शॉर्टकट"),
            |ui| shortcut_table(ui, state),
        );
    });
}

/// Every shortcut the app answers to, printed from [`keys`] so the guide cannot drift from what the
/// keys really do: rename a binding there and this row follows it.
fn shortcut_table(ui: &mut egui::Ui, state: &AppState) {
    let lang = state.settings.lang;
    let or = text(state, "o", "or", "o", "या");
    let rows: [(String, &str); 10] = [
        (
            keys::text(keys::NEW, lang),
            text(state, "Nueva expansión", "New expansion", "Bagong expansion", "नया विस्तार"),
        ),
        (
            format!("{} {or} {}", keys::text(keys::FIND, lang), keys::text(keys::FIND_ALT, lang)),
            text(state, "Ir al buscador", "Go to the search box", "Pumunta sa paghahanap", "खोज बॉक्स पर जाएँ"),
        ),
        (
            format!("{} {}", keys::text(keys::UP, lang), keys::text(keys::DOWN, lang)),
            text(state, "Moverse por la lista", "Move through the list", "Gumalaw sa listahan", "सूची में चलें"),
        ),
        (
            keys::text(keys::OPEN, lang),
            text(state, "Editar la expansión abierta", "Edit the open expansion", "I-edit ang bukas na expansion", "खुला विस्तार संपादित करें"),
        ),
        (
            keys::text(keys::DELETE, lang),
            text(state, "Eliminar la expansión abierta, tras confirmar", "Delete the open expansion, after confirming", "Tanggalin ang bukas na expansion, pagkatapos kumpirmahin", "पुष्टि के बाद खुला विस्तार हटाएँ"),
        ),
        (
            keys::text(keys::RENAME, lang),
            text(state, "Opciones de la carpeta que estás viendo", "Options for the folder you are viewing", "Mga opsyon ng folder na tinitingnan mo", "जो फ़ोल्डर देख रहे हैं उसके विकल्प"),
        ),
        (
            keys::text(keys::SAVE, lang),
            text(state, "Guardar en el editor", "Save in the editor", "I-save sa editor", "संपादक में सहेजें"),
        ),
        (
            keys::text(keys::BACK, lang),
            text(state, "Cerrar el menú, el diálogo o el detalle; salir del editor", "Close the menu, the dialog or the detail; leave the editor", "Isara ang menu, dialog o detalye; umalis sa editor", "मेनू, संवाद या विवरण बंद करें; संपादक से निकलें"),
        ),
        (
            keys::click_text(egui::Modifiers::CTRL, lang),
            text(state, "Añadir una expansión a la selección", "Add one expansion to the selection", "Magdagdag ng isang expansion sa pinili", "चयन में एक विस्तार जोड़ें"),
        ),
        (
            keys::click_text(egui::Modifiers::SHIFT, lang),
            text(state, "Seleccionar todo el intervalo", "Select the whole range", "Piliin ang buong hanay", "पूरी श्रेणी चुनें"),
        ),
    ];
    // The keys in one column, so the eye runs down them; the width of that column is the widest
    // combination, so no meaning starts further right than it has to.
    let key_width = rows
        .iter()
        .map(|(k, _)| controls::widget_line(ui, controls::code(k, ui.visuals()), egui::TextStyle::Body).size().x)
        .fold(0.0_f32, f32::max);
    for (combo, meaning) in &rows {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = controls::GAP_WIDE;
            let (slot, _) = ui.allocate_exact_size(
                egui::vec2(key_width, ui.text_style_height(&egui::TextStyle::Body)),
                egui::Sense::hover(),
            );
            let line = controls::widget_line(ui, controls::code(combo, ui.visuals()), egui::TextStyle::Body);
            ui.painter().galley(slot.left_top(), line, egui::Color32::PLACEHOLDER);
            ui.add(egui::Label::new(*meaning).wrap());
        });
        ui.add_space(controls::GAP_TIGHT);
    }
}

/// The tray picture's own measure, here and in the welcome, where it is drawn larger.
pub const TRAY_SIZE: egui::Vec2 = egui::vec2(264.0, 112.0);

/// Where everything in the tray picture is. [`paint_taskbar`] works it out as it draws the bar,
/// so the welcome's animation and the still picture here cannot disagree about it.
pub struct TrayScene {
    /// The middle of the button that opens the hidden icons.
    pub chevron: egui::Pos2,
    /// The place on the taskbar our icon is heading for.
    pub landing: egui::Rect,
    /// The hidden icons' panel, open.
    pub flyout: egui::Rect,
    /// Our icon's place in that panel.
    pub ours: egui::Pos2,
}

/// The taskbar as Windows 11 draws it, at the foot of `rect`: Start and two apps on the left; on
/// the right the chevron, the place ours will take, network and volume, and the clock showing the
/// time it is, in the user's own format. `pressed` lights the chevron's button, from 0 to 1, as it
/// does under the pointer and while its panel is open.
///
/// Painted rather than shipped as an image. A screenshot would be wrong the moment Windows changes
/// its taskbar, wrong on a different accent colour, wrong in the other theme, and blurry on a scaled
/// display. The clock earns its place: "next to the clock" is how everybody describes where the
/// tray is.
pub fn paint_taskbar(ui: &egui::Ui, rect: egui::Rect, pressed: f32) -> TrayScene {
    let is_light = !ui.visuals().dark_mode;
    let accent = crate::app::accent(ui.visuals());
    let ink = crate::app::secondary_text(is_light);
    let quiet = crate::app::text_tertiary(is_light).gamma_multiply(0.55);
    let painter = ui.painter();

    let bar = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.bottom() - 34.0), egui::vec2(rect.width(), 34.0));
    painter.rect_filled(bar, 5.0, crate::app::win_control_for(is_light));
    painter.rect_stroke(bar, 5.0, egui::Stroke::new(1.0, crate::app::line_strong(is_light)), egui::StrokeKind::Inside);
    let cy = bar.center().y;

    // Start, in four panes, and two apps beside it.
    let start = egui::pos2(bar.left() + 20.0, cy);
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let pane = egui::Rect::from_center_size(start + egui::vec2(dx * 3.25, dy * 3.25), egui::vec2(5.5, 5.5));
        painter.rect_filled(pane, 1.0, accent);
    }
    for i in 0..2 {
        let app = egui::Rect::from_center_size(egui::pos2(start.x + 26.0 + i as f32 * 26.0, cy), egui::vec2(15.0, 15.0));
        painter.rect_filled(app, 3.0, quiet);
    }

    // The clock, right-aligned on two lines, the way the taskbar sets it.
    let (time, date) = crate::datefmt::taskbar_clock();
    let font = egui::FontId::proportional(9.0);
    let right = bar.right() - 10.0;
    let mut clock_left = right;
    for (line, dy) in [(time, -6.0), (date, 6.0)] {
        let galley = painter.layout_no_wrap(line, font.clone(), ink);
        let at = egui::pos2(right - galley.size().x, cy + dy - galley.size().y / 2.0);
        clock_left = clock_left.min(at.x);
        painter.galley(at, galley, ink);
    }

    // Volume and network, as one group; then the empty place; then the chevron.
    let volume = clock_left - 14.0;
    let wifi = volume - 19.0;
    for (x, glyph) in [(wifi, Glyph::Wifi), (volume, Glyph::Volume)] {
        glyphs::paint(ui, egui::Rect::from_center_size(egui::pos2(x, cy), egui::vec2(12.0, 12.0)), glyph, 12.0, ink);
    }
    let landing = egui::Rect::from_center_size(egui::pos2(wifi - 29.0, cy), egui::vec2(17.0, 17.0));
    let chevron = egui::pos2(landing.center().x - 25.0, cy);
    if pressed > 0.0 {
        let button = egui::Rect::from_center_size(chevron, egui::vec2(20.0, 26.0));
        painter.rect_filled(button, 4.0, crate::app::text_tertiary(is_light).gamma_multiply(0.16 * pressed));
    }
    let stroke = egui::Stroke::new(1.3, ink);
    painter.line_segment([chevron + egui::vec2(-4.0, 2.0), chevron + egui::vec2(0.0, -2.0)], stroke);
    painter.line_segment([chevron + egui::vec2(0.0, -2.0), chevron + egui::vec2(4.0, 2.0)], stroke);

    // The panel opens directly above the chevron: three icons to a row, ours the last one in.
    let flyout = egui::Rect::from_center_size(
        egui::pos2(chevron.x, bar.top() - 8.0 - 32.0),
        egui::vec2(96.0, 64.0),
    );
    let ours = flyout_cell(flyout, 1, 1);
    TrayScene { chevron, landing, flyout, ours }
}

/// The middle of the panel's icon at `column`, `row`.
fn flyout_cell(flyout: egui::Rect, column: usize, row: usize) -> egui::Pos2 {
    flyout.left_top() + egui::vec2(20.0 + column as f32 * 28.0, 18.0 + row as f32 * 28.0)
}

/// The hidden icons' panel at `flyout`, with the other icons in it but not ours, which is drawn by
/// whoever is moving it.
pub fn paint_flyout(painter: &egui::Painter, visuals: &egui::Visuals, flyout: egui::Rect) {
    let is_light = !visuals.dark_mode;
    let lift = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: egui::Color32::from_black_alpha(if is_light { 28 } else { 80 }),
    };
    painter.add(lift.as_shape(flyout, 8.0));
    painter.rect_filled(flyout, 8.0, crate::app::win_control_for(is_light));
    painter.rect_stroke(flyout, 8.0, egui::Stroke::new(1.0, crate::app::line_strong(is_light)), egui::StrokeKind::Inside);
    let quiet = crate::app::text_tertiary(is_light).gamma_multiply(0.55);
    for (column, row) in [(0, 0), (1, 0), (2, 0), (0, 1)] {
        let icon = egui::Rect::from_center_size(flyout_cell(flyout, column, row), egui::vec2(15.0, 15.0));
        painter.rect_filled(icon, 3.0, quiet);
    }
}

/// The still version of the welcome's demonstration: the panel open, our icon held by the pointer
/// on its way down, and the place it is going outlined — the gesture the tip is asking for.
pub fn tray_illustration(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(TRAY_SIZE, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let accent = crate::app::accent(ui.visuals());
    let scene = paint_taskbar(ui, rect, 1.0);
    let painter = ui.painter();
    painter.rect_stroke(scene.landing, 3.0, egui::Stroke::new(1.5, accent), egui::StrokeKind::Inside);
    paint_flyout(painter, ui.visuals(), scene.flyout);
    let held = scene.ours + (scene.landing.center() - scene.ours) * 0.55;
    super::onboarding_view::paint_tray_mark(painter, ui.visuals(), egui::Rect::from_center_size(held, egui::vec2(19.0, 19.0)));
    super::onboarding_view::paint_pointer(painter, held + egui::vec2(2.0, 3.0), 0.92, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Key::{ArrowDown as D, ArrowLeft as L, ArrowRight as R, ArrowUp as U};

    fn spell(keys: &[egui::Key]) -> usize {
        keys.iter().fold(0, |p, k| if p == SECRET.len() { p } else { advance(p, *k) })
    }

    #[test]
    fn the_sequence_unlocks_and_nothing_shorter_does() {
        assert_eq!(spell(&[U, U, D, D, L, R, L, R]), SECRET.len());
        assert_eq!(spell(&[U, U, D, D, L, R, L]), 7);
        // Simultaneous is impossible here by construction; out of order is not the sequence.
        assert_ne!(spell(&[U, D, U, D, L, R, L, R]), SECRET.len());
    }

    /// A mistake costs only what it has to: retrying needs no pause and no clearing first.
    #[test]
    fn a_wrong_key_keeps_what_still_starts_the_sequence() {
        assert_eq!(spell(&[U, U, U]), 2);
        assert_eq!(spell(&[U, U, D, D, L, L]), 0);
        assert_eq!(spell(&[U, U, D, D, U]), 1);
        assert_eq!(spell(&[U, U, D, egui::Key::A]), 0);
        assert_eq!(spell(&[R, U, U, U, D, D, L, R, L, R]), SECRET.len());
    }
}
