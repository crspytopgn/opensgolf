//! The play-time count (the port's own, none of it in the original; docs/PLAYING.md "Play-time count"): so the project can
//! see how many hours are played on each system and version, to the second, the game sends anonymous counts to the
//! project's own play-time counter ([`COUNT_URL`], a Cloudflare Worker: stats/README.md):
//!
//! * `start/<platform>/<version>` once at start, e.g. `start/windows/0.6.0`;
//! * `play/<platform>` with `t`, the whole seconds played in this run so far, every [`EVERY`] (5 minutes) of play, and
//!   again whenever the game is left with time not yet sent: the window going to the background (or the app paused on
//!   Android, the page hidden in the browser) and the game quitting. When some of it was multiplayer, also `mc`, `ml`
//!   and `mw`: how many of those seconds were in a shared club, a Neighbours league and the weekly challenge
//!   ([`MP_MODES`]), each kept and only growing as `t` is.
//!
//! `t` only grows within a run, so the counter keeps the highest one it has seen for the run and adds only the increase:
//! a count that is lost is made up by the next, and one that arrives twice adds nothing. That makes it exact to the second
//! without ever counting twice. The last count of each run (its run id, `t` and version: [`Record::last`]) is kept in
//! settings.json and sent again at the next start, so a run that ended without its last count getting through (no
//! internet, the app closed from the background, a crash) is still counted in full; on the desktop and Android it is
//! kept until the counter has answered (at most [`MAX_EARLIER`] runs). Builds up to 0.8.7 sent `play-15min/<platform>` for
//! each 15 minutes instead (and GoatCounter before 0.8.2); the counter still takes those, and their unsent marks
//! ([`Record::pending`]) go once as `kept` seconds, 900 each.
//!
//! The play time is the support reminder's clock (support.rs): real seconds on the course, not paused, not in the
//! background, not on the menus. Each count also carries the version, the count's number in this run and a run id: a random
//! number made at each start, so one run's counts can be told apart (how long people play at a time) but never linked to a
//! player or to another run. Nothing else is sent: no names, saved games or course data, and no cookies.
//! `<platform>` is windows, macos, linux, android or web, with `-arm64` on 64-bit ARM (but Android).
//!
//! It is on by default; the first time a course is played the ticker says so once, and Preferences > More > "Count my play
//! time (anonymous)" turns it off for good (kept in settings.json under [`SETTINGS_KEY`]). Builds for a store or a package
//! manager (`OPENSGOLF_NO_SELF_UPDATE=1`: the `opensgolf_store` cfg, build.rs) have none of it: no tick, no notice, nothing
//! sent, and no address in the program.
//!
//! Each count is one GET of the counter's address ([`count_url`]: `e` the event, `v` the version, `s` the run id or `kept`,
//! `n` the count's number in the run, `t` the seconds, `rnd` against caching): on the desktop and Android by ureq as the
//! update check uses it (net.rs), on a thread of its own with a short timeout (the one at quit is waited for, at most
//! [`QUIT_TIMEOUT_S`]); in the browser by the page (web/opensgolf.js sg_playcount, which makes the run id too: sg_random;
//! the page's own "Count my play time" tick off turns it off as the game's tick does), from inside the page's
//! visibilitychange event when it is hidden. Nothing is ever shown about a failure.
//!
//! Never from tests or scripted runs: under `cargo test`, with --png, --time or --screen, or with any SG_* test hook
//! nothing is sent and the notice is not shown. Test hook: `SG_PLAYCOUNT_LOG=1` prints each count instead of sending it
//! (and shows the notice, without remembering it), with `SG_PLAYTIME=SECONDS` to start the clock anywhere.
// the address and the User-Agent go unused where nothing is sent from Rust (a store's build, the browser, no `net`)
#![cfg_attr(any(opensgolf_store, target_arch = "wasm32", not(feature = "net")), allow(dead_code))]
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Whether this build has the count at all (not a store's build).
pub const AVAILABLE: bool = cfg!(not(opensgolf_store));
/// The project's play-time counter (stats/: a Cloudflare Worker).
#[cfg(not(opensgolf_store))]
pub const COUNT_URL: &str = "https://opensgolf-stats.openmcm2-laps.workers.dev/count";
/// The run id of counts kept from an earlier run (they belong to none).
pub const KEPT_RUN: &str = "kept";
/// settings.json's key.
pub const SETTINGS_KEY: &str = "playcount";
/// The play time between two counts, in seconds.
pub const EVERY: f64 = 5.0 * 60.0;
/// The test hook's variable.
pub const LOG_ENV: &str = "SG_PLAYCOUNT_LOG";
/// Seconds in one of the older builds' kept 15-minute marks.
pub const MARK_S: u64 = 15 * 60;
/// The most one run, or the kept seconds, can report: a week of play (the counter takes no more).
pub const MAX_S: u64 = 7 * 24 * 3600;
/// Earlier runs whose last count is kept until the counter answers.
pub const MAX_EARLIER: usize = 8;
/// How long a count may take before it is dropped.
#[cfg(all(not(opensgolf_store), not(target_arch = "wasm32"), feature = "net"))]
const TIMEOUT_S: u64 = 10;
/// The run's seconds are written to [`Record::last`] this often between counts (not sent), so even a run that is killed
/// is counted to within this at the next start.
pub const CHECKPOINT_S: u64 = 30;
/// The test hook's variable for a shorter time between counts, in seconds (tools/playcount_test.sh): only with a stand-in
/// counter ([`TEST_URL_ENV`]).
pub const TEST_EVERY_ENV: &str = "OPENSGOLF_COUNT_TEST_EVERY";
/// How long the game waits, as it quits, for its last count.
pub const QUIT_TIMEOUT_S: u64 = 2;

/// A run's latest play count: sent again at a later start until the counter has it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunTime {
    /// The run's id.
    pub run: String,
    /// Whole seconds played in the run.
    pub t: u64,
    /// The version that played it.
    pub v: String,
    /// Of those, the seconds in each multiplayer mode ([`MP_MODES`]).
    #[serde(skip_serializing_if = "no_mp")]
    pub mp: [u64; 3],
}

