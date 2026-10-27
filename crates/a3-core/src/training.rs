//! A club's training: three sessions a day for six weeks, a unit each, as
//! the short-term training sets them one by one or a week at a time from
//! the manager's templates; a training camp and a team meeting before the
//! next match; and what the club knows of its opponents' set pieces
//! (TRAINING-KURZ.md in the RE repo).

use std::io;

use a3_files::exe::Exe;

use crate::records::Team;

pub const WEEKS: usize = 6;
pub const DAYS: usize = 7;
pub const SESSIONS: usize = 3;
const PLAN: usize = WEEKS * DAYS * SESSIONS;
/// A week's sessions, as a template holds them.
pub const WEEK: usize = DAYS * SESSIONS;
/// Nothing planned: "Frei".
pub const FREE: u8 = 11;
/// The units, 0 to 38; the tables have a fortieth, empty.
const UNIT_ENTRIES: usize = 40;
/// The highest unit; a read file's byte above this comes down to it.
pub const LAST_UNIT: u8 = 38;

/// The bit of the team's passing word that keeps its training secret
/// (team +0xd2): its opponents learn nothing of it.
pub const SECRET: u16 = 0x8000;
/// A week's template number above this is a training camp's own.
pub const CAMP_WEEK: i16 = 100;

/// The team record's flags for the next match: a training camp, a team
/// meeting.
pub const CAMP: u32 = 0x800;
pub const MEETING: u32 = 0x1000;

/// What a team can of the things its opponents may know, in the order the
/// game reckons their knowledge in.
pub const TRICKS: usize = 0;
pub const PRESSING: usize = 1;
pub const FREE_KICKS: usize = 2;
pub const THROW_INS: usize = 3;
pub const CORNERS: usize = 4;
pub const ANTI_OFFSIDE: usize = 5;
pub const SKILLS: usize = 6;
/// A skill's word for "unbekannt".
pub const UNKNOWN: u8 = 7;

/// How much a club knows of its opponents, by what watching players costs
/// it: below each of these one level less, 4 at most (`FUN_00627bd0`).
const LEVELS: [i32; 3] = [50_000, 100_000, 150_000];
const MOST_LEVEL: usize = 4;
/// The knowledge table's rows: a level's, of which the luck picks.
const KNOWLEDGE_ROW: usize = 31;
const LUCK_SPAN: usize = 30;

const UNIT_TEXTS_AT: u32 = 0x8a88d8;
const UNIT_PICTURES_AT: u32 = 0x8a8978;
const WORDS_AT: u32 = 0x88b354;
const WORDS: usize = 7;
const UNKNOWN_WORD_AT: u32 = 0x88b370;
const KNOWLEDGE_AT: u32 = 0x88b2b8;
const BALLS_AT: u32 = 0x88b420;
const UNKNOWN_BALL: u16 = 2282;
/// The unit popup's lists: how many units each, where, which.
const CATEGORY_COUNTS_AT: u32 = 0x88b6a0;
const CATEGORY_PLACES_AT: u32 = 0x88b6bc;
const CATEGORY_UNITS_AT: u32 = 0x88b818;
const CATEGORIES: usize = 7;
/// The rules for the week's player in individual training, by their texts.
const RULE_TEXTS_AT: u32 = 0x88b53c;
const RULES: usize = 6;
/// The co-trainer's odds of the manager's templates, a row of fourteen by
/// the kind of training he plans: cumulative, out of 100.
const CO_ODDS_AT: u32 = 0x8ab1f0;
/// The game's own 18 week templates, which the strip's third item puts
/// back (`DAT_008aad90`); the block of 18 templates a manager keeps and
/// a `plan\*.Woc` file holds.
const ORIGINAL_WEEKS_AT: u32 = 0x8aad90;
const BLOCK: usize = crate::records::TEMPLATES * crate::records::TEMPLATE_SIZE;
const CO_KINDS: usize = 7;
const CO_TEMPLATES: usize = 14;
/// Above the odds the co-trainer takes this template.
const CO_ELSE: usize = 1;
const CO_DRAW: u32 = 110;
const CO_ODDS_TOTAL: u32 = 100;
const CATEGORY_SIZE: u32 = 0x50;
const CATEGORY_ENTRY: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Training {
    plan: [u8; PLAN],
    /// The team record's flags: [`CAMP`], [`MEETING`] and the game's
    /// others.
    pub flags: u32,
}

