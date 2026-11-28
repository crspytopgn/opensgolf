//! What the exe draws for objects on the map (docs/PUBLISHER_EXE_NOTES.md, "Drawing objects"). The exe registers each building
//! sprite under a number at start-up, loading a different file per theme for the same number; its object drawing routine then
//! picks numbers by object kind and building level (the building's upgrade level; for the snack bar, more than 10 holes on
//! the course). Landmarks are numbered
//! 0x168 + their type. File names are the exe's, relative to Flics/, without ".flc".

/// Landmark art by landmark type (sprite 0x168 + type).
pub const LANDMARKS: [&str; 19] = [
    "Landmarks/Sundial",
    "Landmarks/Barn",
    "Landmarks/civilwarCannon",
    "Landmarks/Stone Two",
    "Landmarks/wmillA",
    "Landmarks/ParklandRock",
    "Landmarks/civilwarStatue",
    "Landmarks/LighthouseC",
    "Landmarks/Buddha",
    "Landmarks/Windmill",
    "Landmarks/equestrian",
    "Landmarks/Easter",
    "Landmarks/Pagoda",
    "Landmarks/LighthouseB",
    "Landmarks/ChineseHouse",
    "Landmarks/Tarpit",
    "Landmarks/Wtow2",
    "Landmarks/Radio Tower",
    "Landmarks/Red Oil Pump",
];

/// The port's Cut content extra (docs/CUT_CONTENT.md): landmark art in the game files that the shipped game never loads or
/// never draws, offered as landmarks of types `CUT_BASE` and up: (file, its palette, name key). The original has no price or
/// effect for them; the port charges `crate::extras::CUT_LANDMARK_UNITS` and golfers enjoy them as they enjoy a landmark's
/// sight (no landmark power: they are outside the type & 3 effects, which only types below 16 have).
pub const CUT_LANDMARKS: [(&str, &str, &str); 14] = [
    ("Scenic/CactusGroupB", "Scenic/CactusGroupBPal.pcx", "STR_CUT_CACTUS_GROUP"),
    ("Scenic/BlackPine", "Scenic/BlackPinePal.pcx", "STR_CUT_BLACK_PINE"),
    ("Landmarks/cathedral_ruins", "Landmarks/cathedral_ruinsPal.pcx", "STR_CUT_CATHEDRAL_RUINS"),
    ("Landmarks/gargoyle", "Landmarks/gargoylePal.pcx", "STR_CUT_GARGOYLE"),
    ("Scenic/Trop Boulder", "Scenic/TropBoulderPal.pcx", "STR_CUT_BOULDER"),
    ("Landmarks/Rock Garden", "Landmarks/Rockgarden_010001.pcx", "STR_CUT_ROCK_GARDEN"),
    ("Landmarks/SundialB", "Landmarks/sundialPal.pcx", "STR_CUT_GARDEN_SUNDIAL"),
    ("Landmarks/Chapel", "Landmarks/chapel_Palette.pcx", "STR_CUT_CHAPEL"),
    ("Scenic/Scenic_Cattails", "Scenic/CattailsPal.pcx", "STR_CUT_CATTAILS"),
    ("Scenic/Koi Pond", "Scenic/KoipondPal.pcx", "STR_CUT_KOI_POND"),
    ("Scenic/HotSpring", "Scenic/HotSpringPal.pcx", "STR_CUT_HOT_SPRING"),
    ("Scenic/Oasis", "Scenic/OasisPal.pcx", "STR_CUT_OASIS"),
    ("Scenic/desert fountain", "Scenic/DesertFountPal.pcx", "STR_CUT_DESERT_FOUNTAIN"),
    ("Scenic/LinksWaterRock", "Scenic/LinksWaterRockPal.pcx", "STR_CUT_SEA_ROCK"),
];

/// The first landmark type of `CUT_LANDMARKS` (after the original's 19).
pub const CUT_BASE: i32 = 19;

/// The landmark "types" the Buyable animals extra uses for its strip and tool: `ANIMAL_BASE` + the wildlife kind (0..8).
/// They never become landmark objects: placing one buys a pair of animals (sg_core::wildlife).
pub const ANIMAL_BASE: i32 = 100;

/// Whether golfers enjoy a landmark of type `t` (the original: types below 16; 16..18 are the eyesores). The cut landmarks
/// are scenic too.
pub fn lovely_landmark(t: i32) -> bool {
    t < 16 || (CUT_BASE..CUT_BASE + CUT_LANDMARKS.len() as i32).contains(&t)
}