/// The multiplayer modes whose seconds a play count carries, as their query parameters (stats/src/index.js MP_PARAMS):
/// a shared club, a Neighbours league, the weekly challenge.
pub const MP_MODES: [&str; 3] = ["mc", "ml", "mw"];

fn no_mp(mp: &[u64; 3]) -> bool {
    mp.iter().all(|&s| s == 0)
}

/// The multiplayer seconds `mp` (fractions) as sent with `t` seconds: whole, never more than `t`.
pub fn mp_seconds(mp: [f64; 3], t: u64) -> [u64; 3] {
    mp.map(|s| (s.max(0.0).floor() as u64).min(t))
}

/// What is kept in settings.json.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// Preferences > More's tick was taken off: nothing is sent.
    pub off: bool,
    /// The one-time notice was shown.
    pub noticed: bool,
    /// Older builds' 15-minute counts that did not get through: read once, then sent as [`Record::kept_s`].
    pub pending: u32,
    /// Seconds from those to send, with no run (`kept`).
    pub kept_s: u64,
    /// This run's latest play count, kept as it is sent; moves to `earlier` at the next start.
    pub last: Option<RunTime>,
    /// Earlier runs' last counts the counter has not answered yet.
    pub earlier: Vec<RunTime>,
}

/// What a sending thread found got through, for the frame to take off the record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ack {
    /// A run's count with `t` seconds.
    Run(String, u64),
    /// Kept seconds.
    Kept(u64),
}

/// Answers the sending threads have had, waiting for the next frame.
static ACKS: Mutex<Vec<Ack>> = Mutex::new(Vec::new());

fn ack(a: Ack) {
    if let Ok(mut v) = ACKS.lock() {
        v.push(a);
    }
}

fn take_acks() -> Vec<Ack> {
    ACKS.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
}

/// What this run does with the count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Nothing: a store's build, or the player turned it off.
    Off,
    /// A test or scripted run: nothing sent, no notice.
    Quiet,
    /// SG_PLAYCOUNT_LOG: each count printed instead of sent.
    Log,
    /// Counts are sent.
    Send,
}

/// The run's mode: `available` (the build has it), `off` (the player's tick), `log` (the test hook), `scripted` (a test or
/// scripted run).
pub fn mode(available: bool, off: bool, log: bool, scripted: bool) -> Mode {
    if !available || off {
        Mode::Off
    } else if log {
        Mode::Log
    } else if scripted {
        Mode::Quiet
    } else {
        Mode::Send
    }
}

/// The platform's name in the counts: `os` and `arch` as `std::env::consts` has them, `web` in the browser.
pub fn platform(os: &str, arch: &str, web: bool) -> String {
    if web {
        return "web".into();
    }
    let os = match os {
        "windows" | "macos" | "linux" | "android" => os,
        _ => "other",
    };
    if arch == "aarch64" && os != "android" {
        format!("{os}-arm64")
    } else {
        os.to_string()
    }
}

/// This build's platform name.
pub fn platform_here() -> String {
    platform(std::env::consts::OS, std::env::consts::ARCH, cfg!(target_arch = "wasm32"))
}

/// The count sent at start.
pub fn start_event(platform: &str, version: &str) -> String {
    format!("start/{platform}/{version}")
}

/// The play-time count.
pub fn play_event(platform: &str) -> String {
    format!("play/{platform}")
}

/// The first mark after `t` seconds of this run's play: the next whole `every` ([`EVERY`]).
pub fn first_mark(t: f64, every: f64) -> f64 {
    ((t.max(0.0) / every).floor() + 1.0) * every
}

/// Whether the run's play time `t` has reached the mark `next`; if so the mark moves to the next one after `t` (one count
/// however far the clock jumped).
pub fn reached(t: f64, next: &mut f64, every: f64) -> bool {
    if t < *next {
        return false;
    }
    *next = first_mark(t, every);
    true
}

/// The time between counts: [`EVERY`], or the test hook's (5 s or more) with a stand-in counter.
pub fn every_from(test_url: Option<&str>, test_every: Option<&str>) -> f64 {
    let t = local_test_url(test_url).and(test_every.and_then(|v| v.trim().parse::<f64>().ok()));
    t.filter(|t| t.is_finite() && *t >= 5.0).unwrap_or(EVERY)
}

/// Whole seconds played in this run: the clock `played` less where it stood at start, never more than [`MAX_S`].
pub fn run_seconds(played: f64, at_start: f64) -> u64 {
    let t = (played - at_start).max(0.0).floor();
    (t as u64).min(MAX_S)
}

/// Whether the one-time notice is due on the course.
pub fn notice_due(mode: Mode, r: &Record) -> bool {
    matches!(mode, Mode::Send | Mode::Log) && !r.noticed
}

/// `s` as a query value (letters, digits and `-._~` kept, the rest as %XX).
pub fn query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The test hook's variable for a stand-in counter on this computer (tools/playcount_test.sh); not an SG_* hook, so the
/// game counts as it would for a player.
pub const TEST_URL_ENV: &str = "OPENSGOLF_COUNT_TEST_URL";

