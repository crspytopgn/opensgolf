//! The reference half of the web site's feature guide (web/guide.html, published with the browser version on GitHub
//! Pages): every option of Preferences, Game options and the Enhanced graphics settings, with where it is, what it does,
//! whether it is on for a new player and which presets switch it on, and every key of Change keys. It is made here from
//! the same lists, texts and defaults the game draws its boxes from, so it cannot drift from the game: the test below
//! fails when web/guide.html's generated part differs from what this makes.
//!
//! After adding or changing an option, a key or an option's English text, run
//!
//! ```text
//! OPENSGOLF_WRITE_GUIDE=1 cargo test -p opensgolf guide
//! ```
//!
//! to write the new reference into web/guide.html, and add a line to `enhanced_note` for a new Enhanced graphics option
//! (those have no description in the game). The hand-written half of the page (the feature sections above the reference)
//! is edited in web/guide.html itself.

use crate::extras_ui::{self, ExtrasLine, Preset, Tag};
use crate::prefs_ui::{self, Line};
use crate::{controls, enhanced};
use sg_core::extras::Extras;
use sg_core::lang::tr;

/// The lines around the generated part of web/guide.html.
pub const BEGIN: &str =
    "<!-- reference:begin (made by crates/simgolf/src/guide.rs: OPENSGOLF_WRITE_GUIDE=1 cargo test -p opensgolf guide) -->";
pub const END: &str = "<!-- reference:end -->";