/// The art of a landmark type: the sprite file (under Flics, without ".flc") and the palette to draw it in (under Flics),
/// for the original's types (palette 100 + type), the cut ones and the animals of the Buyable animals strip (the leader's
/// standing clip in its own palette, which the clip carries).
pub fn landmark_art(t: i32, theme: u8) -> Option<(&'static str, Option<&'static str>)> {
    if (0..LANDMARKS.len() as i32).contains(&t) {
        return Some((LANDMARKS[t as usize], crate::decor::palette_file(100 + t as u8, theme)));
    }
    if let Some(c) = CUT_LANDMARKS.get((t - CUT_BASE).max(-1) as usize) {
        return Some((c.0, Some(c.1)));
    }
    crate::wildlife::STAND_FILES.get((t - ANIMAL_BASE).max(-1) as usize).map(|&f| (f, None))
}

/// Building sprites 0x1b6..0x1fb by exe theme (0 Parkland, 1 Desert, 2 Tropical, 3 Links); None where a theme loads nothing.
const SPRITES: &[(u16, [Option<&str>; 4])] = &[
    // tournament TV tower and commentary booth (loaded by 0x442180); the same art in every theme
    (0x246, [Some("Tees/TVTowerSTATIC"), Some("Tees/TVTowerSTATIC"), Some("Tees/TVTowerSTATIC"), Some("Tees/TVTowerSTATIC")]),
    (0x247, [Some("Tees/TVTowerANIM"), Some("Tees/TVTowerANIM"), Some("Tees/TVTowerANIM"), Some("Tees/TVTowerANIM")]),
    (0x248, [Some("Tees/TVCommentSTATIC"), Some("Tees/TVCommentSTATIC"), Some("Tees/TVCommentSTATIC"), Some("Tees/TVCommentSTATIC")]),
    (0x249, [Some("Tees/TVCommentANIM"), Some("Tees/TVCommentANIM"), Some("Tees/TVCommentANIM"), Some("Tees/TVCommentANIM")]),
    (
        0x1b6,
        [
            Some("bldgs/park/buildDRL1"),
            Some("bldgs/desert/dbuildDRL1"),
            Some("bldgs/tropical/TropDrivingRShopL1"),
            Some("bldgs/links/DR_shop"),
        ],
    ),
    (
        0x1b7,
        [
            Some("bldgs/park/buildDRL2"),
            Some("bldgs/desert/desDRL2house"),
            Some("bldgs/tropical/TropDrivingRShopL2"),
            Some("bldgs/links/DRL2_shop"),
        ],
    ),
    (
        0x1b8,
        [
            Some("bldgs/park/plat_topDRL1"),
            Some("bldgs/desert/dplatDRL1"),
            Some("bldgs/tropical/TropDrivingRL1PlatTop"),
            Some("bldgs/links/DR_platform"),
        ],
    ),
    (
        0x1b9,
        [
            Some("bldgs/park/plat_baseDRL1"),
            Some("bldgs/desert/dplatDRL1base"),
            Some("bldgs/tropical/TropDrivingRL1PlatBase"),
            Some("bldgs/links/DR_platform_dirt"),
        ],
    ),
    (
        0x1ba,
        [
            Some("bldgs/park/100DRL1"),
            Some("bldgs/desert/d100DRL1"),
            Some("bldgs/tropical/TropDrivingR100L1"),
            Some("bldgs/links/DR_100sign"),
        ],
    ),
    (
        0x1bb,
        [
            Some("bldgs/park/200DRL1"),
            Some("bldgs/desert/d200DRL1"),
            Some("bldgs/tropical/TropDrivingR200L1"),
            Some("bldgs/links/DR_200sign"),
        ],
    ),
    (
        0x1bc,
        [
            Some("bldgs/park/300DRL1"),
            Some("bldgs/desert/d300DRL1"),
            Some("bldgs/tropical/TropDrivingR300L1"),
            Some("bldgs/links/DR_300sign"),
        ],
    ),
    (
        0x1bd,
        [
            Some("bldgs/park/flag_DRL1"),
            Some("bldgs/desert/dflagDRL1"),
            Some("bldgs/tropical/TropDrivingRflagL1"),
            Some("bldgs/links/DR_flag"),
        ],
    ),
    (
        0x1be,
        [
            Some("bldgs/park/wall_DRL1"),
            Some("bldgs/desert/dwallDRL1"),
            Some("bldgs/tropical/TropDrivingRfenceL1"),
            Some("bldgs/links/rockwall01"),
        ],
    ),
    (0x1bf, [Some("bldgs/park/buildDRL2_ANIM"), None, Some("bldgs/tropical/TROPDrivingRshopL2_ANIM"), Some("bldgs/links/DRL2_shop_ANIM")]),
    (
        0x1c0,
        [
            Some("bldgs/park/plat_topDRL2"),
            Some("bldgs/desert/desDRL2plattop"),
            Some("bldgs/tropical/TropDrivingRL2PlatTop"),
            Some("bldgs/links/DRL2_platform"),
        ],
    ),
    (
        0x1c1,
        [
            Some("bldgs/park/plat_baseDRL2"),
            Some("bldgs/desert/desDRL2plat"),
            Some("bldgs/tropical/TropDrivingRL2PlatBase"),
            Some("bldgs/links/DRL2_platform_base"),
        ],
    ),
    (
        0x1c2,
        [Some("bldgs/park/CartL1"), Some("bldgs/Desert/DESCartL1"), Some("bldgs/tropical/TropCartL1"), Some("bldgs/links/Cart_garageL1")],
    ),
    (
        0x1c3,
        [Some("bldgs/park/CartL2"), Some("bldgs/Desert/DESCartL2"), Some("bldgs/tropical/TropCartL2"), Some("bldgs/links/Cart_garageL2")],
    ),
    (0x1c4, [Some("bldgs/park/CartL1_ANIM"), None, None, None]),
    (
        0x1c5,
        [
            Some("bldgs/park/CartL1_base"),
            Some("bldgs/Desert/DESCartL1Base"),
            Some("bldgs/tropical/TropCartL1Base"),
            Some("bldgs/links/Cart_garageL1_dirt"),
        ],
    ),
    (
        0x1c6,
        [
            Some("bldgs/park/CartL2_base"),
            Some("bldgs/Desert/DESCartL2Base"),
            Some("bldgs/tropical/TropCartL2_Base"),
            Some("bldgs/links/Cart_garageL2_dirt"),
        ],
    ),
    (0x1c7, [Some("bldgs/park/CartL2_ANIM"), Some("bldgs/Desert/DESCartL2Anim"), None, None]),
    (
        0x1ca,
        [
            Some("Homes/Regular/Parkland/A/ParkHouseA"),
            Some("Homes/Regular/Desert/A/DesHouseA"),
            Some("Homes/Regular/Tropical/B/TropHouseB"),
            Some("Homes/Regular/Links/B/LinksHouseB"),
        ],
    ),
    (0x1c8, [Some("bldgs/park/sign"), Some("bldgs/desert/DESsign"), Some("bldgs/tropical/tropsign"), Some("bldgs/links/sign_links")]),
    (
        0x1c9,
        [
            Some("bldgs/park/Houseconst"),
            Some("bldgs/desert/DESHouseconst"),
            Some("bldgs/tropical/TropHouseconst"),
            Some("bldgs/links/Houseconst_links"),
        ],
    ),
    (0x1ce, [Some("bldgs/park/marL1"), Some("bldgs/Desert/HeliL1"), Some("bldgs/tropical/TropMarL1"), Some("bldgs/links/ChurchL1")]),
    (0x1cf, [Some("bldgs/park/marL2"), Some("bldgs/Desert/HelL2"), Some("bldgs/tropical/TropMarinaL2"), Some("bldgs/links/ChurchL2")]),
    (0x1d0, [Some("bldgs/park/parksnackL2_ANIM"), Some("bldgs/Desert/HelAnimLites"), None, None]),
    (
        0x1d1,
        [
            Some("bldgs/park/marL1_waterANIM"),
            Some("bldgs/Desert/HeliL1_Base"),
            Some("bldgs/tropical/TropMarL1_underwater"),
            Some("bldgs/links/ChurchL1_dirt"),
        ],
    ),
    (
        0x1d2,
        [
            Some("bldgs/park/marL2_waterANIM"),
            Some("bldgs/Desert/HelL2Base"),
            Some("bldgs/tropical/TropMarinaL2_underwater"),
            Some("bldgs/links/ChurchL2_dirt"),
        ],
    ),
    (0x1d3, [Some("bldgs/park/parksnackL1_ANIM"), Some("bldgs/Desert/HelAnimElev"), None, None]),
    (0x1d4, [Some("bldgs/park/tenL1"), Some("bldgs/Desert/spaL1"), Some("bldgs/tropical/TropSwimL1"), Some("bldgs/links/StableL1")]),
    (0x1d5, [Some("bldgs/park/tenL2"), Some("bldgs/Desert/spaL2"), Some("bldgs/tropical/TropSwimL2"), Some("bldgs/links/StableL2")]),
    (0x1d7, [None, Some("bldgs/Desert/spaL1_Base"), Some("bldgs/tropical/TropSwimL1anim"), Some("bldgs/links/StableL1_dirt")]),
    (0x1d8, [None, None, Some("bldgs/tropical/TropSwimL2_anim"), Some("bldgs/links/StableL2_dirt")]),
    (0x1d9, [None, Some("bldgs/Desert/DESCASL2Wheel"), None, Some("bldgs/links/StableL2_anim")]),
    (0x1da, [Some("bldgs/park/airL1"), Some("bldgs/Desert/DESCASL1"), Some("bldgs/tropical/TROPthemeL1"), Some("bldgs/links/CastleL1")]),
    (0x1db, [Some("bldgs/park/airL2"), Some("bldgs/Desert/DESCASL2"), Some("bldgs/tropical/TROPthemeL2"), Some("bldgs/links/CastleL2")]),
    (
        0x1dc,
        [
            Some("bldgs/park/airL1_ANIM"),
            Some("bldgs/Desert/DESCASL1Anim"),
            Some("bldgs/tropical/TROPthemeL1_BoatAnim"),
            Some("bldgs/links/CastleL2_ANIM"),
        ],
    ),
    (
        0x1dd,
        [
            Some("bldgs/park/airL1_base"),
            Some("bldgs/Desert/DESCASL1Base"),
            Some("bldgs/tropical/TROPthemeL2_LoopAnim"),
            Some("bldgs/links/CastleL1_dirt"),
        ],
    ),
    (
        0x1de,
        [
            Some("bldgs/park/airL2_base"),
            Some("bldgs/Desert/DESCASL2Base"),
            Some("bldgs/tropical/TROPthemeL1_CannonAnim"),
            Some("bldgs/links/CastleL2_dirt"),
        ],
    ),
    (0x1df, [Some("bldgs/park/airL2_ANIM"), Some("bldgs/Desert/DESCASL2Status"), Some("bldgs/tropical/TROPthemeL2_SprayAnim"), None]),
    (0x1e0, [Some("bldgs/park/ProsL1"), Some("bldgs/Desert/dproL1"), Some("bldgs/tropical/TropProshopL1"), Some("bldgs/links/Pro_shop")]),
    (
        0x1e1,
        [
            Some("bldgs/park/ProsL2"),
            Some("bldgs/Desert/desproL2"),
            Some("bldgs/tropical/TropProshopL2"),
            Some("bldgs/links/Links_ProshopL2"),
        ],
    ),
    (0x1e2, [Some("bldgs/park/ProsL2_ANIM01"), None, None, Some("bldgs/links/Pro_shop_anim")]),
    (
        0x1e3,
        [
            Some("bldgs/park/ProSL1_base"),
            Some("bldgs/Desert/dproL1Base"),
            Some("bldgs/tropical/TropProshopL1_base"),
            Some("bldgs/links/Pro_shop_dirt"),
        ],
    ),
    (0x1e4, [None, Some("bldgs/Desert/desproL2Anim"), Some("bldgs/tropical/TropProshopL1_anim"), Some("bldgs/links/Links_ProshopL2_dirt")]),
    (0x1e5, [Some("bldgs/park/ProsL2_ANIM02"), None, None, Some("bldgs/links/Links_ProshopL2_anim")]),
    (0x1e6, [Some("bldgs/park/HotelL1"), Some("bldgs/Desert/DESHotelL1"), Some("bldgs/Tropical/TropHotelL1"), Some("bldgs/links/HotelL1")]),
    (0x1e7, [Some("bldgs/park/HotelL2"), Some("bldgs/Desert/DESHoL2"), Some("bldgs/Tropical/TropHotelL2"), Some("bldgs/links/HotelL2")]),
    (0x1e8, [Some("bldgs/park/HotelL1_ANIM"), Some("bldgs/Desert/DESHotelL1Anim"), None, Some("bldgs/links/HotelL1_anim")]),
    (
        0x1e9,
        [
            Some("bldgs/park/HotelL1_base"),
            Some("bldgs/Desert/DESHotelL1base"),
            Some("bldgs/Tropical/TropHotelL1base"),
            Some("bldgs/links/HotelL1_dirt"),
        ],
    ),
    (
        0x1ea,
        [
            Some("bldgs/park/HotelL2_base"),
            Some("bldgs/Desert/DESHoL2Status"),
            Some("bldgs/Tropical/TropHotelL2_base"),
            Some("bldgs/links/HotelL2_dirt"),
        ],
    ),
    (
        0x1eb,
        [
            Some("bldgs/park/HotelL2_ANIM"),
            Some("bldgs/Desert/DESHoL2elev"),
            Some("bldgs/Tropical/TropHotelL2_ANIM"),
            Some("bldgs/links/HotelL2_anim"),
        ],
    ),
    (
        0x1ec,
        [Some("bldgs/park/parksnackL1"), Some("bldgs/Desert/DESSnack"), Some("bldgs/Tropical/TropSnackL1"), Some("bldgs/links/Pub_SQ")],
    ),
    (0x1ed, [Some("bldgs/park/parksnackL1_base"), Some("bldgs/Desert/DESSnackBase"), None, Some("bldgs/links/Pub_SQ_dirt")]),
    (
        0x1ee,
        [Some("bldgs/park/ParkSnackL2"), Some("bldgs/Desert/DESSnackL2"), Some("bldgs/Tropical/TropSnackL2"), Some("bldgs/links/PubL2")],
    ),
    (0x1ef, [Some("bldgs/park/ParkSnackL2_base"), None, Some("bldgs/Tropical/TropSnackL2_ANIM"), Some("bldgs/links/PubL2_dirt")]),
    // the putting green's flags (0x441b7c), drawn round its middle by 0x463180 case 6
    (
        0x191,
        [
            Some("bldgs/park/putt_whiteflag"),
            Some("bldgs/desert/dputt_whiteflag"),
            Some("bldgs/tropical/TROPPuttingG_whiteflag"),
            Some("bldgs/links/pg_whiteflag"),
        ],
    ),
    // the placement previews of the putting green (0x17c, upgraded 0x17d) and driving range (0x17e, 0x17f), one picture
    // a theme (frame = exe theme) in the palette DrivPuttPal (0x4430f3, preview 0x41a36a)
    (0x17c, [Some("Bldgs/1Putt"), Some("Bldgs/1Putt"), Some("Bldgs/1Putt"), Some("Bldgs/1Putt")]),
    (0x17d, [Some("Bldgs/2Putt"), Some("Bldgs/2Putt"), Some("Bldgs/2Putt"), Some("Bldgs/2Putt")]),
    (0x17e, [Some("Bldgs/1Drive"), Some("Bldgs/1Drive"), Some("Bldgs/1Drive"), Some("Bldgs/1Drive")]),
    (0x17f, [Some("Bldgs/2Drive"), Some("Bldgs/2Drive"), Some("Bldgs/2Drive"), Some("Bldgs/2Drive")]),
    (0x1f0, [Some("bldgs/park/puttL1"), Some("bldgs/desert/dputtL1"), Some("bldgs/tropical/TropPuttingGL1"), Some("bldgs/links/PG_hutL1")]),
    (
        0x1f1,
        [
            Some("bldgs/park/puttL1"),
            Some("bldgs/desert/dputtL1base"),
            Some("bldgs/tropical/TropPuttingGL1"),
            Some("bldgs/links/PG_hutL1_dirt"),
        ],
    ),
    (
        0x1f2,
        [
            Some("bldgs/park/shortflagL1"),
            Some("bldgs/desert/dshortflagL1"),
            Some("bldgs/tropical/TropPuttingGFlagL1"),
            Some("bldgs/links/PG_pflag"),
        ],
    ),
    (
        0x1f3,
        [Some("bldgs/park/puttL2"), Some("bldgs/desert/desputtL2"), Some("bldgs/tropical/TropPuttingGL2"), Some("bldgs/links/PG_hutL2")],
    ),
    (
        0x1f4,
        [
            Some("bldgs/park/puttL2_ANIM"),
            Some("bldgs/desert/desputtL2"),
            Some("bldgs/tropical/TropPuttingGL2_Anim"),
            Some("bldgs/links/PG_hutL2_dirt"),
        ],
    ),
    (
        0x1f5,
        [
            Some("bldgs/park/puttscenic"),
            Some("bldgs/desert/desputtL2scenic"),
            Some("bldgs/tropical/TropPuttingGL2_Scenic"),
            Some("bldgs/links/PG_scenicL2"),
        ],
    ),
    (0x1f6, [Some("bldgs/park/clubL1"), Some("bldgs/Desert/DESclubL1"), Some("bldgs/tropical/TropclubL1"), Some("bldgs/links/clubL1")]),
    (0x1f7, [Some("bldgs/park/clubL2"), Some("bldgs/Desert/DESclubL2"), Some("bldgs/tropical/TropclubL2"), Some("bldgs/links/clubL2")]),
    (0x1f8, [Some("bldgs/park/clubL2_base"), None, Some("bldgs/tropical/TropclubL2_base"), Some("bldgs/links/clubL2_dirt")]),
    (0x1f9, [Some("bldgs/park/clubL1_base"), Some("bldgs/Desert/DESclubL1base"), None, Some("bldgs/links/clubL1_dirt")]),
    (0x1fa, [Some("bldgs/park/clubL1_ANIM"), None, Some("bldgs/tropical/TropclubL1anim"), Some("bldgs/links/clubL1_ANIM")]),
    (0x1fb, [Some("bldgs/park/clubL2_ANIM"), None, Some("bldgs/tropical/TropclubL2_ANIM"), Some("bldgs/links/clubL2_ANIM")]),
];