/// A stand-in counter's address from [`TEST_URL_ENV`]: only one on this computer (`http://127.0.0.1` or
/// `http://localhost`), so counts can never be sent anywhere else.
pub fn local_test_url(v: Option<&str>) -> Option<String> {
    let v = v?.trim();
    let local = ["http://127.0.0.1:", "http://127.0.0.1/", "http://localhost:", "http://localhost/"];
    local.iter().any(|p| v.starts_with(p)).then(|| v.to_string())
}

/// One count to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    pub event: String,
    pub version: String,
    /// The run id, or [`KEPT_RUN`].
    pub run: String,
    /// The count's number in the run (0: the start count, or one sent again).
    pub n: u32,
    /// Seconds (play counts only).
    pub t: Option<u64>,
    /// Of those, the multiplayer seconds ([`MP_MODES`]; play counts only).
    pub mp: [u64; 3],
    /// What to take off the record when it gets through.
    pub ack: Option<Ack>,
}

/// The address that sends `c`; `rnd` against caching.
#[cfg(not(opensgolf_store))]
pub fn count_url(c: &Count, rnd: u64) -> String {
    let e = query_value(&c.event);
    let (v, s) = (query_value(&c.version), query_value(&c.run));
    #[cfg(not(target_arch = "wasm32"))]
    let base = local_test_url(std::env::var(TEST_URL_ENV).ok().as_deref()).unwrap_or_else(|| COUNT_URL.to_string());
    #[cfg(target_arch = "wasm32")]
    let base = COUNT_URL;
    let t = c.t.map_or(String::new(), |t| format!("&t={t}"));
    let mp: String = MP_MODES.iter().zip(c.mp).filter(|(_, s)| *s > 0 && c.t.is_some()).map(|(k, s)| format!("&{k}={s}")).collect();
    format!("{base}?e={e}&v={v}&s={s}&n={}{t}{mp}&rnd={rnd:x}", c.n)
}

/// The User-Agent of a count, e.g. "OpenSGolf/0.6.0 (linux)".
pub fn user_agent(version: &str, platform: &str) -> String {
    format!("OpenSGolf/{version} ({platform})")
}

/// 64 random bits as 16 hex digits: the run id, made at each start.
pub fn run_id_hex(bits: u64) -> String {
    format!("{bits:016x}")
}

/// A fresh run id: the standard library's random hash keys (and the clock) on the desktop and Android, the browser's
/// crypto.getRandomValues in the browser (web/opensgolf.js sg_random).
pub fn new_run_id() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let bits = {
        use std::hash::{BuildHasher, Hasher};
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.write_u32(std::process::id());
        h.finish()
    };
    #[cfg(all(target_arch = "wasm32", not(opensgolf_store)))]
    let bits = {
        #[link(wasm_import_module = "env")]
        extern "C" {
            /// 32 random bits from the browser (web/opensgolf.js).
            fn sg_random() -> u32;
        }
        // SAFETY: no arguments
        unsafe { (u64::from(sg_random()) << 32) | u64::from(sg_random()) }
    };
    #[cfg(all(target_arch = "wasm32", opensgolf_store))]
    let bits = 0u64;
    run_id_hex(bits)
}

/// A record read from settings.json made safe: older builds' unsent marks turned into kept seconds, everything within a
/// week, the run ids valid, at most [`MAX_EARLIER`] earlier runs; nothing kept when the tick is off.
pub fn tidy(mut r: Record) -> Record {
    if r.off {
        return Record { off: true, noticed: r.noticed, ..Default::default() };
    }
    r.kept_s = r.kept_s.saturating_add(u64::from(r.pending).saturating_mul(MARK_S)).min(MAX_S);
    r.pending = 0;
    let ok =
        |l: &RunTime| (8..=32).contains(&l.run.len()) && l.run.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) && l.t > 0;
    r.earlier.retain(ok);
    for l in &mut r.earlier {
        l.t = l.t.min(MAX_S);
    }
    if r.last.as_ref().is_some_and(|l| !ok(l)) {
        r.last = None;
    }
    r
}

/// At start: the last run's count joins the earlier ones still to send (the newest [`MAX_EARLIER`]).
pub fn start_run(r: &mut Record) {
    if let Some(l) = r.last.take() {
        r.earlier.retain(|e| e.run != l.run);
        r.earlier.push(l);
    }
    let n = r.earlier.len();
    if n > MAX_EARLIER {
        r.earlier.drain(..n - MAX_EARLIER);
    }
}

/// Takes what got through off the record: an earlier run sent at least as far, this run's `last` too, the kept seconds
/// sent. True when the record changed.
pub fn apply_acks(r: &mut Record, acks: &[Ack]) -> bool {
    let before = r.clone();
    for a in acks {
        match a {
            Ack::Run(run, t) => {
                r.earlier.retain(|e| !(e.run == *run && e.t <= *t));
                if r.last.as_ref().is_some_and(|l| l.run == *run && l.t <= *t) {
                    r.last = None;
                }
            }
            Ack::Kept(s) => r.kept_s = r.kept_s.saturating_sub(*s),
        }
    }
    *r != before
}

