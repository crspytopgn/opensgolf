//! Report a problem (the port's own helper; docs/PLAYING.md, "Reporting a problem"): Preferences > More > *Report a
//! problem...*, or Shift+F8 on the course, open a small box saying what will happen, with a tick for a
//! screenshot. Its OK opens the project's bug report form (.github/ISSUE_TEMPLATE/bug.yml) in the browser, filled in
//! through the issue form's address parameters with the version, the platform, the base set and a short system line
//! with the game's recent warnings, and puts a copy of the game being played (the normal save format) and the screenshot
//! in a Reports folder for the player to drag into the form:
//!
//! * desktop: `Reports` in the save folder, opened in the file manager (explorer, open, xdg-open);
//! * Android: `Reports` in the app's external files folder (seen from a computer over USB, like Photos);
//! * browser: the files are handed to the browser as downloads (web/opensgolf.js, download/).
//!
//! Nothing is sent anywhere: the form is the player's to read, finish and submit. Nothing changes until the player uses
//! it. Test hook: SG_REPORT=1 prints the address it would open and writes the files, without opening the browser or the
//! folder.
//!
//! The port keeps no log file of its own on the desktop, so the warnings and errors it prints (a sprite that cannot be
//! read, a pack's problem, a failed save) are also kept here, the last [`KEEP`] of them ([`warn`]), for the report.

use crate::app::*;
use sg_core::lang::{tr, trf};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The issue form (.github/ISSUE_TEMPLATE/bug.yml); its fields are filled in by their ids.
pub const FORM: &str = "https://github.com/crspytopgn/opensgolf/issues/new?template=bug.yml";

/// The longest address the report opens (browsers and GitHub take longer ones, but some mail and chat programs a
/// player might copy it into do not).
pub const MAX_URL: usize = 6000;

/// Recent warnings and errors kept for a report.
pub const KEEP: usize = 30;

/// The longest warning kept (a path in it can be long).
const LINE_MAX: usize = 300;

static RECENT: Mutex<VecDeque<(String, u32)>> = Mutex::new(VecDeque::new());

/// A warning or an error: printed to the error output as before, and kept for a report (the same line again only counts).
pub fn warn(msg: &str) {
    eprintln!("{msg}");
    note(msg);
}

fn note(msg: &str) {
    let msg: String = msg.chars().take(LINE_MAX).collect();
    let mut r = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(last) = r.back_mut().filter(|l| l.0 == msg) {
        last.1 += 1;
        return;
    }
    if r.len() >= KEEP {
        r.pop_front();
    }
    r.push_back((msg, 1));
}

/// The warnings kept, oldest first ("(x3)" after one seen three times in a row).
pub fn recent() -> Vec<String> {
    let r = RECENT.lock().unwrap_or_else(|e| e.into_inner());
    r.iter().map(|(m, n)| if *n > 1 { format!("{m} (x{n})") } else { m.clone() }).collect()
}

/// The form's Platform option (bug.yml's names) for the system `os` (std::env::consts::OS), the browser build, and a
/// browser on a phone or tablet.
pub fn platform_option(os: &str, browser: bool, touch: bool) -> &'static str {
    if browser {
        return if touch { "Browser (phone or tablet)" } else { "Browser (desktop)" };
    }
    match os {
        "windows" => "Windows",
        "macos" => "macOS",
        "android" => "Android",
        "ios" => "iOS",
        _ => "Linux",
    }
}

/// The form's Base set option.
pub fn base_option(free: bool) -> &'static str {
    if free {
        "OpenSGolf free set"
    } else {
        "Original game files"
    }
}

/// An address parameter's value: UTF-8, every byte but the unreserved ones (RFC 3986) percent-escaped.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// What the form is filled in with.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub version: String,
    pub platform: &'static str,
    pub base: &'static str,
    /// The system line (OS, graphics driver, Enhanced, packs, extras).
    pub system: String,
    /// The recent warnings, oldest first.
    pub log: Vec<String>,
    /// The files written for the player to attach (their names).
    pub files: Vec<String>,
}

