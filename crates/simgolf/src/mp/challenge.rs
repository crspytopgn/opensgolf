//! The weekly rival-courses challenge: worked out from the ISO week's id alone, so every player gets the same start
//! without anything being fetched. multiplayer/worker/src/challenge.js is the same function for the leaderboard; both are
//! tested against multiplayer/worker/test/vectors.json.

use sg_core::data::Theme;
use sg_core::scenario::{Difficulty, Facts};

/// What the board sorts by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Cash,
    Fun,
    Members,
    Skill,
}

impl Measure {
    pub const ALL: [Measure; 4] = [Measure::Cash, Measure::Fun, Measure::Members, Measure::Skill];

    pub fn id(self) -> &'static str {
        match self {
            Measure::Cash => "cash",
            Measure::Fun => "fun",
            Measure::Members => "members",
            Measure::Skill => "skill",
        }
    }

    /// The week's score of a game's facts.
    pub fn score(self, f: &Facts) -> i64 {
        match self {
            Measure::Cash => f.cash,
            Measure::Fun => f.fun as i64,
            Measure::Members => f.members as i64,
            Measure::Skill => f.skill as i64,
        }
    }
}

/// Course themes in the seed's order.
pub const THEMES: [Theme; 4] = [Theme::Parkland, Theme::Desert, Theme::Tropical, Theme::Links];
/// Difficulties a challenge uses, in the seed's order (never Impossible: a week is short).
pub const DIFFICULTIES: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Moderate, Difficulty::Difficult];
/// Kinds of week, in the rotation's order (challenge.js KINDS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The usual start.
    Standard,
    /// Half the usual starting money.
    Budget,
    /// A course of at most NINE_HOLES holes.
    Nine,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Standard => "standard",
            Kind::Budget => "budget",
            Kind::Nine => "nine",
        }
    }
}

pub const KINDS: [Kind; 5] = [Kind::Standard, Kind::Budget, Kind::Standard, Kind::Nine, Kind::Standard];
/// The most holes a Nine week's course may have.
pub const NINE_HOLES: usize = 9;
/// The rotation's first week (the weeks before it were drawn from the hash alone).
pub const ROTATION_FROM: &str = "2026-W49";
/// A challenge ends at the end of this game year (2001 is the first).
pub const END_YEAR: i32 = 2001;

/// One week's challenge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    /// The ISO week, "YYYY-Www".
    pub id: String,
    pub measure: Measure,
    pub theme: Theme,
    pub difficulty: Difficulty,
    pub kind: Kind,
    /// The new game's seed (the land, the golfers' arrivals).
    pub seed: u32,
    pub end_year: i32,
}

/// FNV-1a, 32 bits.
pub fn fnv1a(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// (year, month 1-12, day 1-31) of a day count since 1970-01-01 (H. Hinnant's civil_from_days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Days since 1970-01-01 of a date (H. Hinnant's days_from_civil).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The ISO week id of a moment (seconds since 1970, UTC).
pub fn week_id(unix_s: i64) -> String {
    let days = unix_s.div_euclid(86_400);
    // 1970-01-01 was a Thursday: Monday is 1 .. Sunday 7
    let weekday = (days + 3).rem_euclid(7) + 1;
    let thursday = days + 4 - weekday;
    let (year, _, _) = civil(thursday);
    let week = (thursday - days_from_civil(year, 1, 1)) / 7 + 1;
    format!("{year}-W{week:02}")
}

/// Whether `id` looks like a week id ("YYYY-Www", weeks 01 to 53), as challenge.js `isWeekId`.
pub fn is_week_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 8
        && b[..4].iter().all(u8::is_ascii_digit)
        && &b[4..6] == b"-W"
        && b[6..].iter().all(u8::is_ascii_digit)
        && (1..=53).contains(&id[6..].parse::<u32>().unwrap_or(0))
}

/// The week before a week id (challenge.js previousWeekId of a moment in it).
pub fn previous_week(id: &str) -> String {
    week_id((monday_of(id) - 7) * 86_400)
}

/// The week after a week id.
pub fn next_week(id: &str) -> String {
    week_id((monday_of(id) + 7) * 86_400)
}

/// Days since 1970-01-01 of a week id's Monday (challenge.js mondayOf).
pub fn monday_of(id: &str) -> i64 {
    let year: i64 = id[0..4].parse().unwrap_or(1970);
    let week: i64 = id[6..].parse().unwrap_or(1);
    let jan4 = days_from_civil(year, 1, 4);
    // 1970-01-01 was a Thursday: Monday is 1 .. Sunday 7
    let day = (jan4 + 3).rem_euclid(7) + 1;
    jan4 - (day - 1) + 7 * (week - 1)
}