/// The play-time count's state (`App::playcount`).
#[derive(Debug)]
pub struct State {
    pub record: Record,
    /// The mode with the tick on (the tick can be taken off and put back while the game runs).
    base: Mode,
    /// The support clock when this run started: the run's play time is counted from there.
    at_start: f64,
    /// The run's next play time (from `at_start`) that sends a count.
    next: f64,
    /// The time between counts ([`every_from`]).
    every: f64,
    /// The run's seconds in the last play count sent (or tried).
    sent: u64,
    /// The browser page's own "Count my play time" tick is off (web/index.html #playcount): off as with the game's tick.
    page_off: bool,
    platform: String,
    /// This run's id ([`new_run_id`]; empty until a count is sent).
    run: String,
    /// Play counts sent (or tried) in this run: the next one's number less one.
    marks: u32,
    /// The support clock at the last frame: what it moved by since is this frame's play.
    last_played: f64,
    /// This run's seconds in each multiplayer mode ([`MP_MODES`]).
    mp: [f64; 3],
}

impl Default for State {
    fn default() -> State {
        State {
            record: Record::default(),
            base: Mode::Off,
            at_start: 0.0,
            next: EVERY,
            every: EVERY,
            sent: 0,
            page_off: false,
            platform: String::new(),
            run: String::new(),
            marks: 0,
            last_played: 0.0,
            mp: [0.0; 3],
        }
    }
}

impl State {
    /// The mode in use.
    pub fn mode(&self) -> Mode {
        mode(AVAILABLE, self.record.off || self.page_off, self.base == Mode::Log, self.base == Mode::Quiet)
    }

    /// Preferences > More's tick.
    pub fn on(&self) -> bool {
        !self.record.off
    }
}

/// settings.json in the save folder.
fn settings_path() -> std::path::PathBuf {
    let mut p = crate::app::save_dir();
    p.push(crate::hd::SETTINGS_FILE);
    p
}

fn settings_json() -> serde_json::Value {
    sg_core::fsutil::read_file(settings_path())
        .and_then(|d| serde_json::from_slice::<serde_json::Value>(&d).ok())
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Saves the record into settings.json, keeping its other keys.
fn save(r: &Record) {
    let mut v = settings_json();
    if let Ok(e) = serde_json::to_value(r) {
        v[SETTINGS_KEY] = e;
    }
    if let Ok(t) = serde_json::to_string_pretty(&v) {
        sg_core::fsutil::write_file(settings_path(), t.as_bytes());
    }
}

/// Whether a test hook is set (any SG_* variable but the log's own). The browser has no environment (reading it panics
/// there), so never there.
fn test_hooks() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return std::env::vars_os().any(|(k, _)| k.to_str().is_some_and(|k| k.starts_with("SG_") && k != LOG_ENV));
    #[cfg(target_arch = "wasm32")]
    false
}

/// The log test hook (`SG_PLAYCOUNT_LOG`), never in the browser (see `test_hooks`).
fn log_hook() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    return std::env::var_os(LOG_ENV).is_some_and(|v| !v.is_empty() && v != "0");
    #[cfg(target_arch = "wasm32")]
    false
}

/// At start up, after the support reminder's clock (`played`): the saved record, the run's mode (`scripted`: --png,
/// --time or --screen), the start count, and after it the kept seconds and earlier runs' last counts.
pub fn init(played: f64, scripted: bool) -> State {
    let record = tidy(serde_json::from_value(settings_json()[SETTINGS_KEY].clone()).unwrap_or_default());
    let log = log_hook();
    let scripted = scripted || cfg!(test) || test_hooks();
    let base = mode(AVAILABLE, false, log, scripted);
    #[cfg(not(target_arch = "wasm32"))]
    let every = every_from(std::env::var(TEST_URL_ENV).ok().as_deref(), std::env::var(TEST_EVERY_ENV).ok().as_deref());
    #[cfg(target_arch = "wasm32")]
    let every = EVERY;
    let mut s = State {
        record,
        base,
        at_start: played.max(0.0),
        last_played: played.max(0.0),
        next: every,
        every,
        page_off: page_off(),
        platform: platform_here(),
        ..Default::default()
    };
    if log {
        println!("play count: {:?} (platform {}, a count every {:.0} s of play)", s.mode(), s.platform, s.every);
    }
    if !matches!(s.mode(), Mode::Send | Mode::Log) {
        return s;
    }
    s.run = new_run_id();
    start_run(&mut s.record);
    let v = crate::update::VERSION.to_string();
    let mut counts =
        vec![Count { event: start_event(&s.platform, &v), version: v.clone(), run: s.run.clone(), n: 0, t: None, mp: [0; 3], ack: None }];
    if s.record.kept_s > 0 {
        let k = s.record.kept_s;
        let ack = Some(Ack::Kept(k));
        counts.push(Count { event: play_event(&s.platform), version: v, run: KEPT_RUN.into(), n: 0, t: Some(k), mp: [0; 3], ack });
    }
    for e in &s.record.earlier {
        let ack = Some(Ack::Run(e.run.clone(), e.t));
        let (version, run, mp) = (e.v.clone(), e.run.clone(), e.mp.map(|x| x.min(e.t)));
        counts.push(Count { event: play_event(&s.platform), version, run, n: 0, t: Some(e.t), mp, ack });
    }
    // the browser never hears back: its earlier runs go once, now (each is safe to send again: it adds nothing twice)
    if cfg!(target_arch = "wasm32") {
        s.record.earlier.clear();
        s.record.kept_s = 0;
    }
    if s.mode() == Mode::Send {
        save(&s.record);
    }
    s.dispatch(counts, false);
    s
}