pub fn sprite(id: u16, theme: u8) -> Option<&'static str> {
    SPRITES.iter().find(|s| s.0 == id).and_then(|s| s.1[theme.min(3) as usize])
}

/// One drawn layer of an object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub file: &'static str,
    /// Drawn on the ground under everything else (bases, dirt, water).
    pub flat: bool,
    /// Plays through its frames with the game tick.
    pub animated: bool,
}

fn add(out: &mut Vec<Layer>, id: u16, theme: u8, flat: bool, animated: bool) {
    if let Some(file) = sprite(id, theme) {
        out.push(Layer { file, flat, animated });
    }
}

/// The frame of a TV tower's or booth's animated sprite (n frames) standing at tile (a, b) on game tick `tick` (0x463180):
/// a step every second tick, offset by 13 a tile down and 7 a tile across, forwards through the frames, held on the last
/// for as long, back again, and held on the first, 4n steps in all.
pub fn tv_frame(tick: u32, a: i32, b: i32, n: i32) -> i32 {
    let n = n.max(1);
    let mut i = (tick as i64 / 2 + b as i64 * 13 + a as i64 * 7).rem_euclid(4 * n as i64) as i32;
    if i >= 2 * n {
        i = 3 * n - i - 1;
    }
    i.clamp(0, n - 1)
}