impl Default for Training {
    fn default() -> Self {
        Self { plan: [FREE; PLAN], flags: 0 }
    }
}

impl Training {
    pub fn new(plan: [u8; PLAN], flags: u32) -> Self {
        Self { plan, flags }
    }

    pub fn plan(&self) -> [u8; PLAN] {
        self.plan
    }

    /// The unit of a session in the week shown (the plan's first).
    pub fn unit(&self, day: usize, session: usize) -> u8 {
        self.plan[day * SESSIONS + session]
    }

    pub fn set_unit(&mut self, day: usize, session: usize, unit: u8) {
        self.plan[day * SESSIONS + session] = unit;
    }

    /// A template's week into a week of the plan: into this one from
    /// today on, the days gone staying; into a later one whole
    /// (`FUN_0074cb28`).
    pub fn apply(&mut self, plan_week: usize, week: &[u8; WEEK], today: usize) {
        let first = if plan_week == 0 { today } else { 0 };
        for day in first..DAYS {
            let at = (plan_week * DAYS + day) * SESSIONS;
            let from = day * SESSIONS;
            self.plan[at..at + SESSIONS].copy_from_slice(&week[from..from + SESSIONS]);
        }
    }

    /// The week as it starts: nothing planned, then the week's template
    /// if one was chosen (`FUN_0074e870`).
    pub fn start_week(&mut self, week: Option<&[u8; WEEK]>) {
        self.plan[..WEEK].fill(FREE);
        if let Some(week) = week {
            self.apply(0, week, 0);
        }
    }

    /// Whether this week has anything planned: else the turn asks before
    /// it ends (`FUN_0074d706`).
    pub fn planned(&self) -> bool {
        self.plan[..WEEK].iter().any(|&unit| unit != FREE)
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    pub fn set(&mut self, flag: u32, on: bool) {
        self.flags = if on { self.flags | flag } else { self.flags & !flag };
    }
}

/// What a manager planned for a week of the season (the manager object's
/// +0x720 + week × 0x3a): whether it is over, what kind of week it is,
/// the team's condition and freshness, the template (1 more than its
/// number, 0 none, above [`CAMP_WEEK`] a camp's), the player in individual
/// training, and the names they had when the week ended.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Week {
    pub done: bool,
    pub kind: u8,
    pub condition: u8,
    pub freshness: u8,
    pub template: i16,
    pub player: u16,
    pub template_name: String,
    pub player_name: String,
}

/// What the co-trainer trains when he plans a week (the club's extra data
/// +0x2826): on how many days, how many units, and how many of each focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CoTraining {
    pub days: i16,
    pub units: i16,
    pub focus: [u8; 9],
}

/// A week template of the manager: its name and its sessions.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Template {
    pub name: String,
    pub week: [u8; WEEK],
}

/// The manager's own weeks as the training strip writes them to
/// `plan\*.Woc`: all 18 templates as his record holds them, each a name
/// of 30 bytes and 21 units — the block of `records::Manager` and the
/// file are the same 918 bytes (`FUN_0059f989`).
pub fn week_file(templates: &[Template]) -> Vec<u8> {
    use crate::records::{TEMPLATES, TEMPLATE_NAME, TEMPLATE_SIZE};
    let mut block = vec![0; TEMPLATES * TEMPLATE_SIZE];
    for (t, template) in templates.iter().take(TEMPLATES).enumerate() {
        let at = t * TEMPLATE_SIZE;
        let name: Vec<u8> = template
            .name
            .chars()
            .map(|c| a3_files::bitmap_font::to_cp1252(c).unwrap_or(b'?'))
            .collect();
        let n = name.len().min(TEMPLATE_NAME - 1);
        block[at..at + TEMPLATE_NAME][..n].copy_from_slice(&name[..n]);
        block[at + TEMPLATE_NAME..at + TEMPLATE_SIZE].copy_from_slice(&template.week);
    }
    block
}