impl State {
    /// Sends the play count for the run's seconds now (`t`), first keeping it as [`Record::last`] so a later start sends it
    /// again if it never gets through; `wait`: as the game quits, waited for.
    fn send_play(&mut self, t: u64, wait: bool) {
        let mode = self.mode();
        if !matches!(mode, Mode::Send | Mode::Log) || t == 0 || t <= self.sent || self.run.is_empty() {
            return;
        }
        self.sent = t;
        self.marks += 1;
        let v = crate::update::VERSION.to_string();
        let mp = mp_seconds(self.mp, t);
        self.record.last = Some(RunTime { run: self.run.clone(), t, v: v.clone(), mp });
        if mode == Mode::Send {
            save(&self.record);
        }
        let c = Count {
            event: play_event(&self.platform),
            version: v,
            run: self.run.clone(),
            n: self.marks,
            t: Some(t),
            mp,
            ack: Some(Ack::Run(self.run.clone(), t)),
        };
        self.dispatch(vec![c], wait);
    }

    /// Sends `counts` in order (Send; the rest are dropped after one fails), prints them (Log), or nothing.
    fn dispatch(&self, counts: Vec<Count>, wait: bool) {
        match self.mode() {
            Mode::Log => {
                for c in &counts {
                    let t = c.t.map_or(String::new(), |t| format!(", {t} s"));
                    println!("play count: would send {} (run {}, count {}{t}){}", c.event, c.run, c.n, if wait { " and wait" } else { "" });
                    if let Some(a) = c.ack.clone() {
                        ack(a);
                    }
                }
            }
            Mode::Send => send_now(counts, &self.platform, wait),
            _ => {}
        }
    }
}

/// The browser page's own tick is off (web/opensgolf.js sg_playcount_off); never elsewhere.
fn page_off() -> bool {
    #[cfg(all(not(opensgolf_store), target_arch = "wasm32"))]
    {
        #[link(wasm_import_module = "env")]
        extern "C" {
            /// 1 when the page's "Count my play time" tick is off (web/opensgolf.js).
            fn sg_playcount_off() -> i32;
        }
        // SAFETY: no arguments; the page reads its own storage
        unsafe { sg_playcount_off() != 0 }
    }
    #[cfg(not(all(not(opensgolf_store), target_arch = "wasm32")))]
    false
}

/// The counts on their way: GETs one after another on the desktop and Android, on a thread (or here and waited for, at
/// most [`QUIT_TIMEOUT_S`] each, with `wait`), what gets through told to [`ACKS`]; the page's fetches in the browser.
#[allow(unused_variables)]
fn send_now(counts: Vec<Count>, platform: &str, wait: bool) {
    #[cfg(all(not(opensgolf_store), not(target_arch = "wasm32"), feature = "net"))]
    {
        let agent_name = user_agent(crate::update::VERSION, platform);
        let timeout = if wait { QUIT_TIMEOUT_S } else { TIMEOUT_S };
        let go = move || {
            let agent: ureq::Agent =
                ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(timeout))).build().into();
            for (i, c) in counts.into_iter().enumerate() {
                if i > 0 {
                    // a second apart, as a few counts after the start count
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                let rnd = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
                if agent.get(&count_url(&c, rnd)).header("User-Agent", &agent_name).call().is_err() {
                    return;
                }
                if let Some(a) = c.ack {
                    ack(a);
                }
            }
        };
        if wait {
            go();
        } else {
            // named: a panic in it ends only the count (android.rs); the answer is read only for whether it got through
            let _ = std::thread::Builder::new().name("opensgolf-count".into()).spawn(go);
        }
    }
    #[cfg(all(not(opensgolf_store), target_arch = "wasm32"))]
    {
        #[link(wasm_import_module = "env")]
        extern "C" {
            /// Sends a count: the page fetches the address (web/opensgolf.js).
            fn sg_playcount(ptr: *const u8, len: usize);
        }
        for c in &counts {
            let url = count_url(c, u64::from(c.n) ^ c.t.unwrap_or(0));
            // SAFETY: the page reads `len` bytes at `ptr`, which live for the call
            unsafe { sg_playcount(url.as_ptr(), url.len()) };
        }
    }
}

impl crate::app::App {
    /// Once a frame, after the play time is counted: a count at each 5 minutes of play, what got through taken off the
    /// record, and the one-time notice the first time the course is played.
    pub fn playcount_frame(&mut self) {
        let played = self.support.record.played;
        // this frame's play, counted to the multiplayer mode being played (if any)
        let dt = played - self.playcount.last_played;
        self.playcount.last_played = played;
        let mode = self.playcount.mode();
        if matches!(mode, Mode::Off | Mode::Quiet) {
            return;
        }
        if dt > 0.0 {
            if let Some(i) = self.playcount_mp_mode() {
                self.playcount.mp[i] += dt;
            }
        }
        let p = &mut self.playcount;
        let acks = take_acks();
        if apply_acks(&mut p.record, &acks) && mode == Mode::Send {
            save(&p.record);
        }
        let t = run_seconds(played, p.at_start);
        if reached(t as f64, &mut p.next, p.every) {
            p.send_play(t, false);
        } else if !p.run.is_empty() && t >= p.record.last.as_ref().map_or(p.sent, |l| l.t) + CHECKPOINT_S {
            // between counts the seconds are only written down, for the next start should this run be killed
            let mp = mp_seconds(p.mp, t);
            p.record.last = Some(RunTime { run: p.run.clone(), t, v: crate::update::VERSION.to_string(), mp });
            if mode == Mode::Send {
                save(&p.record);
            }
        }
        let quiet = self.popup.is_none() && self.notice.is_none() && self.ui_ok;
        if self.screen == crate::app::Screen::Play && quiet && notice_due(mode, &self.playcount.record) {
            self.playcount.record.noticed = true;
            if mode == Mode::Send {
                save(&self.playcount.record);
            } else {
                println!("play count: notice shown (not remembered: {LOG_ENV})");
            }
            self.show_toast(sg_core::lang::tr("STR_PLAYCOUNT_NOTICE"));
        }
    }