/// The "Screenshots, save file, log" field: the files to drag in, then the system line and the warnings as a code block.
fn extra_field(f: &Facts, log: &[String], system: &str) -> String {
    let mut t = String::new();
    if !f.files.is_empty() {
        t += &trf("STR_BUGREPORT_FORM_FILES", &[("FILES", &f.files.join(", "))]);
        t += "\n\n";
    }
    t += "```\n";
    t += system;
    t.push('\n');
    if log.is_empty() {
        t += tr("STR_BUGREPORT_FORM_NO_LOG");
        t.push('\n');
    } else {
        t += tr("STR_BUGREPORT_FORM_LOG");
        t.push('\n');
        for l in log {
            t += l;
            t.push('\n');
        }
    }
    t += "```";
    t
}

fn url_of(f: &Facts, log: &[String], system: &str) -> String {
    format!(
        "{FORM}&version={}&platform={}&baseset={}&what={}&extra={}",
        escape(&f.version),
        escape(f.platform),
        escape(f.base),
        escape(tr("STR_BUGREPORT_FORM_WHAT")),
        escape(&extra_field(f, log, system))
    )
}

/// The filled-in form's address, at most `MAX_URL` long: the oldest warnings are left out first, then the system line is
/// cut short.
pub fn form_url(f: &Facts) -> String {
    let mut log: &[String] = &f.log;
    loop {
        let u = url_of(f, log, &f.system);
        if u.len() <= MAX_URL {
            return u;
        }
        if log.is_empty() {
            break;
        }
        log = &log[1..];
    }
    let mut sys: String = f.system.clone();
    while !sys.is_empty() {
        let n = sys.chars().count();
        sys = sys.chars().take(n * 3 / 4).collect();
        let u = url_of(f, &[], &sys);
        if u.len() <= MAX_URL {
            return u;
        }
    }
    url_of(f, &[], "")
}

/// A report's files' common name: "OpenSGolf report" and the date and time (UTC, as photos are named).
pub fn file_stem(secs: i64) -> String {
    let (y, mo, d, h, mi, s) = crate::photo::civil(secs);
    format!("OpenSGolf report {y:04}-{mo:02}-{d:02} {h:02}-{mi:02}-{s:02}")
}

/// Where a report's files go: `Reports` in the save folder (on Android in the app's external files folder, which a
/// computer sees over USB; in the browser they are downloaded).
pub fn reports_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(p) = crate::android::game_dir().parent() {
        return p.join("Reports");
    }
    crate::app::save_dir().join("Reports")
}

/// The confirmation box's options: the screenshot tick, OK and Cancel (the two drawn without a tick box); what its
/// popup returns for OK.
pub const BOX_SHOT: usize = 0;
pub const BOX_GO: usize = 1;
pub const BOX_CANCEL: usize = 2;
pub const BOX_PLAIN: u32 = 1 << BOX_GO | 1 << BOX_CANCEL;

/// The width the box's explanation is wrapped to.
const BOX_TEXT_W: f32 = 430.0;

/// The confirmation box's lines: the title, the explanation as notes (popup_ui's lines starting with a tab; their shorter
/// rows leave the OK tick room under the options), then the options.
pub fn box_lines() -> Vec<String> {
    let key = if cfg!(target_arch = "wasm32") {
        "STR_BUGREPORT_EXPLAIN_WEB"
    } else if cfg!(target_os = "android") {
        "STR_BUGREPORT_EXPLAIN_ANDROID"
    } else {
        "STR_BUGREPORT_EXPLAIN"
    };
    // an empty note under the title keeps the explanation clear of the free set's header band
    let mut v = vec![tr("STR_BUGREPORT_TITLE").to_string(), "\t".to_string()];
    for para in tr(key).split('\n') {
        v.extend(crate::extras::wrap(crate::popup_ui::NOTE_FONT, para, BOX_TEXT_W).into_iter().map(|l| format!("\t{l}")));
    }
    v.extend([tr("STR_BUGREPORT_SCREENSHOT"), tr("STR_BUGREPORT_OPEN"), tr("STR_BUGREPORT_CANCEL")].map(str::to_string));
    v
}