/// The layers the exe draws for a building of `kind` at `level` (0 or 1) in exe theme `theme`, bottom first.
pub fn building_layers(kind: i32, level: u16, theme: u8) -> Vec<Layer> {
    let mut v = Vec::new();
    let l = level.min(1);
    match kind {
        // the putting green is drawn piece by piece (`putting_green`); these are its hut's layers alone
        6 => {
            let k = if l == 1 { 3 } else { 0 };
            add(&mut v, 0x1f1 + k, theme, theme == 1 || theme == 3, true);
            add(&mut v, 0x1f0 + k, theme, false, false);
        }
        7 => {
            if l == 0 {
                if theme != 2 {
                    add(&mut v, 0x1ed, theme, true, false);
                }
                add(&mut v, 0x1ec, theme, false, true);
                if theme == 0 {
                    add(&mut v, 0x1d3, theme, false, true);
                }
            } else {
                if theme != 2 && theme != 1 {
                    add(&mut v, 0x1ef, theme, true, false);
                }
                add(&mut v, 0x1ee, theme, false, true);
                if theme == 2 {
                    add(&mut v, 0x1ef, theme, false, true);
                }
                if theme == 0 {
                    add(&mut v, 0x1d0, theme, false, true);
                }
            }
        }
        8 => {
            if theme != 2 || l == 0 {
                add(&mut v, 0x1e3 + l, theme, !(theme == 2 && l != 0), true);
            }
            add(&mut v, 0x1e0 + l, theme, false, true);
        }
        9 => {
            if theme != 0 && theme != 1 {
                add(&mut v, 0x1d7 + l, theme, theme != 2, true);
            }
            add(&mut v, 0x1d4 + l, theme, false, true);
        }
        11 => {
            add(&mut v, 0x1c5 + l, theme, true, false);
            add(&mut v, 0x1c2 + l, theme, false, false);
        }
        12 => {
            add(&mut v, 0x1d1 + l, theme, true, true);
            add(&mut v, 0x1ce + l, theme, false, false);
        }
        13 => {
            add(&mut v, 0x1e9 + l, theme, true, false);
            add(&mut v, 0x1e6 + l, theme, false, false);
        }
        14 => {
            add(&mut v, 0x1dd + l, theme, true, false);
            add(&mut v, 0x1da + l, theme, false, false);
        }
        15 => {
            let skip_base = (theme == 2 && l == 0) || (theme == 1 && l == 1);
            if !skip_base {
                add(&mut v, 0x1f9 - l, theme, true, false);
            }
            add(&mut v, 0x1f6 + l, theme, false, false);
            if theme != 1 {
                add(&mut v, 0x1fa + l, theme, false, true);
            }
        }
        // the TV tower (kind 0x11) and booth (0x12), 0x463180: the static sprite (0x246 / 0x248, frame 0) queued in a box
        // of 80 / 60 then the animated one (0x247 / 0x249, frame `tv_frame`) one step nearer, both in palette 0xb3
        17 => {
            add(&mut v, 0x246, theme, false, false);
            add(&mut v, 0x247, theme, false, true);
        }
        18 => {
            add(&mut v, 0x248, theme, false, false);
            add(&mut v, 0x249, theme, false, true);
        }
        _ => {}
    }
    v
}