    /// The multiplayer mode being played, as its place in [`MP_MODES`]: a shared club, a Neighbours league's plot, the
    /// weekly challenge; none in single player (or a build without multiplayer).
    pub fn playcount_mp_mode(&self) -> Option<usize> {
        #[cfg(feature = "multiplayer")]
        {
            let scenario = self.scen.goals.scenario.as_deref();
            if self.club_on() {
                return Some(0);
            }
            if crate::mp::league::league_of(scenario).is_some() {
                return Some(1);
            }
            if scenario.and_then(|s| s.strip_prefix(crate::mp::rival::ID_PREFIX)).is_some_and(crate::mp::challenge::is_week_id) {
                return Some(2);
            }
        }
        None
    }

    /// The game is left (the window to the background, the app paused, the page hidden): the seconds played since the
    /// last count go now, so a run that never comes back is still counted to the second.
    pub fn playcount_left(&mut self) {
        let t = run_seconds(self.support.record.played, self.playcount.at_start);
        self.playcount.send_play(t, false);
        // the player's own clock kept to the second too (support.rs), when it has moved
        if self.support.unsaved > 0.0 {
            self.support_save_now();
        }
    }

    /// The game quits: the last seconds go now, waited for (at most [`QUIT_TIMEOUT_S`]); if they do not get through they
    /// stay in settings.json for the next start.
    pub fn playcount_quit(&mut self) {
        let t = run_seconds(self.support.record.played, self.playcount.at_start);
        self.playcount.send_play(t, true);
        // the player's own clock kept to the second too (support.rs), when it has moved
        if self.support.unsaved > 0.0 {
            self.support_save_now();
        }
        let acks = take_acks();
        if apply_acks(&mut self.playcount.record, &acks) && self.playcount.mode() == Mode::Send {
            save(&self.playcount.record);
        }
    }