/// The box's top for `n` lines: in the middle of the 800 x 600 screen, on an even y (the exe's framed boxes).
pub fn box_top(n: usize) -> f32 {
    let h = (n as f32 * 3.0 + 3.0) * 8.0;
    (((600.0 - h) / 2.0).max(4.0) as i32 & !1) as f32
}

#[cfg(target_arch = "wasm32")]
mod web {
    #[link(wasm_import_module = "env")]
    extern "C" {
        /// Opens an address in a new tab (web/opensgolf.js); a note with the link when the browser blocks it.
        pub fn sg_open_url(ptr: *const u8, len: usize);
        /// 1 on a phone or tablet (web/opensgolf.js sgTouchDevice).
        pub fn sg_touch_device() -> i32;
    }
}

/// This build's Platform option.
fn platform_here() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: a plain call into the page's script, no arguments
    let touch = unsafe { web::sg_touch_device() } != 0;
    #[cfg(not(target_arch = "wasm32"))]
    let touch = false;
    platform_option(std::env::consts::OS, cfg!(target_arch = "wasm32"), touch)
}

/// Opens an address (the form, the support page in support.rs): a new tab in the browser, the system's browser elsewhere;
/// false when no browser could be started.
pub fn open_link(url: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: the page reads `len` bytes at `ptr`, which live for the call
        unsafe { web::sg_open_url(url.as_ptr(), url.len()) };
        true
    }
    #[cfg(not(target_arch = "wasm32"))]
    crate::update::open_url(url)
}

/// The test hook: the address is printed and nothing is opened.
fn dry_run() -> bool {
    std::env::var_os("SG_REPORT").is_some_and(|v| !v.is_empty() && v != "0")
}

impl App {
    /// The confirmation box (popup_ui PopupKind::Report), the screenshot ticked.
    pub fn open_report_box(&mut self) {
        self.open_popup(crate::popup_ui::PopupKind::Report);
        if let Some(p) = self.popup.as_mut() {
            p.checks = 1;
        }
    }