/// One sprite of a driving range (0x463180 case 10), placed in map units from the range's anchor corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangePiece {
    pub file: &'static str,
    /// Map units (1024 a tile) right and down from the anchor tile's corner.
    pub at: (i32, i32),
    pub flat: bool,
    pub animated: bool,
    /// Added to the game tick for an animated piece's frame (the flags wave out of step).
    pub phase: u32,
    /// Drawn in this view whatever the camera (the flags).
    pub view: Option<i32>,
    /// Turned by the object's facing as well as the camera (the putting green's hut and bridge).
    pub turns: bool,
}

/// The sprites the exe draws for a driving range of `level` (0 or 1; the range is 5 + level tiles square) in exe theme
/// `theme` (0x463180 case 10). The exe walks the range's tiles, column i across and row j down: the first column has the
/// shop on its first tile (with the level 2 shop's animation, except in the Desert, which loads none) and a hitting
/// platform with its base on each tile after; the other columns have a fence piece at the tile's middle and at its corner on
/// the range's near and far edges. Every target green (`green(i, j)`, tile type 1) gets a waving flag, drawn in view 0, and
/// the tiles (2, 2), (4, 3) and (6, 4) the 100, 200 and 300 yard signs (column 6 is past even the larger range, so the 300
/// yard sign never shows). A flag or sign on an edge row stands where the
/// exe last placed the fence (the tile's corner on the edge), as it reuses that screen point.
pub fn driving_range(level: u16, theme: u8, green: &dyn Fn(i32, i32) -> bool) -> Vec<RangePiece> {
    let l = level.min(1) as i32;
    let n = 5 + l;
    let mut v = Vec::new();
    let put = |v: &mut Vec<RangePiece>, id: u16, at: (i32, i32), flat: bool, animated: bool, phase: u32, view: Option<i32>| {
        if let Some(file) = sprite(id, theme) {
            v.push(RangePiece { file, at, flat, animated, phase, view, turns: false });
        }
    };
    for i in 0..n {
        for j in 0..n {
            let mut at = (i * 1024 + 512, j * 1024 + 512);
            if i == 0 {
                if j == 0 {
                    put(&mut v, 0x1b6 + l as u16, at, false, false, 0, None);
                    if l != 0 && theme != 1 {
                        put(&mut v, 0x1bf, at, false, true, 0, None);
                    }
                } else {
                    let k = if l != 0 { 8 } else { 0 };
                    put(&mut v, 0x1b8 + k, at, false, false, 0, None);
                    put(&mut v, 0x1b9 + k, at, true, false, 0, None);
                }
            } else if j == 0 || j == l + 4 {
                let y = if j == 0 { 0 } else { (5 + l) * 1024 };
                put(&mut v, 0x1be, (i * 1024 + 512, y), false, false, 0, None);
                at = (i * 1024, y);
                put(&mut v, 0x1be, at, false, false, 0, None);
            }
            if green(i, j) {
                put(&mut v, 0x1bd, at, false, true, (i + j) as u32, Some(0));
            } else if j * 2 - 2 == i {
                let sign = match i {
                    2 => 0x1ba,
                    4 => 0x1bb,
                    6 => 0x1bc,
                    _ => continue,
                };
                put(&mut v, sign, at, false, false, 0, None);
            }
        }
    }
    v
}