/// What an Enhanced graphics option does, for the guide (the game's box has no descriptions for them). Every option of
/// `enhanced::OPTIONS` needs one: the test says which is missing.
fn enhanced_note(key: &str) -> Option<&'static str> {
    Some(match key {
        "native_text" => "Draws text from the fonts at your screen's real resolution, so it stays sharp at any window size. Layouts stay the original's.",
        "smooth_ui" => "Interface art, sprites and portraits scale smoothly, with clean edges and no shimmer when drawn small.",
        "widescreen" => "On a wide window the course badge, dock, panels and buttons move out to the window's edges instead of sitting in a 4:3 box in the middle.",
        "terrain_filter" => "Anisotropic filtering keeps the course sharp towards the top of the screen. Desktop only.",
        "msaa" => "4x multisampling smooths the edges of terrain, cliffs and lines. Desktop only.",
        "smooth_camera" => "Continuous wheel and pinch zoom, animated quarter turns and gliding scrolls. The game's rules still use the original's three zoom levels.",
        "smooth_motion" => "Golfers, staff, carts, animals and balls move smoothly between the game's 11.5 ticks a second instead of jumping once a tick. Drawing only.",
        "photo_mode" => "Lets the C key or the camera button open photo mode: hide the interface, frame the shot, add a look and a focus blur, and save a picture up to 4K.",
        "frame_cap" => "The most frames a second the game draws: 30, 60, 120, 144, 240 or no cap. Counts with Classic graphics too.",
        "vsync" => "Waits for the display's refresh, so the picture never tears. Takes effect from the next start; always on in a browser. Counts with Classic graphics too.",
        "pixel_perfect" => "Scales the 800 x 600 picture by whole numbers only, so every art pixel is the same size, with bars round it. Also a line of Preferences > Graphics; counts with Classic graphics too.",
        "fx_scaler" => "Sprites and pictures drawn larger than their pixels go through a pixel-art scaler instead of a plain blur.",
        "fx_scaler_mode" => "Which scaler: Sharp (crisp pixels), Edge-directed (smooths diagonals) or Smooth (follows the art's own edges; the strongest).",
        "fx_water" => "Water gets moving waves, sky reflections, glints and foam along the shore, over the theme's own water art.",
        "fx_terrain_light" => "Per-pixel light, ambient occlusion in valleys and soft shadows of the hills on the ground.",
        "fx_sprite_shadows" => "Softens the edges of the art's own shadows under buildings, trees and people.",
        "fx_cast_shadows" => "Buildings, trees and people cast their own outline along the sun onto the ground.",
        "fx_ball_trail" => "A soft trail fades out behind a ball in the air.",
        "fx_ball_impacts" => "A puff of sand in a bunker, rings and drops on water, a flick of grass where a ball comes down.",
        "fx_quality" => "Low, medium or high: the detail of the light map, the blurs and the shadows. Lower is quicker on slow machines.",
        "fx_grass" => "Tiny tufts of grass standing up from the rough and swaying in the wind, at the closest zoom.",
        "fx_sharpen" => "Contrast-adaptive sharpening brings out the art's own detail on big screens, without halos. Desktop only.",
        "fx_crowds" => "Spectators round the greens and fairways of the holes in play during tournaments and championships, cheering now and then.",
        "fx_life" => "Flocks of birds crossing the sky, butterflies over flower beds in summer, leaves falling in autumn.",
        "fx_sky" => "A sky gradient beyond the property and below its cliff instead of the original's black.",
        "fx_time_of_day" => "Morning, midday, golden hour, sunset and a short blue dusk pass over the course.",
        "fx_day_length" => "How long a day of light lasts with Time of day: short (one game month), normal (three) or long (six).",
        "fx_evening_lights" => "With Time of day: lamps with pools of light and fireflies at dusk.",
        "fx_window_lights" => "With Time of day: a warm light in the buildings' windows at night.",
        "fx_sprinklers" => "With Time of day: sprinklers watering the fairways and greens at first light.",
        "fx_clouds" => "Soft cloud shadows drift over the course and dim what is under them.",
        "fx_haze" => "The far part of the view pales towards the horizon's colour, more when zoomed out.",
        "fx_reflections" => "Trees, buildings and golfers mirrored in the water, rippling, with sun glitter.",
        "fx_tilt_shift" => "Blurs the top and bottom of the screen for a miniature look.",
        "fx_grading" => "Gentle contrast, vibrance and a tint for each theme, with slightly darker corners.",
        "fx_bloom" => "The brightest parts (sparkle, white sand, highlights) glow a little.",
        "fx_rain" => "Light rain on some days (the same days for a save), with greyer light and faint shadows. Looks only: see Weather that matters for rain that changes play.",
        "fx_after_rain" => "With Rain: wet, glossy ground and puddles during and after a rainy day, and now and then a rainbow.",
        "fx_lightning" => "Flashes and distant flicker in heavy rain where the climate is wet; never in the desert.",
        "fx_heat" => "On a desert course's hot summer days: bleached midday light, heat haze, and parched grass on the hottest days.",
        "fx_winter" => "The first days of March on parkland and links courses: morning frost, snow showers instead of rain, a light dusting some mornings.",
        "fx_frost" => "Frost on cold spring and autumn mornings, melting by late morning.",
        "fx_mist" => "Mist pooling in hollows and over water, thick at dawn and gone by late morning.",
        "fx_wind" => "A wind for each day: trees, palms and grasses sway, flags fly downwind, and the rain and clouds follow it.",
        "fx_seasons" => "Blossom and fresh greens in spring, autumn colours in September and October, dew on the grass.",
        "fx_water_colour" => "Lighter, more turquoise shallows and a deeper blue in the middle, from the theme's own water colour.",
        "fx_waves" => "Soft waves lying across the wind, drifting downwind: glassy at dawn and dusk, choppy in a storm.",
        "fx_shoreline" => "Water lapping in and out along the shore, with a wet band on the bank.",
        "fx_caustics" => "Sunlight rippling in a web over the bottom of shallow water.",
        "fx_rain_rings" => "With Rain: rings opening on the water where drops fall.",
        "fx_water_life" => "Reeds and lily pads at the edges of ponds, ducks and swans on the larger lakes, and now and then a jumping fish.",
        "fx_ice" => "With Winter: small ponds ice over in the late-winter cold snap, and larger water gets a rim of ice.",
        "fx_spray" => "Foam, drops and rings round fountains and at the foot of waterfalls.",
        _ => return None,
    })
}

/// When an option of Preferences' own boxes is there (none: always).
fn pref_note(l: Line) -> Option<&'static str> {
    Some(match l {
        Line::MakeHd => "With your own SimGolf files in use; not in browsers yet.",
        Line::RemoveHd => "Once you have made an HD pack.",
        Line::Files => "Not in the browser: there you choose the files on the start page.",
        Line::Language => "When a translation is installed.",
        Line::UpdateNow => "Not in the browser, which is always the newest version.",
        Line::PlayCount => "Not in store builds.",
        _ => return None,
    })
}