    /// The system line: version, OS, the graphics driver's names (`gl`, empty when unknown), Enhanced, HD, packs, extras.
    pub fn report_system_line(&self, gl: &str) -> String {
        #[cfg(target_os = "android")]
        let os = crate::android::device();
        #[cfg(not(target_os = "android"))]
        let os = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
        let packs: Vec<String> = if self.content.stamps.is_empty() {
            self.content.loaded.clone()
        } else {
            self.content
                .stamps
                .iter()
                .map(|s| if s.version.is_empty() { s.id.clone() } else { format!("{} {}", s.id, s.version) })
                .collect()
        };
        let hd = if self.hd.enabled {
            self.hd.pack_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "on".into())
        } else {
            "off".into()
        };
        let on = |b: bool| if b { "on" } else { "off" };
        format!(
            "OpenSGolf {} on {os}; GL: {}; Enhanced {}; HD {hd}; packs: {}; gameplay extras {:#x}; interface extras {:#x}; \
             window {}x{}",
            crate::update::VERSION,
            if gl.is_empty() { "unknown" } else { gl },
            on(crate::enhanced::settings().enabled),
            if packs.is_empty() { "none".into() } else { packs.join(", ") },
            self.extras_shown().bits(),
            crate::extras::settings().bits(),
            self.draw_w as u32,
            self.draw_h as u32
        )
    }

    /// The box's OK: the files written (the game being played, and `shot` as a PNG), the form opened with them named, and a
    /// note saying where they are and what to do with them.
    pub fn report_problem(&mut self, shot: Option<sg_core::assets::Rgba>, gl: &str) {
        let stem = file_stem(miniquad::date::now() as i64);
        let web = cfg!(target_arch = "wasm32");
        // the browser hands a file under download/ to the player as a download (web/opensgolf.js)
        let dir = if web { Path::new("download").to_path_buf() } else { reports_dir() };
        let mut files = Vec::new();
        let mut failed = None;
        if self.land.is_some() {
            let name = format!("{stem}.sgs");
            let path = dir.join(&name);
            match self.save_bytes(6) {
                Ok(b) if sg_core::fsutil::write_file(&path, &b) => files.push(name),
                Ok(_) => failed = Some(path.display().to_string()),
                Err(e) => failed = Some(e),
            }
        }
        if let Some(img) = shot {
            let name = format!("{stem}.png");
            let path = dir.join(&name);
            if sg_core::png::write_png(&path, &img) {
                files.push(name);
            } else {
                failed = Some(path.display().to_string());
            }
        }
        if let Some(f) = &failed {
            warn(&format!("report: could not write {f}"));
        }
        let facts = Facts {
            version: crate::update::VERSION.to_string(),
            platform: platform_here(),
            base: base_option(self.on_free_set()),
            system: self.report_system_line(gl),
            log: recent(),
            files: files.clone(),
        };
        let url = form_url(&facts);
        for f in &files {
            println!("report: wrote {}", dir.join(f).display());
        }
        let folder = dir.display().to_string();
        if dry_run() {
            println!("report: would open {url}");
            if !web && !files.is_empty() {
                println!("report: would open the folder {folder}");
            }
        } else {
            if !open_link(&url) {
                // no browser: the address on the clipboard for the player to paste
                miniquad::window::clipboard_set(&url);
                return self.show_notice(tr("STR_BUGREPORT_NO_BROWSER"));
            }
            #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
            if !files.is_empty() {
                crate::update::open_folder(&dir);
            }
        }
        let msg = match (files.is_empty(), web, cfg!(target_os = "android")) {
            (true, _, _) => tr("STR_BUGREPORT_DONE_NO_FILES").to_string(),
            (false, true, _) => trf("STR_BUGREPORT_DONE_WEB", &[("FILES", &files.join(", "))]),
            (false, false, true) => trf("STR_BUGREPORT_DONE_ANDROID", &[("FILES", &files.join(", ")), ("FOLDER", &folder)]),
            (false, false, false) => trf("STR_BUGREPORT_DONE", &[("FILES", &files.join(", ")), ("FOLDER", &folder)]),
        };
        // the box's title first (the free set draws a notice's first line on its header band)
        self.show_notice(&format!("{}\n{msg}", tr("STR_BUGREPORT_TITLE")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = include_str!("../../../.github/ISSUE_TEMPLATE/bug.yml");

    /// The options of the template's dropdown `id`.
    fn options(id: &str) -> Vec<String> {
        let at = TEMPLATE.find(&format!("id: {id}")).unwrap();
        let line = TEMPLATE[at..].lines().find(|l| l.trim_start().starts_with("options:")).unwrap();
        let list = line.split_once('[').unwrap().1.rsplit_once(']').unwrap().0;
        list.split(", ").map(|s| s.trim().to_string()).collect()
    }

    #[test]
    fn every_platform_and_base_set_is_one_of_the_forms_options() {
        let platforms = options("platform");
        for (os, web, touch) in [
            ("windows", false, false),
            ("macos", false, false),
            ("linux", false, false),
            ("freebsd", false, false),
            ("android", false, false),
            ("ios", false, false),
            ("unknown", true, false),
            ("unknown", true, true),
        ] {
            let p = platform_option(os, web, touch);
            assert!(platforms.iter().any(|o| o == p), "{p} for {os}");
        }
        assert_eq!(platform_option("windows", false, false), "Windows");
        assert_eq!(platform_option("macos", false, false), "macOS");
        assert_eq!(platform_option("android", false, true), "Android");
        assert_eq!(platform_option("unknown", true, false), "Browser (desktop)");
        assert_eq!(platform_option("unknown", true, true), "Browser (phone or tablet)");
        let bases = options("baseset");
        assert!(bases.iter().any(|o| o == base_option(true)));
        assert!(bases.iter().any(|o| o == base_option(false)));
        assert_ne!(base_option(true), base_option(false));
        // the field ids the address fills are the template's
        for id in ["what", "version", "platform", "baseset", "extra"] {
            assert!(TEMPLATE.contains(&format!("id: {id}\n")), "{id}");
        }
    }

    #[test]
    fn values_are_escaped() {
        assert_eq!(escape("a-Z_0.~"), "a-Z_0.~");
        assert_eq!(escape("a b&c=d#e+f?g/h"), "a%20b%26c%3Dd%23e%2Bf%3Fg%2Fh");
        assert_eq!(escape("x\ny"), "x%0Ay");
        assert_eq!(escape("é"), "%C3%A9");
        assert_eq!(escape("Browser (desktop)"), "Browser%20%28desktop%29");
    }

    fn facts(log: usize, line: usize) -> Facts {
        Facts {
            version: "0.5.5".into(),
            platform: "Linux",
            base: base_option(false),
            system: "OpenSGolf 0.5.5 on linux x86_64; GL: Mesa & co".into(),
            log: (0..log).map(|i| format!("sprite: line {i} {}", "x".repeat(line))).collect(),
            files: vec!["OpenSGolf report 2026-10-24 12-00-00.sgs".into()],
        }
    }

    #[test]
    fn the_address_fills_the_fields_and_keeps_to_its_length() {
        let u = form_url(&facts(3, 10));
        assert!(u.starts_with("https://github.com/crspytopgn/opensgolf/issues/new?template=bug.yml&version=0.5.5&platform=Linux&"));
        assert!(u.contains("&baseset=Original%20game%20files&"));
        assert!(u.contains("&what=") && u.contains("&extra="));
        // nothing unescaped past the parameters' own & and =
        let extra = u.split("&extra=").nth(1).unwrap();
        assert!(!extra.contains(['&', '=', ' ', '\n', '#']));
        assert!(extra.contains("Mesa%20%26%20co") && extra.contains("sprite%3A%20line%202"));
        assert!(extra.contains("OpenSGolf%20report%202026-10-24%2012-00-00.sgs"));
        // a long log: the oldest lines go first, the newest stay
        let long = facts(KEEP, LINE_MAX);
        let u = form_url(&long);
        assert!(u.len() <= MAX_URL, "{}", u.len());
        assert!(u.contains(&escape(long.log.last().unwrap())));
        assert!(!u.contains(&escape(&long.log[0])));
        // a system line too long on its own is cut short
        let huge = Facts { system: "y".repeat(10_000), ..facts(0, 0) };
        assert!(form_url(&huge).len() <= MAX_URL);
    }

    #[test]
    fn warnings_are_kept_newest_last_and_repeats_counted() {
        for i in 0..KEEP + 5 {
            note(&format!("test warning {i}"));
        }
        note("test warning again");
        note("test warning again");
        let r = recent();
        assert_eq!(r.len(), KEEP);
        assert_eq!(r.last().unwrap(), "test warning again (x2)");
        assert!(!r.iter().any(|l| l == "test warning 0"));
    }

    #[test]
    fn the_box_has_its_tick_ok_and_cancel_in_order() {
        let l = box_lines();
        let opts: Vec<&String> = l.iter().filter(|s| s.starts_with(' ')).collect();
        assert_eq!(opts, [tr("STR_BUGREPORT_SCREENSHOT"), tr("STR_BUGREPORT_OPEN"), tr("STR_BUGREPORT_CANCEL")]);
        assert_eq!(l[0], tr("STR_BUGREPORT_TITLE"));
        assert!(l[1..l.len() - 3].iter().all(|s| s.starts_with('\t')), "the explanation is notes");
        assert_eq!(BOX_PLAIN, 0b110);
        let top = box_top(l.len());
        assert!(top >= 4.0 && top as i32 % 2 == 0 && top + (l.len() as f32 * 3.0 + 3.0) * 8.0 <= 596.0);
    }

    #[test]
    fn files_are_named_by_date_and_time() {
        assert_eq!(file_stem(1_791_824_733), "OpenSGolf report 2026-10-12 17-05-33");
    }
}