/// The challenge of a week id. From ROTATION_FROM the weeks take turns (challenge.js `challenge`): the measure every 4
/// weeks, the difficulty every 3 and the land type shifting so two weeks in a row never share it (48 weeks play every
/// combination once); the kind of week comes round every 5. The land itself (the seed) still comes from the hash.
pub fn challenge(id: &str) -> Challenge {
    let h = fnv1a(&format!("opensgolf-challenge/{id}"));
    let n = (monday_of(id) - monday_of(ROTATION_FROM)) / 7;
    let (measure, theme, difficulty, kind) = if n >= 0 {
        let n = n as usize;
        (Measure::ALL[n % 4], THEMES[(n + n / 12 + 1) % 4], DIFFICULTIES[n % 3], KINDS[n % 5])
    } else {
        (Measure::ALL[(h % 4) as usize], THEMES[((h >> 2) % 4) as usize], DIFFICULTIES[((h >> 4) % 3) as usize], Kind::Standard)
    };
    Challenge { id: id.to_string(), measure, theme, difficulty, kind, seed: h >> 8, end_year: END_YEAR }
}

/// The property a week is played on: before the rotation the land type's first one; from it, one of that type's
/// properties picked by the seed, so every property comes up (only the game needs it: the board scores the facts alone).
pub fn property(c: &Challenge) -> usize {
    let of_theme: Vec<usize> = (0..16).filter(|&i| sg_core::data::property_theme(i) == c.theme).collect();
    let pick = if monday_of(&c.id) >= monday_of(ROTATION_FROM) { (c.seed >> 4) as usize % of_theme.len().max(1) } else { 0 };
    of_theme.get(pick).copied().unwrap_or(0)
}

/// This week's challenge.
pub fn this_week(unix_s: i64) -> Challenge {
    challenge(&week_id(unix_s))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VECTORS: &str = include_str!("../../../../multiplayer/worker/test/vectors.json");

    #[test]
    fn week_ids() {
        assert!(is_week_id("2026-W47") && is_week_id("2020-W53") && is_week_id("2026-W01"));
        for id in ["2026-W00", "2026-W54", "2026-47", "2026-w47", "26-W47", "2026-W4x", "2026-W470"] {
            assert!(!is_week_id(id), "{id}");
        }
    }

    fn theme_id(t: Theme) -> &'static str {
        match t {
            Theme::Parkland => "parkland",
            Theme::Desert => "desert",
            Theme::Tropical => "tropical",
            Theme::Links => "links",
        }
    }

    fn difficulty_id(d: Difficulty) -> &'static str {
        ["easy", "moderate", "difficult", "impossible"][d.level() as usize]
    }

    #[test]
    fn matches_the_workers_vectors() {
        let v: serde_json::Value = serde_json::from_str(VECTORS).unwrap();
        for (day, id) in v["weeks"].as_object().unwrap() {
            let y: i64 = day[0..4].parse().unwrap();
            let m: u32 = day[5..7].parse().unwrap();
            let d: u32 = day[8..10].parse().unwrap();
            let noon = days_from_civil(y, m, d) * 86_400 + 12 * 3600;
            assert_eq!(week_id(noon), id.as_str().unwrap(), "{day}");
        }
        for c in v["challenges"].as_array().unwrap() {
            let ours = challenge(c["id"].as_str().unwrap());
            assert_eq!(ours.measure.id(), c["measure"].as_str().unwrap());
            assert_eq!(theme_id(ours.theme), c["theme"].as_str().unwrap());
            assert_eq!(difficulty_id(ours.difficulty), c["difficulty"].as_str().unwrap());
            assert_eq!(ours.seed as u64, c["seed"].as_u64().unwrap());
            assert_eq!(ours.kind.id(), c["kind"].as_str().unwrap());
            assert_eq!(ours.end_year as i64, c["endYear"].as_i64().unwrap());
        }
        assert_eq!(fnv1a(""), 0x811c_9dc5);
        assert_eq!(fnv1a("a"), 0xe40c_292c);
    }

    #[test]
    fn weeks_at_their_edges() {
        let at = |y, m, d, h: i64| days_from_civil(y, m, d) * 86_400 + h * 3600;
        // a week starts on Monday 00:00 UTC
        assert_eq!(week_id(at(2026, 11, 15, 23)), "2026-W46");
        assert_eq!(week_id(at(2026, 11, 16, 0)), "2026-W47");
        // years whose first days belong to the year before, and the other way round
        assert_eq!(week_id(at(2027, 1, 3, 12)), "2026-W53");
        assert_eq!(week_id(at(2027, 1, 4, 12)), "2027-W01");
        assert_eq!(week_id(at(2025, 12, 29, 12)), "2026-W01");
        for days in 0..20_000 {
            assert_eq!(civil(days_from_civil(civil(days).0, civil(days).1, civil(days).2)), civil(days));
        }
    }
}