/// The preview sprite the exe shows under the pointer while placing a putting green or driving range (0x41a36a, 0x41a3b5):
/// 0x17c (0x17e) plus the number already built, frame = exe theme, palette `PREVIEW_PALETTE`. One picture of the whole lot,
/// flags, hut and (upgraded) bridge included.
pub fn placement_preview(kind: i32, built: i32) -> Option<&'static str> {
    let base = match kind {
        6 => 0x17c,
        10 => 0x17e,
        _ => return None,
    };
    sprite(base + built.clamp(0, 1) as u16, 0)
}

/// The palette the placement previews are drawn in (`placement_preview`; the exe loads it at 0x442e4f).
pub const PREVIEW_PALETTE: &str = "Bldgs/DrivPuttPal.pcx";

/// The sprites the exe draws for a putting green of `level` (0 or 1; the green is 3 + level tiles square) in exe theme
/// `theme` (0x463180 case 6), in map units from the anchor tile's corner. The hut stands on the anchor tile (the lot's one
/// building tile, `Land::place_sized`): its base 0x1f1 (upgraded 0x1f4; flat in the Desert and Links, else queued behind
/// the hut), animated, and the hut 0x1f0 (0x1f3), both facing the object's way. Upgraded, the scenic bridge 0x1f5 stands on
/// tile (1, 1). Then 2 * tier + 3 waving flags 0x191 (tier 1, or 2 upgraded: 5 or 7) in view 0 round the middle of the
/// lot: flag n at screen offset (2r sin t, r cos t) with r = (n + 3) * 4 pixels and t = n * 0x4aaaaaa6 of a 2^32 turn,
/// taken back to the map through view 0's projection (0x42fb90: x = (a + b) / 32, y = 0.625 (b - a) / 32).
pub fn putting_green(level: u16, theme: u8) -> Vec<RangePiece> {
    let l = level.min(1) as i32;
    let k = if l != 0 { 3 } else { 0 };
    let mut v = Vec::new();
    let put = |v: &mut Vec<RangePiece>, id: u16, at: (i32, i32), flat: bool, animated: bool, view: Option<i32>, turns: bool| {
        if let Some(file) = sprite(id, theme) {
            v.push(RangePiece { file, at, flat, animated, phase: 0, view, turns });
        }
    };
    let anchor = (512, 512);
    put(&mut v, 0x1f1 + k, anchor, theme == 1 || theme == 3, true, None, true);
    put(&mut v, 0x1f0 + k, anchor, false, false, None, true);
    if l != 0 {
        put(&mut v, 0x1f5, (1024 + 512, 1024 + 512), false, true, None, true);
    }
    let mid = (3 + l) * 1024 / 2;
    for n in 0..(2 * (l + 1) + 3) {
        let t = (n as u32).wrapping_mul(0x4aaa_aaa6) as f64 / 4_294_967_296.0 * std::f64::consts::TAU;
        let r = ((n + 3) * 4) as f64;
        let (sx, sy) = (2.0 * r * t.sin(), r * t.cos());
        let (da, db) = (16.0 * sx - 25.6 * sy, 16.0 * sx + 25.6 * sy);
        put(&mut v, 0x191, (mid + da.round() as i32, mid + db.round() as i32), false, true, Some(0), false);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_frames_go_and_come_back() {
        // 4 frames: up 0..3, held on 3, down 3..0, held on 0, a step every second tick
        let seq: Vec<i32> = (0..16).map(|s| tv_frame(s * 2, 0, 0, 4)).collect();
        assert_eq!(seq, [0, 1, 2, 3, 3, 3, 3, 3, 3, 2, 1, 0, 0, 0, 0, 0]);
        // the tile offsets the phase: 13 a row, 7 a column
        assert_eq!(tv_frame(0, 1, 0, 4), tv_frame(14, 0, 0, 4));
        assert_eq!(tv_frame(0, 0, 1, 4), tv_frame(26, 0, 0, 4));
        assert_eq!(building_layers(17, 0, 0).iter().map(|l| l.animated).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn driving_range_pieces() {
        let v = driving_range(0, 0, &|i, j| (i, j) == (3, 2));
        // the shop on the anchor tile, four platforms with their bases down the first column
        assert_eq!(v[0].file, "bldgs/park/buildDRL1");
        assert_eq!(v[0].at, (512, 512));
        assert_eq!(v.iter().filter(|p| p.file == "bldgs/park/plat_topDRL1").count(), 4);
        assert_eq!(v.iter().filter(|p| p.file == "bldgs/park/plat_baseDRL1" && p.flat).count(), 4);
        // two fence pieces per column on each long edge
        assert_eq!(v.iter().filter(|p| p.file == "bldgs/park/wall_DRL1").count(), 16);
        // a flag on the green, the 100 and 200 yard signs (the 300's tile, column 6, lies past even the larger range)
        assert!(v.iter().any(|p| p.file == "bldgs/park/flag_DRL1" && p.at == (3 * 1024 + 512, 2 * 1024 + 512) && p.phase == 5));
        assert!(v.iter().any(|p| p.file == "bldgs/park/100DRL1" && p.at == (2 * 1024 + 512, 2 * 1024 + 512)));
        assert!(v.iter().any(|p| p.file == "bldgs/park/200DRL1"));
        assert!(!v.iter().any(|p| p.file == "bldgs/park/300DRL1"));
        assert!(!driving_range(1, 0, &|_, _| false).iter().any(|p| p.file == "bldgs/park/300DRL1"));
        // the level 2 shop's animation is not loaded in the Desert
        assert!(driving_range(1, 0, &|_, _| false).iter().any(|p| p.animated));
        assert!(!driving_range(1, 1, &|_, _| false).iter().any(|p| p.animated));
    }

    #[test]
    fn putting_green_pieces() {
        // the hut on the anchor tile, five flags round the middle of the 3 tile lot, all on the lot
        let v = putting_green(0, 0);
        assert_eq!((v[1].file, v[1].at), ("bldgs/park/puttL1", (512, 512)));
        let flags: Vec<_> = v.iter().filter(|p| p.file == "bldgs/park/putt_whiteflag").collect();
        assert_eq!(flags.len(), 5);
        assert!(flags.iter().all(|p| p.view == Some(0) && (0..3 * 1024).contains(&p.at.0) && (0..3 * 1024).contains(&p.at.1)));
        // the first flag stands straight below the middle on the screen: up b, down a
        assert_eq!(flags[0].at, (1536 - 307, 1536 + 307));
        // upgraded: the level 2 hut, the bridge on tile (1, 1), seven flags on the 4 tile lot
        let v = putting_green(1, 0);
        assert!(v.iter().any(|p| p.file == "bldgs/park/puttL2"));
        assert!(v.iter().any(|p| p.file == "bldgs/park/puttscenic" && p.at == (1536, 1536)));
        assert_eq!(v.iter().filter(|p| p.view == Some(0)).count(), 7);
        assert_eq!(placement_preview(6, 1), Some("Bldgs/2Putt"));
    }

    #[test]
    fn free_buildings_use_the_theme_art() {
        // Scotland's free building (kind 14) is the castle in Links; Phoenix's (kind 9) the spa in the Desert.
        assert!(building_layers(14, 0, 3).iter().any(|l| l.file == "bldgs/links/CastleL1"));
        assert!(building_layers(9, 0, 1).iter().any(|l| l.file == "bldgs/Desert/spaL1"));
        assert_eq!(building_layers(15, 0, 0)[0].file, "bldgs/park/clubL1_base");
    }
}