/// A `plan\*.Woc` read in: nothing if it is not one whole block of 18
/// templates, and with every unit above [`LAST_UNIT`] brought down to it,
/// as the game does after its `fread` (`FUN_0059fb37`). The block is what
/// his record keeps, [`week_templates`] what his screens show.
pub fn read_week_file(bytes: &[u8]) -> Option<Vec<u8>> {
    use crate::records::{TEMPLATES, TEMPLATE_NAME, TEMPLATE_SIZE};
    (bytes.len() == TEMPLATES * TEMPLATE_SIZE).then(|| {
        bytes
            .iter()
            .enumerate()
            .map(|(i, &unit)| {
                if i % TEMPLATE_SIZE >= TEMPLATE_NAME { unit.min(LAST_UNIT) } else { unit }
            })
            .collect()
    })
}

/// The templates of such a block, clamped or not: names and sessions.
pub fn week_templates(block: &[u8]) -> Vec<Template> {
    use crate::records::{TEMPLATES, TEMPLATE_NAME, TEMPLATE_SIZE};
    (0..TEMPLATES)
        .filter_map(|t| {
            let at = t * TEMPLATE_SIZE;
            let entry = block.get(at..at + TEMPLATE_SIZE)?;
            let name = &entry[..TEMPLATE_NAME];
            Some(Template {
                name: a3_files::sav::cp1252(
                    &name[..name.iter().position(|&b| b == 0).unwrap_or(TEMPLATE_NAME)],
                ),
                week: entry[TEMPLATE_NAME..].try_into().ok()?,
            })
        })
        .collect()
}

/// The season's plan as the training strip writes it to `plan\*.pla`: the
/// fifty entries, each as his record keeps it — the block of
/// `records::Manager` at +0x86c and the file are the same 2900 bytes
/// (`FUN_0059fe1e`).
pub fn plan_file(weeks: &[Week]) -> Vec<u8> {
    use crate::records::{WEEKS, WEEK_SIZE};
    let mut block = vec![0; WEEKS * WEEK_SIZE];
    for (k, entry) in weeks.iter().take(WEEKS).enumerate() {
        let at = k * WEEK_SIZE;
        block[at] = u8::from(entry.done);
        block[at + 1] = entry.kind;
        block[at + 2] = entry.condition;
        block[at + 3] = entry.freshness;
        block[at + 4..at + 6].copy_from_slice(&entry.template.to_le_bytes());
        block[at + 6..at + 8].copy_from_slice(&entry.player.to_le_bytes());
        for (offset, len, name) in [(8, 30, &entry.template_name), (0x26, 20, &entry.player_name)]
        {
            let bytes: Vec<u8> =
                name.chars().map(|c| a3_files::bitmap_font::to_cp1252(c).unwrap_or(b'?')).collect();
            let n = bytes.len().min(len - 1);
            block[at + offset..at + offset + len][..n].copy_from_slice(&bytes[..n]);
        }
    }
    block
}

/// The fifty entries of a `plan\*.pla` read in, as the game copies them from
/// its buffer, each week's own done flag and individual player aside
/// (`FUN_005a01db`; what it does with the current week is the screen's).
pub fn read_plan_file(bytes: &[u8]) -> Option<Vec<Week>> {
    use crate::records::{WEEKS, WEEK_SIZE};
    (bytes.len() >= WEEKS * WEEK_SIZE).then(|| {
        (0..WEEKS)
            .map(|k| {
                let at = k * WEEK_SIZE;
                let name = |offset: usize, len: usize| {
                    let field = &bytes[at + offset..at + offset + len];
                    a3_files::sav::cp1252(
                        &field[..field.iter().position(|&b| b == 0).unwrap_or(len)],
                    )
                };
                Week {
                    done: bytes[at] != 0,
                    kind: bytes[at + 1],
                    condition: bytes[at + 2],
                    freshness: bytes[at + 3],
                    template: i16::from_le_bytes([bytes[at + 4], bytes[at + 5]]),
                    player: u16::from_le_bytes([bytes[at + 6], bytes[at + 7]]),
                    template_name: name(8, 30),
                    player_name: name(0x26, 20),
                }
            })
            .collect()
    })
}

/// What a team can, 0 to 5 each (7 shows as unknown), in the order of
/// [`TRICKS`] …: from its record's pressing, anti-offside and set-piece
/// bits (`FUN_006989c4`, `FUN_00698b3d`, `FUN_00698598`, `FUN_006987a6`,
/// `FUN_00698699`, `FUN_00698a9c`).
pub fn skills(team: &Team) -> [u8; SKILLS] {
    let bits = team.set_pieces();
    let field = |shift: u16| ((bits >> shift) & 7) as u8;
    [field(12), team.pressing(), field(0), field(6), field(3), team.anti_offside()]
}