/// Escapes text for HTML.
fn esc(s: &str) -> String {
    s.trim().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// An anchor id from a text key ("STR_EXTRAS_WEATHER" -> "opt-extras-weather").
fn anchor(key: &str) -> String {
    format!("opt-{}", key.trim_start_matches("STR_").to_lowercase().replace('_', "-"))
}

/// The status pill: on, off or a value.
fn pill(on: bool, text: &str) -> String {
    format!("<span class=\"pill {}\">{}</span>", if on { "on" } else { "off" }, esc(text))
}

fn on_off(on: bool) -> String {
    pill(on, if on { "On" } else { "Off" })
}

/// The interface extras a new player has on: those of a settings.json written without them (extras.rs `from_json`).
fn new_player_ui() -> crate::extras::Settings {
    crate::extras::new_player()
}

/// The rows of the Preferences boxes (the port's lines that are a setting or do something; not the lines that only open
/// another box).
fn prefs_rows(out: &mut String) {
    use crate::popup_ui::PopupKind;
    let ctx = prefs_ui::Ctx {
        files: Some(("Use my SimGolf files... / Use the free graphics", false)),
        languages: true,
        update_now: true,
        play_count: true,
        played: 0,
        make_hd: Some(crate::hd_make::Offer::Make),
        remove_hd: true,
    };
    let e = enhanced::Settings::DEFAULT;
    let boxes = [
        (PopupKind::Prefs, "Preferences"),
        (PopupKind::PrefsGraphics, "Preferences › Graphics"),
        (PopupKind::PrefsFiles, "Preferences › Game files"),
        (PopupKind::PrefsMore, "Preferences › More"),
    ];
    out.push_str("<h3 id=\"ref-preferences\">Preferences and its boxes</h3>\n");
    out.push_str("<p class=\"lede\">In a game: <b>System Functions › Preferences</b> (the dock's System Functions button). The original's four ticks come first; these are OpenSGolf's own lines under them.</p>\n");
    out.push_str("<div class=\"tw\"><table>\n<thead><tr><th>Option</th><th>What it does</th><th>New player</th></tr></thead>\n<tbody>\n");
    for (kind, place) in boxes {
        for l in prefs_ui::items(kind, &ctx) {
            // the original's ticks, and the lines that only open the next box (their box has its own rows)
            if matches!(
                l,
                Line::Thoughts
                    | Line::Advisor
                    | Line::Animals
                    | Line::Sound
                    | Line::GameOptions
                    | Line::Graphics
                    | Line::Controls
                    | Line::GameFiles
                    | Line::More
            ) {
                continue;
            }
            let label = prefs_ui::label(l, &ctx, 100);
            // a stepped line's name without its value ("Text size: normal" -> "Text size"), the value in its own column
            let label = match l {
                Line::TextSize | Line::Language => label.split(':').next().unwrap_or(&label).to_string(),
                _ => label,
            };
            let desc = prefs_ui::desc_key(l, &ctx).map(tr).unwrap_or("");
            let default = match l {
                Line::Hd => on_off(false),
                Line::Enhanced => on_off(e.enabled),
                Line::PixelPerfect => on_off(e.pixel_perfect),
                Line::FixBugs => on_off(e.fix_original_bugs),
                Line::UpdateCheck => on_off(new_player_ui().update_check),
                Line::PlayCount => on_off(true),
                Line::TextSize => pill(false, tr(crate::textsize::name_key(100))),
                Line::Language => pill(false, "English"),
                _ => "<span class=\"pill act\">Opens</span>".to_string(),
            };
            let note = pref_note(l).map_or(String::new(), |n| format!(" <span class=\"when\">{}</span>", esc(n)));
            out.push_str(&format!(
                "<tr><td><b>{}</b><span class=\"where\">{}</span></td><td>{}{}</td><td>{}</td></tr>\n",
                esc(&label),
                esc(place),
                esc(desc),
                note,
                default
            ));
        }
    }
    out.push_str("</tbody></table></div>\n");
}

/// The rows of the Game options box, page by page.
fn extras_rows(out: &mut String) {
    use crate::extras::Settings;
    let off = (Extras::default(), Settings::ALL_OFF);
    let ui0 = new_player_ui();
    let helpers = Preset::Helpers.values().unwrap();
    let osg = Preset::OpenSGolf.values().unwrap();
    // a tick the presets leave as the player has it (extras.rs `with_kept`)
    let kept = |l: ExtrasLine| match l {
        ExtrasLine::Ui(i) => Settings::ALL_OFF.with_kept(&Settings::from_bits(1 << i)).bits() & 1 << i != 0,
        _ => false,
    };
    out.push_str("<h3 id=\"ref-game-options\">Game options</h3>\n");
    out.push_str(&format!(
        "<p class=\"lede\"><b>Preferences › Game options</b>. The first line steps through the presets: <b>{}</b>, <b>{}</b>, <b>{}</b> and <b>{}</b>. \
         The same choice is the <b>Game</b> box on Select Difficulty and the launcher's <b>How to play</b> tab. \
         Lines that change how the game plays carry a tag: <i>{}</i>, <i>{}</i>, <i>{}</i> or <i>{}</i>.</p>\n",
        esc(tr(Preset::Original.key())),
        esc(tr(Preset::Helpers.key())),
        esc(tr(Preset::OpenSGolf.key())),
        esc(tr(Preset::Custom.key())),
        esc(tr(Tag::Light.key())),
        esc(tr(Tag::Noticeable.key())),
        esc(tr(Tag::Big.key())),
        esc(tr(Tag::Experimental.key())),
    ));
    for page in 0..extras_ui::PAGES.len() as u8 {
        let name = tr(extras_ui::PAGES[page as usize]);
        out.push_str(&format!(
            "<h4 id=\"ref-game-options-{}\">{} <span class=\"banner\">{}</span></h4>\n",
            name.to_lowercase().replace(' ', "-"),
            esc(name),
            esc(tr(extras_ui::BANNERS[page as usize]))
        ));
        out.push_str(&format!(
            "<div class=\"tw\"><table>\n<thead><tr><th>Option</th><th>What it does</th><th>New player</th><th>{}</th><th>{}</th></tr></thead>\n<tbody>\n",
            esc(tr(Preset::Helpers.key())),
            esc(tr(Preset::OpenSGolf.key()))
        ));
        // every line, the cut content's too (listed only with the original's files)
        extras_ui::show_cut_content(true);
        for l in extras_ui::page_lines(page) {
            let (text, _) = extras_ui::line_text(l, &off.0, &off.1);
            // a line's name without its value ("Calendar: normal" -> "Calendar")
            let text = match text.split_once(':') {
                Some((a, _)) if extras_ui::is_plain(l) => a.to_string(),
                _ => text,
            };
            let key = match l {
                ExtrasLine::Game(i) => sg_core::extras::OPTION_KEYS[i],
                ExtrasLine::Ui(i) => crate::extras::UI_KEYS[i],
                ExtrasLine::Calendar => "STR_EXTRAS_CALENDAR",
                ExtrasLine::Group(g) => extras_ui::GROUPS[g].0,
                ExtrasLine::Preset => "STR_EXTRAS_PRESET",
            };
            let desc = tr(extras_ui::desc_key(l, &off.0, &off.1));
            let tag = extras_ui::tag(l).map_or(String::new(), |t| format!(" <span class=\"tag\">{}</span>", esc(tr(t.key()))));
            let value = |e: &Extras, ui: &Settings| {
                let v = extras_ui::line_value(l, e, ui);
                if extras_ui::is_plain(l) {
                    let (t, _) = extras_ui::line_text(l, e, ui);
                    let t = t.split_once(':').map_or(t.as_str(), |(_, b)| b).trim().to_string();
                    pill(v != 0, &t)
                } else {
                    on_off(v == 1)
                }
            };
            let new = value(&off.0, &ui0);
            let (h, o) = if kept(l) {
                let k = "<span class=\"pill kept\">Your choice</span>".to_string();
                (k.clone(), k)
            } else {
                (value(&helpers.0, &helpers.1), value(&osg.0, &osg.1))
            };
            let when = if l == ExtrasLine::Game(1) { " <span class=\"when\">Listed only with your own SimGolf files.</span>" } else { "" };
            out.push_str(&format!(
                "<tr id=\"{}\"><td><b>{}</b>{}</td><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                anchor(key),
                esc(&text),
                tag,
                esc(desc),
                when,
                new,
                h,
                o
            ));
        }
        out.push_str("</tbody></table></div>\n");
    }
}

/// The rows of the Enhanced graphics settings, page by page.
fn enhanced_rows(out: &mut String) {
    let d = enhanced::Settings::DEFAULT;
    out.push_str("<h3 id=\"ref-enhanced\">Enhanced graphics settings</h3>\n");
    out.push_str(&format!(
        "<p class=\"lede\"><b>Preferences › Graphics › Enhanced graphics settings</b>. Every page starts with the master switch, <b>{}</b>, which is <b>off</b> for a new player: \
         with it off you play in Classic and none of these draw, except the three marked as counting with Classic too. \
         The last column is what each option is set to once you switch Enhanced graphics on. {}</p>\n",
        esc(tr("STR_ENHANCED_MASTER")),
        esc(tr("STR_ENHANCED_BANNER"))
    ));
    for page in 0..enhanced::PAGES.len() as u8 {
        let name = tr(enhanced::PAGES[page as usize]);
        out.push_str(&format!("<h4 id=\"ref-enhanced-{}\">{}</h4>\n", name.to_lowercase(), esc(name)));
        out.push_str(
            "<div class=\"tw\"><table>\n<thead><tr><th>Option</th><th>What it does</th><th>With Enhanced on</th></tr></thead>\n<tbody>\n",
        );
        for o in enhanced::OPTIONS.iter().filter(|o| o.page == page) {
            let label = tr(o.label);
            let label = if o.choices.is_empty() { label.to_string() } else { label.split(':').next().unwrap_or(label).to_string() };
            let v = (o.get)(&d);
            let value = match o.choices.get(v as usize) {
                Some(c) => pill(true, tr(c)),
                None => on_off(v != 0),
            };
            out.push_str(&format!(
                "<tr id=\"opt-enhanced-{}\"><td><b>{}</b></td><td>{}</td><td>{}</td></tr>\n",
                o.key.replace('_', "-"),
                esc(&label),
                esc(enhanced_note(o.key).unwrap_or("")),
                value
            ));
        }
        out.push_str("</tbody></table></div>\n");
    }
}

/// A key's name as the page shows it: letters as on the keyboard (the game's own list writes a letter without Shift in
/// lower case).
fn key_label(b: controls::Binding) -> String {
    let l = b.label();
    if l.chars().count() == 1 {
        l.to_uppercase()
    } else {
        l
    }
}

/// The keys of Change keys.
fn key_rows(out: &mut String) {
    let keys = controls::Settings::default();
    out.push_str("<h3 id=\"ref-keys\">Every key</h3>\n");
    out.push_str("<p class=\"lede\">Each can be given another key on <b>Preferences › Controls and keys › Change keys</b>, which also has the <b>Original</b> and <b>Modern controls</b> presets and the <b>WASD camera</b>. F8 in a game lists them too.</p>\n");
    out.push_str("<div class=\"tw\"><table class=\"keys\">\n<thead><tr><th>Action</th><th>Key</th></tr></thead>\n<tbody>\n");
    for (i, a) in controls::ACTIONS.iter().enumerate() {
        let key = keys.binding(i).map(key_label).unwrap_or_else(|| "-".into());
        let wasd = match a.wasd {
            controls::Wasd::None => String::new(),
            controls::Wasd::Also(k) => format!(
                " <span class=\"when\">WASD camera: also {}</span>",
                esc(&key_label(controls::Binding { key: k, shift: false, ctrl: false }))
            ),
            controls::Wasd::Moves(k) => format!(
                " <span class=\"when\">WASD camera: {}</span>",
                esc(&key_label(controls::Binding { key: k, shift: false, ctrl: false }))
            ),
        };
        let aim = if a.ctx == controls::Ctx::Aim { " <span class=\"when\">While aiming your pro</span>" } else { "" };
        out.push_str(&format!("<tr><td>{}{}</td><td><kbd>{}</kbd>{}</td></tr>\n", esc(tr(a.text)), aim, esc(&key), wasd));
    }
    out.push_str("</tbody></table></div>\n");
}

/// The generated part of web/guide.html, markers included.
pub fn reference() -> String {
    let mut out = String::new();
    out.push_str(BEGIN);
    out.push('\n');
    prefs_rows(&mut out);
    extras_rows(&mut out);
    enhanced_rows(&mut out);
    key_rows(&mut out);
    out.push_str(END);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/guide.html")
    }

    #[test]
    fn every_enhanced_option_has_a_note() {
        let missing: Vec<_> = enhanced::OPTIONS.iter().filter(|o| enhanced_note(o.key).is_none()).map(|o| o.key).collect();
        assert!(missing.is_empty(), "add a line for {missing:?} to guide.rs enhanced_note (the web site's feature guide)");
    }

    #[test]
    fn guide_reference_is_current() {
        let path = page_path();
        let page = std::fs::read_to_string(&path).expect("web/guide.html");
        let (Some(a), Some(b)) = (page.find(BEGIN), page.find(END)) else {
            panic!("web/guide.html has lost its reference markers ({BEGIN} ... {END})");
        };
        let now = &page[a..b + END.len()];
        let want = reference();
        if now == want {
            return;
        }
        if std::env::var_os("OPENSGOLF_WRITE_GUIDE").is_some() {
            let new = format!("{}{}{}", &page[..a], want, &page[b + END.len()..]);
            std::fs::write(&path, new).expect("writing web/guide.html");
            return;
        }
        panic!(
            "web/guide.html's option reference is out of date (an option, key or text changed). \
             Run OPENSGOLF_WRITE_GUIDE=1 cargo test -p opensgolf guide to write it, and check the page's hand-written sections too"
        );
    }
}
