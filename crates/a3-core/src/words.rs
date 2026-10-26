//! What the screens call a player's things: the game's tables of text and
//! graphic ids for his traits, positions, foot, talent, abilities, injuries,
//! the years left on a contract and his portrait.

use std::io;

use a3_files::exe::Exe;

const TRAITS_AT: [u32; 2] = [0x8a8550, 0x8a8590];
const POSITIONS_AT: u32 = 0x8a84b8;
const FEET_AT: u32 = 0x88885c;
const TALENT_TEXTS_AT: u32 = 0x8a8538;
const TALENT_PICTURES_AT: u32 = 0x888aa4;
const FIELD_ABILITIES_AT: u32 = 0x8887e0;
const KEEPER_ABILITIES_AT: u32 = 0x888820;
const YEARS_AT: u32 = 0x8a8468;
const INJURIES_AT: u32 = 0x8a8758;
const PORTRAITS_AT: u32 = 0x888870;
/// First names by nation, a hundred each (the first fifty men's): the
/// partners' and children's names.
const FIRST_NAMES_AT: u32 = 0x8a23b0;
const FIRST_NAMES: usize = 800;
const PARTNERS_AT: u32 = 0x888c08;
const PARTNERSHIPS_AT: u32 = 0x888c10;
const HOBBIES_AT: u32 = 0x8a8c7e;
const HOBBIES: usize = 40;
const CONTENTMENT_AT: u32 = 0x8a8734;
/// The privates screen's formats (ERFOLGE.md §B.1): its caption, the
/// months until his second year, and the two purchase labels.
const PRIVATE_CAPTION_AT: u32 = 0x88664c;
const MONTH_ONE_AT: u32 = 0x886658;
const MONTH_MANY_AT: u32 = 0x886660;
const PURCHASE_AT: u32 = 0x88666c;
const PURCHASE_HOBBY_AT: u32 = 0x886684;
const REASONS_AT: u32 = 0x888b38;
const REASON_MASKS_AT: u32 = 0x888bfc;
const BITS: usize = 16;
const KEEPER_BITS: usize = 9;
const POSITIONS: usize = 11;
const FEET: usize = 4;
const TALENTS: usize = 5;
/// "ein Jahr" to "zehn Jahre", by the count.
const YEARS: usize = 11;
/// Kinds of injuries and, from 70, illnesses.
const INJURIES: usize = 100;
/// A face of three parts: 8 × 4 × 7 portraits.
const PORTRAITS: usize = 8 * 28;

pub struct Words {
    /// By bit of each word of character.
    pub traits: [Vec<u16>; 2],
    /// By position, keeper to striker, from 1.
    pub positions: Vec<u16>,
    /// By foot: 1 right, 2 left, 3 both.
    pub feet: Vec<u16>,
    /// By talent, 0 huge to 4 none: its word and its picture.
    pub talent_texts: Vec<u16>,
    pub talent_pictures: Vec<u16>,
    /// By bit: a field player's abilities, a keeper's.
    pub field_abilities: Vec<u16>,
    pub keeper_abilities: Vec<u16>,
    /// By count of years.
    pub years: Vec<u16>,
    /// By kind of injury.
    pub injuries: Vec<u16>,
    /// The portraits' graphics, by face.
    pub portraits: Vec<u16>,
    pub first_names: Vec<String>,
    /// A partner's kind, and how they stand.
    pub partners: Vec<u16>,
    pub partnerships: Vec<u16>,
    pub hobbies: Vec<u16>,
    /// How content he is, happy to wanting to leave; the reasons by bit of
    /// his record's flags, and which of them count first when he is least
    /// content with the trainer, the team or the club.
    pub contentment: Vec<u16>,
    pub reasons: Vec<u16>,
    pub reason_masks: Vec<u32>,
    /// "ANSTOSS 3 - Privates Toni Lubiniecki".
    pub private_caption: String,
    /// "1 Monat" and "%d Monate".
    pub month_one: String,
    pub month_many: String,
    /// "Kleine Eigentumswohnung (Preis 200.000 €)" and the hobby's
    /// shorter "Golfausrüstung kaufen (20.000 €)".
    pub purchase: String,
    pub purchase_hobby: String,
}

impl Words {
    pub fn load(exe: &Exe) -> io::Result<Self> {
        let ids = |at: u32, n: usize| -> io::Result<Vec<u16>> {
            Ok(exe.u32s(at, n)?.into_iter().map(|id| id as u16).collect())
        };
        Ok(Self {
            traits: [ids(TRAITS_AT[0], BITS)?, ids(TRAITS_AT[1], BITS)?],
            positions: ids(POSITIONS_AT, POSITIONS)?,
            feet: ids(FEET_AT, FEET)?,
            talent_texts: ids(TALENT_TEXTS_AT, TALENTS)?,
            talent_pictures: ids(TALENT_PICTURES_AT, TALENTS)?,
            field_abilities: ids(FIELD_ABILITIES_AT, BITS)?,
            keeper_abilities: ids(KEEPER_ABILITIES_AT, KEEPER_BITS)?,
            years: ids(YEARS_AT, YEARS)?,
            injuries: ids(INJURIES_AT, INJURIES)?,
            portraits: exe.u16s(PORTRAITS_AT, PORTRAITS)?,
            first_names: exe
                .u32s(FIRST_NAMES_AT, FIRST_NAMES)?
                .into_iter()
                .map(|p| exe.text_at(p).unwrap_or_default())
                .collect(),
            partners: exe.u16s(PARTNERS_AT, 4)?,
            partnerships: exe.u16s(PARTNERSHIPS_AT, 6)?,
            hobbies: exe.u16s(HOBBIES_AT, HOBBIES)?,
            contentment: ids(CONTENTMENT_AT, 4)?,
            reasons: ids(REASONS_AT, 32)?,
            reason_masks: exe.u32s(REASON_MASKS_AT, 3)?,
            private_caption: exe.text_at(PRIVATE_CAPTION_AT).unwrap_or_default(),
            month_one: exe.text_at(MONTH_ONE_AT).unwrap_or_default(),
            month_many: exe.text_at(MONTH_MANY_AT).unwrap_or_default(),
            purchase: exe.text_at(PURCHASE_AT).unwrap_or_default(),
            purchase_hobby: exe.text_at(PURCHASE_HOBBY_AT).unwrap_or_default(),
        })
    }

    /// The portrait of a face (the record's two bytes: `FUN_0055a57e`).
    pub fn portrait(&self, [a, b]: [u8; 2]) -> Option<u16> {
        let at = usize::from(a & 7) * 28 + usize::from((b >> 4) & 7) * 7 + usize::from(b & 7);
        self.portraits.get(at).copied()
    }
}