/// How much a club knows of its next opponent, 0 to 4: by what watching
/// players costs it, everything for a national team, nothing while the
/// opponent trains in secret.
pub fn level(observation_cost: i32, national: bool, secret: bool) -> usize {
    if secret {
        0
    } else if national {
        MOST_LEVEL
    } else {
        1 + LEVELS.iter().take_while(|&&limit| observation_cost >= limit).count()
    }
}

/// The game's tables of training.
pub struct TrainingTables {
    /// Each unit's text and picture.
    pub unit_texts: Vec<u16>,
    pub unit_pictures: Vec<u16>,
    /// A skill's word, by its value, and "unbekannt".
    words: Vec<u16>,
    unknown_word: u16,
    /// A skill's picture in the picture view, by its value, and the
    /// unknown's.
    balls: Vec<u16>,
    /// Which skills a level knows, by a luck number and the skill.
    knowledge: Vec<u8>,
    /// The unit popup's lists.
    pub categories: Vec<Category>,
    /// The rules for the week's player in individual training.
    pub rule_texts: Vec<u16>,
    /// The game's own 18 week templates, which the training strip's third
    /// item puts back.
    pub originals: Vec<Template>,
    co_odds: Vec<u8>,
}

/// A list of the unit popup: where it starts in the popup, and its units
/// from the top, each with the unit its text is of.
pub struct Category {
    pub at: (i32, i32),
    pub units: Vec<(u8, u8)>,
}