    /// Preferences > More's tick: off stops every count (and the start count of later runs); saved.
    pub fn set_playcount(&mut self, on: bool) {
        if !AVAILABLE || self.playcount.on() == on {
            return;
        }
        // on again: the counts go on from the seconds already sent (the time with it off is not counted)
        if on {
            self.playcount.at_start = self.support.record.played - self.playcount.sent as f64;
            self.playcount.next = first_mark(self.playcount.sent as f64, self.playcount.every);
            if self.playcount.run.is_empty() {
                self.playcount.run = new_run_id();
            }
        }
        self.playcount.record.off = !on;
        // the notice would only say what the player has just seen in Preferences
        self.playcount.record.noticed = true;
        // off: nothing kept to send later, never sent
        if !on {
            let noticed = self.playcount.record.noticed;
            self.playcount.record = Record { off: true, noticed, ..Default::default() };
            take_acks();
        }
        save(&self.playcount.record);
        if self.playcount.base == Mode::Log {
            println!("play count: {}", if on { "on" } else { "off" });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_every_5_minutes_of_play() {
        let mut next = first_mark(0.0, EVERY);
        assert_eq!(next, EVERY);
        let mut sent = 0;
        let mut t = 0.0;
        // an hour of play in 1/60 s frames: twelve counts, each at its mark
        while t < 3600.0 + 0.5 {
            if reached(t, &mut next, EVERY) {
                sent += 1;
                assert!((t - sent as f64 * EVERY).abs() < 0.1, "at {t}");
            }
            t += 1.0 / 60.0;
        }
        assert_eq!(sent, 12);
        // a jump counts once
        let mut next = first_mark(0.0, EVERY);
        assert!(reached(1000.0, &mut next, EVERY));
        assert_eq!(next, 1200.0);
        assert!(!reached(1001.0, &mut next, EVERY));
        // the test hook's shorter time, only with a stand-in counter
        let local = Some("http://127.0.0.1:8771/count");
        assert_eq!(every_from(local, Some("20")), 20.0);
        assert_eq!(every_from(local, Some("1")), EVERY, "not under 5 s");
        assert_eq!(every_from(Some("https://example.com/count"), Some("20")), EVERY);
        assert_eq!(every_from(None, Some("20")), EVERY);
        assert_eq!(every_from(local, None), EVERY);
    }

    #[test]
    fn the_run_counts_from_its_start_to_the_second() {
        assert_eq!(run_seconds(20_000.0, 20_000.0), 0);
        assert_eq!(run_seconds(20_937.9, 20_000.0), 937, "whole seconds");
        assert_eq!(run_seconds(10.0, 20.0), 0);
        assert_eq!(run_seconds(1e12, 0.0), MAX_S);
    }

    #[test]
    fn nothing_when_off_scripted_or_in_a_store_build() {
        assert_eq!(mode(true, false, false, false), Mode::Send);
        assert_eq!(mode(true, true, false, false), Mode::Off, "the tick off");
        assert_eq!(mode(true, true, true, false), Mode::Off, "the tick off, the log too");
        assert_eq!(mode(false, false, false, false), Mode::Off, "a store build");
        assert_eq!(mode(false, false, true, true), Mode::Off);
        assert_eq!(mode(true, false, false, true), Mode::Quiet, "a test or scripted run");
        assert_eq!(mode(true, false, true, true), Mode::Log, "the test hook");
        // this build's AVAILABLE follows the store build's switch
        assert_eq!(AVAILABLE, !crate::update::STORE_BUILD);
        // under cargo test a run never sends
        let s = State { base: mode(AVAILABLE, false, false, cfg!(test)), ..Default::default() };
        assert_ne!(s.mode(), Mode::Send);
        // the tick: off is Off, back on is the run's mode again
        let mut s = State { base: Mode::Send, ..Default::default() };
        assert_eq!(s.mode(), if AVAILABLE { Mode::Send } else { Mode::Off });
        s.record.off = true;
        assert_eq!(s.mode(), Mode::Off);
        assert!(!s.on());
        // the browser page's own tick off is off too, whatever the game's tick
        let s = State { base: Mode::Send, page_off: true, ..Default::default() };
        assert_eq!(s.mode(), Mode::Off);
        assert!(!notice_due(s.mode(), &s.record));
    }

    #[test]
    fn the_notice_once_and_only_where_counts_go() {
        let mut r = Record::default();
        assert!(notice_due(Mode::Send, &r));
        assert!(notice_due(Mode::Log, &r));
        assert!(!notice_due(Mode::Quiet, &r), "not in stills or tests");
        assert!(!notice_due(Mode::Off, &r));
        r.noticed = true;
        assert!(!notice_due(Mode::Send, &r), "once");
        // it reads back from settings.json, and old settings read as on and not yet shown
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(serde_json::from_value::<Record>(v).unwrap(), r);
        let none: Record = serde_json::from_value(serde_json::json!({})[SETTINGS_KEY].clone()).unwrap_or_default();
        assert_eq!(none, Record::default());
    }

    #[test]
    fn older_builds_settings_read_safely() {
        // 0.8.x kept unsent 15-minute marks as a number: they go once as kept seconds
        let r = tidy(serde_json::from_value(serde_json::json!({"off": false, "noticed": true, "pending": 3})).unwrap());
        assert_eq!((r.pending, r.kept_s, r.noticed), (0, 3 * MARK_S, true));
        let r = tidy(Record { pending: u32::MAX, kept_s: 5, ..Default::default() });
        assert_eq!(r.kept_s, MAX_S, "an edited settings.json: a week at most");
        // the tick off keeps nothing
        let r = tidy(Record {
            off: true,
            noticed: true,
            pending: 4,
            last: Some(RunTime { run: "0123abcd".into(), t: 9, v: "1.0.0".into(), ..Default::default() }),
            ..Default::default()
        });
        assert_eq!(r, Record { off: true, noticed: true, ..Default::default() });
        // damaged runs are dropped, long ones held to a week
        let bad = RunTime { run: "NOT HEX".into(), t: 5, v: "x".into(), ..Default::default() };
        let none = RunTime { run: "0123abcd".into(), t: 0, v: "x".into(), ..Default::default() };
        let long = RunTime { run: "0123abcd".into(), t: u64::MAX, v: "x".into(), ..Default::default() };
        let r = tidy(Record { earlier: vec![bad.clone(), none, long], last: Some(bad), ..Default::default() });
        assert_eq!(r.earlier.len(), 1);
        assert_eq!(r.earlier[0].t, MAX_S);
        assert_eq!(r.last, None);
    }

    #[test]
    fn a_runs_last_count_is_sent_again_until_it_gets_through() {
        let rt = |run: &str, t| RunTime { run: run.into(), t, v: "0.8.8".into(), ..Default::default() };
        let mut r = Record { last: Some(rt("aaaaaaaa", 937)), ..Default::default() };
        // the next start: the last run's count is to be sent again
        start_run(&mut r);
        assert_eq!((r.last.clone(), r.earlier.clone()), (None, vec![rt("aaaaaaaa", 937)]));
        // this run's counts: kept as they go, taken off as they get through
        r.last = Some(rt("bbbbbbbb", 300));
        assert!(!apply_acks(&mut r, &[Ack::Run("aaaaaaaa".into(), 600)]), "an older count of the run is not enough");
        assert!(apply_acks(&mut r, &[Ack::Run("aaaaaaaa".into(), 937)]));
        assert!(r.earlier.is_empty());
        r.last = Some(rt("bbbbbbbb", 600));
        apply_acks(&mut r, &[Ack::Run("bbbbbbbb".into(), 300)]);
        assert_eq!(r.last, Some(rt("bbbbbbbb", 600)), "a count that came back late does not take off a newer one");
        apply_acks(&mut r, &[Ack::Run("bbbbbbbb".into(), 600)]);
        assert_eq!(r.last, None);
        // kept seconds
        r.kept_s = 2700;
        apply_acks(&mut r, &[Ack::Kept(2700)]);
        assert_eq!(r.kept_s, 0);
        // runs offline for many starts: the newest few are kept
        let mut r = Record::default();
        for i in 0..20u64 {
            r.last = Some(rt(&format!("{:08x}", i + 1), 60));
            start_run(&mut r);
        }
        assert_eq!(r.earlier.len(), MAX_EARLIER);
        assert_eq!(r.earlier.last().unwrap().run, "00000014");
        // the same run never twice
        r.last = Some(rt("00000014", 120));
        start_run(&mut r);
        assert_eq!(r.earlier.len(), MAX_EARLIER);
        assert_eq!(r.earlier.iter().filter(|e| e.run == "00000014").count(), 1);
        assert_eq!(r.earlier.last().unwrap().t, 120);
    }

    #[test]
    fn a_stand_in_counter_only_on_this_computer() {
        assert_eq!(local_test_url(Some("http://127.0.0.1:8770/count")).as_deref(), Some("http://127.0.0.1:8770/count"));
        assert_eq!(local_test_url(Some("http://localhost:9/c")).as_deref(), Some("http://localhost:9/c"));
        assert_eq!(local_test_url(Some("https://example.com/count")), None);
        assert_eq!(local_test_url(Some("http://127.0.0.1.example.com/count")), None);
        assert_eq!(local_test_url(Some("http://localhost.example.com/")), None);
        assert_eq!(local_test_url(None), None);
    }

    #[test]
    fn what_is_sent() {
        assert_eq!(platform("windows", "x86_64", false), "windows");
        assert_eq!(platform("windows", "aarch64", false), "windows-arm64");
        assert_eq!(platform("macos", "aarch64", false), "macos-arm64");
        assert_eq!(platform("linux", "x86_64", false), "linux");
        assert_eq!(platform("linux", "aarch64", false), "linux-arm64");
        assert_eq!(platform("android", "aarch64", false), "android");
        assert_eq!(platform("", "wasm32", true), "web");
        assert_eq!(platform("freebsd", "x86_64", false), "other");
        assert_eq!(start_event("linux", "0.6.0"), "start/linux/0.6.0");
        assert_eq!(play_event("macos-arm64"), "play/macos-arm64");
        assert_eq!(query_value("start/linux/0.6.0-rc1+x"), "start%2Flinux%2F0.6.0-rc1%2Bx");
        assert_eq!(user_agent("0.6.0", "linux"), "OpenSGolf/0.6.0 (linux)");
        let c =
            |run: &str, n, t| Count { event: "play/linux".into(), version: "0.8.8".into(), run: run.into(), n, t, mp: [0; 3], ack: None };
        #[cfg(not(opensgolf_store))]
        {
            assert_eq!(
                count_url(&c("00ff00ff00ff00ff", 3, Some(937)), 255),
                format!("{COUNT_URL}?e=play%2Flinux&v=0.8.8&s=00ff00ff00ff00ff&n=3&t=937&rnd=ff")
            );
            assert!(count_url(&c(KEPT_RUN, 0, Some(1800)), 1).contains("&s=kept&n=0&t=1800&"));
            // the multiplayer seconds, only those played, only with play counts
            let mp = Count { mp: [300, 0, 120], ..c("00ff00ff00ff00ff", 3, Some(937)) };
            assert!(count_url(&mp, 1).contains("&t=937&mc=300&mw=120&rnd="));
            assert!(!count_url(&Count { mp: [5, 5, 5], ..c("00ff00ff", 0, None) }, 1).contains("&mc="));
            assert!(!count_url(&Count { event: start_event("linux", "0.8.8"), ..c("00ff00ff", 0, None) }, 1).contains("&t="));
        }
        // a run id is 16 hex digits (the counter takes 8 to 32), different each run
        assert_eq!(run_id_hex(0xab), "00000000000000ab");
        let (a, b) = (new_run_id(), new_run_id());
        assert_eq!(a.len(), 16);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_ne!(a, b);
    }

    #[test]
    fn multiplayer_seconds_are_part_of_the_runs_and_kept_with_it() {
        assert_eq!(mp_seconds([299.9, 0.0, 1e9], 600), [299, 0, 600], "whole seconds, never more than the run's");
        assert_eq!(mp_seconds([-3.0, 0.0, 0.0], 600), [0, 0, 0]);
        // kept in settings.json only when there are some, and older settings read as none
        let r = RunTime { run: "0123abcd".into(), t: 900, v: "0.9.11".into(), mp: [0; 3] };
        assert!(!serde_json::to_string(&r).unwrap().contains("mp"));
        let r = RunTime { mp: [600, 0, 0], ..r };
        assert_eq!(serde_json::from_value::<RunTime>(serde_json::to_value(&r).unwrap()).unwrap(), r);
        let old: RunTime = serde_json::from_value(serde_json::json!({"run": "0123abcd", "t": 900, "v": "0.9.10"})).unwrap();
        assert_eq!(old.mp, [0; 3]);
    }

    #[test]
    fn the_multiplayer_mode_being_played() {
        let mut a = crate::app::App::with_clock(std::path::PathBuf::new(), 4242);
        assert_eq!(a.playcount_mp_mode(), None, "single player");
        #[cfg(feature = "multiplayer")]
        {
            a.scen.goals.scenario = Some(format!("{}2026-W48", crate::mp::rival::ID_PREFIX));
            assert_eq!(a.playcount_mp_mode(), Some(2), "the weekly challenge");
            a.scen.goals.scenario = Some(crate::mp::league::game_id("abcd2345", 1));
            assert_eq!(a.playcount_mp_mode(), Some(1), "a league's plot");
            a.scen.goals.scenario = Some(format!("{}not a week", crate::mp::rival::ID_PREFIX));
            assert_eq!(a.playcount_mp_mode(), None);
        }
        // the frame's play goes to the mode played, and with the play count
        a.playcount.base = Mode::Log;
        a.playcount.run = "0123abcd".into();
        a.support.record.played = 100.0;
        a.playcount_frame();
        assert_eq!(a.playcount.mp, [0.0; 3], "no multiplayer mode: nothing");
        #[cfg(feature = "multiplayer")]
        {
            a.scen.goals.scenario = Some(format!("{}2026-W48", crate::mp::rival::ID_PREFIX));
            a.support.record.played = 160.0;
            a.playcount_frame();
            assert_eq!(a.playcount.mp, [0.0, 0.0, 60.0]);
            a.playcount.send_play(160, false);
            assert_eq!(a.playcount.record.last.as_ref().map(|l| l.mp), Some([0, 0, 60]));
        }
    }
}