impl TrainingTables {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let u16s = |at: u32, count: usize| -> io::Result<Vec<u16>> {
            Ok(exe.u32s(at, count)?.into_iter().map(|v| v as u16).collect())
        };
        let counts = exe.u32s(CATEGORY_COUNTS_AT, CATEGORIES)?;
        let places = exe.u32s(CATEGORY_PLACES_AT, 2 * CATEGORIES)?;
        let categories = (0..CATEGORIES)
            .map(|c| {
                let at = CATEGORY_UNITS_AT + c as u32 * CATEGORY_SIZE;
                let units = (0..counts[c])
                    .map(|i| {
                        let entry = exe.u16s(at + i * CATEGORY_ENTRY, 3)?;
                        Ok((entry[0] as u8, entry[2] as u8))
                    })
                    .collect::<io::Result<_>>()?;
                Ok(Category { at: (places[2 * c] as i32, places[2 * c + 1] as i32), units })
            })
            .collect::<io::Result<_>>()?;
        Ok(Self {
            unit_texts: u16s(UNIT_TEXTS_AT, UNIT_ENTRIES)?,
            unit_pictures: u16s(UNIT_PICTURES_AT, UNIT_ENTRIES)?,
            words: u16s(WORDS_AT, WORDS)?,
            unknown_word: exe.u32s(UNKNOWN_WORD_AT, 1)?[0] as u16,
            balls: u16s(BALLS_AT, WORDS)?,
            knowledge: exe.bytes(KNOWLEDGE_AT, (MOST_LEVEL + 1) * KNOWLEDGE_ROW)?.to_vec(),
            categories,
            rule_texts: u16s(RULE_TEXTS_AT, RULES)?,
            originals: week_templates(exe.bytes(ORIGINAL_WEEKS_AT, BLOCK)?),
            co_odds: exe.bytes(CO_ODDS_AT, CO_KINDS * CO_TEMPLATES)?.to_vec(),
        })
    }

    /// A unit's text.
    pub fn unit_text(&self, unit: u8) -> u16 {
        self.unit_texts.get(usize::from(unit)).copied().unwrap_or(0)
    }

    /// A unit's picture (`FUN_00732438`).
    pub fn unit_picture(&self, unit: u8) -> u16 {
        self.unit_pictures.get(usize::from(unit)).copied().unwrap_or(0)
    }

    /// The word for a skill's value (`FUN_005994d8`).
    pub fn word(&self, value: u8) -> u16 {
        match value {
            UNKNOWN => self.unknown_word,
            _ => self.words.get(usize::from(value)).copied().unwrap_or(0),
        }
    }

    /// The picture for a skill's value (`FUN_0059d525`).
    pub fn ball(&self, value: u8) -> u16 {
        match value {
            UNKNOWN => UNKNOWN_BALL,
            _ => self.balls.get(usize::from(value)).copied().unwrap_or(0),
        }
    }

    /// The template a co-trainer of a kind picks for a week, by a draw
    /// below 110 (`FUN_0074ccbd`).
    pub fn co_template(&self, kind: u8, draw: u32) -> usize {
        if draw >= CO_ODDS_TOTAL {
            return CO_ELSE;
        }
        let row = usize::from(kind) * CO_TEMPLATES;
        let odds = self.co_odds.get(row..row + CO_TEMPLATES).unwrap_or_default();
        odds.iter().take_while(|&&limit| u32::from(limit) <= draw).count()
    }

    /// What a co-trainer's template draw is out of.
    pub fn co_draw(&self) -> u32 {
        CO_DRAW
    }

    /// Whether a club knows an opponent's skill, at its level and by the
    /// opponent's luck number (`FUN_0059957a`).
    pub fn known(&self, level: usize, luck: usize, skill: usize) -> bool {
        let at = level * KNOWLEDGE_ROW + (luck + skill) % LUCK_SPAN;
        self.knowledge.get(at).is_some_and(|&known| known != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_leaves_the_days_gone() {
        let mut training = Training::default();
        training.set_unit(0, 0, 26);
        training.apply(0, &[16; WEEK], 1);
        assert_eq!(training.unit(0, 0), 26);
        assert_eq!(training.unit(0, 1), FREE);
        assert_eq!((training.unit(1, 0), training.unit(6, 2)), (16, 16));
        assert!(training.planned());
        assert!(!Training::default().planned());
    }

    #[test]
    fn a_week_file_holds_the_managers_templates_whole() {
        use crate::records::{TEMPLATES, TEMPLATE_NAME, TEMPLATE_SIZE};
        let templates = vec![
            Template { name: "gemischte Woche".to_owned(), week: [11; WEEK] },
            Template { name: "Ausdauer".to_owned(), week: [1; WEEK] },
        ];
        let block = week_file(&templates);
        assert_eq!(block.len(), TEMPLATES * TEMPLATE_SIZE);
        assert_eq!(read_week_file(&block).as_deref(), Some(block.as_slice()));
        let back = week_templates(&block);
        assert_eq!(back.len(), TEMPLATES);
        assert_eq!(&back[0], &templates[0]);
        assert_eq!(&back[1], &templates[1]);
        assert_eq!((back[2].name.as_str(), back[2].week), ("", [0; WEEK]));

        // A file's unit above the last one comes down to it; a short file
        // is no file at all.
        let mut rough = block.clone();
        rough[TEMPLATE_NAME] = 60;
        assert_eq!(week_templates(&read_week_file(&rough).unwrap())[0].week[0], LAST_UNIT);
        assert!(read_week_file(&block[..BLOCK - 1]).is_none());
    }

    #[test]
    #[ignore = "reads an installed game from $ANSTOSS3_DIR"]
    fn the_games_own_weeks_stand_in_the_program() {
        let dir = std::path::PathBuf::from(std::env::var_os("ANSTOSS3_DIR").expect("ANSTOSS3_DIR"));
        let exe = Exe::open(&dir.join("anstoss3.exe")).unwrap();
        let tables = TrainingTables::load(&exe).unwrap();
        assert_eq!(tables.originals.len(), crate::records::TEMPLATES);
        assert_eq!(tables.originals[0].name, "gemischte Woche");
        assert_eq!(
            tables.originals[0].week,
            [11, 11, 11, 15, 33, 11, 17, 27, 11, 10, 16, 11, 5, 26, 11, 18, 33, 11, 11, 11, 11]
        );
        // The block's 17th says just "Frei", though the texts call it
        // "Frei 1" — the strip copies the block, not the texts.
        assert_eq!(tables.originals[16].name, "Frei");
        assert!(tables.originals[17].week.iter().all(|&unit| unit == FREE));
    }

    #[test]
    fn the_level_grows_with_the_observation_list() {
        assert_eq!(level(0, false, false), 1);
        assert_eq!(level(50_000, false, false), 2);
        assert_eq!(level(149_999, false, false), 3);
        assert_eq!(level(150_000, false, false), 4);
        assert_eq!(level(0, true, false), 4);
        assert_eq!(level(200_000, true, true), 0);
    }
}
