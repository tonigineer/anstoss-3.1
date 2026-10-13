//! A talk with one of the players, as the manager booked it in his week
//! planner (`CEinzelgespraech`, dialog 461): one of twelve topics, and what
//! the player makes of it — his contentment with the trainer or his private
//! life, a reason of his flags, the team's mood, the manager's standing, a
//! suspension, an informer, a mole. Each topic answers with a text of the
//! game's, the player's name for its `%s`.

use crate::discipline::{Club, SUSPENDED};
use crate::mood::{self, LAST_REAL_ID, MoodTables};
use crate::random::Random;
use crate::records::{Player, Records};

/// The topics, in the order the dialog lists them.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Topic {
    Praise,
    BestMan,
    BuildUp,
    CriticiseMildly,
    CriticiseSharply,
    Suspend,
    Reconcile,
    Comfort,
    Hearten,
    Personal,
    Informer,
    Mole,
}

pub const TOPICS: [Topic; 12] = [
    Topic::Praise,
    Topic::BestMan,
    Topic::BuildUp,
    Topic::CriticiseMildly,
    Topic::CriticiseSharply,
    Topic::Suspend,
    Topic::Reconcile,
    Topic::Comfort,
    Topic::Hearten,
    Topic::Personal,
    Topic::Informer,
    Topic::Mole,
];

/// Bits of a player's record (`shown`, `flags`, `traits[1]`).
pub const INFORMER: u32 = 0x8000;
const MOLE: u32 = 0x10000;
const SHUNNED: u32 = 0x20000;
const BAD_TRAINER: u32 = 0x1000;
const GREAT_TRAINER: u32 = 0x80000;
const GOOD_MOOD: u32 = 0x40000;
const TEAM_GRIEVANCE: u32 = 0x80;
const PHLEGMATIC: u16 = 0x20;
const CHEERFUL: u16 = 0x4;
const MODEL_PRO: u16 = 0x100;
const RETIRING: u8 = 0x80;

/// The texts of each outcome, one drawn at random where there are several.
const LAUGHS: u16 = 2243;
const PRAISED_POORLY: [u16; 3] = [3324, 3336, 3337];
const PRAISED: [u16; 8] = [3328, 3329, 3330, 3331, 3332, 3333, 3334, 3335];
const BEST_MAN: [u16; 7] = [2185, 3317, 3318, 3319, 3320, 3321, 3322];
const BEST_MAN_SHRUG: u16 = 2186;
const BEST_MAN_LIE: [u16; 5] = [3323, 3324, 3325, 3326, 3327];
const BUILT_UP: [u16; 4] = [3328, 3338, 3339, 3340];
const BUILT_UP_NEEDLESSLY: [u16; 2] = [3341, 3342];
const CRITICISED_RIGHTLY_OLD: [u16; 3] = [3343, 3344, 3345];
const CRITICISED_RIGHTLY_YOUNG: [u16; 3] = [3346, 3347, 3348];
const CRITICISED_SHRUGGED: [u16; 4] = [2194, 3349, 3350, 3351];
const CRITICISED_PHLEGMATIC: [u16; 2] = [2195, 3352];
const CRITICISED_HURT: [u16; 2] = [2196, 3353];
const SHARPLY_RIGHT: [u16; 5] = [2197, 3354, 3355, 3356, 3357];
const SHARPLY_WRONG: [u16; 2] = [2198, 3358];
const SUSPENDED_BAD: [u16; 2] = [3361, 3362];
const SUSPENDED_ROUGHLY: [u16; 3] = [3363, 3364, 3365];
const SUSPENDED_UNFAIRLY: [u16; 6] = [3366, 3367, 3368, 3369, 3370, 3371];
const NOT_YET: u16 = 2200;
const SECOND_CHANCE: [u16; 3] = [2201, 3359, 3360];
const RECONCILE_HATES: u16 = 4115;
const RECONCILED: u16 = 4116;
const NOT_RECONCILED: u16 = 4117;
const NOTHING_TO_RECONCILE: u16 = 4118;
const COMFORTED: [u16; 4] = [4092, 4093, 4094, 4095];
const HEARTENED: u16 = 4096;
const NO_NEED: u16 = 4098;
const MADE_UP: u16 = 4099;
const GREAT_TRAINER_TEXT: [u16; 2] = [4100, 4101];
const CHAT: [u16; 2] = [4102, 4103];
const CHAT_GRUDGE: [u16; 2] = [4104, 4105];
const INFORMS: u16 = 4108;
/// The question an informer asks: a regular place for it.
pub const INFORMER_ASKS: u16 = 4109;
const INFORMS_FOR_A_PLACE: u16 = 4110;
const NO_PLACE_NO_INFORMER: u16 = 4111;
const REFUSES: u16 = 4112;
const OUTRAGED: u16 = 4113;
const NO_MORE_INFORMER: u16 = 4114;
const NOT_THE_MOLE: u16 = 4106;
const THE_MOLE: u16 = 4107;

/// A form at least this good is beyond reproach.
const FORM_MOST: i32 = 20;

/// What a topic comes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Said {
    /// The player's reaction, a text.
    Text(u16),
    /// He asks a question first (the informer's price); answer it with
    /// [`Talk::informer_answer`].
    Asks(u16),
    /// The team-mate to reconcile him with is to be chosen; go on with
    /// [`Talk::reconcile`].
    ChooseMate,
}

/// What a talk goes by, besides the records: the player's form and that of
/// the eleven, his age, how long he is still injured.
pub struct Talk<'a> {
    pub tables: &'a MoodTables,
    pub records: &'a mut Records,
    pub random: &'a mut Random,
    pub club: Club,
    /// The manager's slot.
    pub slot: usize,
    pub player: u16,
    /// Form by player id, for the squad.
    pub form: &'a dyn Fn(u16) -> i32,
    /// The eleven, by id.
    pub eleven: Vec<u16>,
    pub age: i32,
    /// Days his injury still keeps him out.
    pub injured: i32,
}

impl Talk<'_> {
    fn r(&self) -> &Player {
        self.records.player(self.player).expect("the player")
    }

    fn r_mut(&mut self) -> &mut Player {
        let id = usize::from(self.player);
        &mut self.records.players[id]
    }

    fn pick<const N: usize>(&mut self, texts: [u16; N]) -> u16 {
        texts[self.random.below(N as u32) as usize]
    }

    fn suspended(&self) -> bool {
        self.r().away_kind() & SUSPENDED != 0
    }

    fn trainer(&mut self, by: i32) {
        mood::change_trainer(self.r_mut(), by);
    }

    fn manager(&mut self) -> Option<&mut crate::records::Manager> {
        let slot = self.slot;
        self.records.managers.get_mut(slot)
    }

    /// The manager's authority, at his level.
    fn authority(&self) -> i32 {
        let m = self.records.managers.get(self.slot);
        m.map_or(0, |m| {
            let level = usize::from(m.level()).min(15);
            i32::from(m.abilities()[level][3])
        })
    }

    /// Whether he is one of the three worst of the eleven, and bad enough
    /// that criticism is deserved (`FUN_004900c9`): 0 then.
    fn deserves_criticism(&self) -> bool {
        let own = (self.form)(self.player);
        if own > 9 {
            return false;
        }
        let worse = self.eleven.iter().filter(|&&id| id != 0 && id != self.player);
        worse.filter(|&&id| (self.form)(id) < own).count() < 3
    }

    /// Whether he is not in the eleven, or weaker by two than the weakest
    /// of the others there (`FUN_0048ff9e`): then he is not the best man.
    fn would_be_a_lie(&self) -> bool {
        let others = self.eleven.iter().filter(|&&id| id != 0 && id != self.player);
        let weakest = others.map(|&id| (self.form)(id)).fold(FORM_MOST, i32::min);
        !self.eleven.contains(&self.player) || (self.form)(self.player) < weakest - 1
    }

    /// The team's mood moved by `by`, and what it does to the squad
    /// (`FUN_007147ce`, `FUN_00771701`): in a great mood the cheerful get
    /// "tolle Stimmung"; in an ordinary one nobody keeps that or a grudge
    /// against the team.
    fn team_mood(&mut self, by: i32) {
        let club = self.club;
        let Some(team) = team_mut(self.records, club) else { return };
        let mood = (i32::from(team.mood()) + by).clamp(0, 200) as u8;
        team.set_mood(mood);
        let squad = team.players();
        for id in squad {
            let Some(r) = self.records.players.get_mut(usize::from(id)) else { continue };
            if mood >= 151 {
                if r.traits()[1] & CHEERFUL != 0 {
                    self.tables.set_reasons(r, GOOD_MOOD);
                }
            } else if mood >= 50 {
                mood::clear_reasons(r, TEAM_GRIEVANCE);
                mood::clear_reasons(r, GOOD_MOOD);
            }
        }
    }

    /// How the public, the fans and the media see the manager, and the
    /// board's confidence (`FUN_00418065`).
    fn standing(&mut self, popularity: i32, confidence: i32) {
        let Some(m) = self.manager() else { return };
        let liked = m.popularity().map(|p| (i32::from(p) + popularity).clamp(0, 100) as u8);
        m.set_popularity(liked);
        m.set_confidence((i32::from(m.confidence()) + confidence).clamp(0, 100) as i16);
    }

    /// The worst he did this week or last, 1 to 4 (`FUN_00702e05`).
    fn offence(&self) -> u8 {
        let offences = self.r().offences();
        (1..=4).rev().find(|&k| offences & (0x11 << (k - 1)) != 0).unwrap_or(0)
    }

    /// Whether he would inform on his team-mates (`FUN_0048fb13`,
    /// `FUN_005168fb`).
    fn willing(&self) -> bool {
        let r = self.r();
        if r.shown() & SHUNNED != 0 {
            return false;
        }
        let level = mood::content_level(mood::parts(r));
        let years_left =
            i32::from(r.contract() & 0xfff) - i32::from(self.records.globals.start_year());
        let kind = if r.retiring() & RETIRING != 0
            || r.id() > LAST_REAL_ID
            || level >= 2
            || r.strength() >= 7
        {
            0
        } else if years_left <= 1 || self.age > 30 || r.traits()[1] & MODEL_PRO != 0 {
            2
        } else {
            1
        };
        match kind {
            1 => true,
            2 => i32::from(self.records.luck.of(self.player)) < self.authority(),
            _ => false,
        }
    }

    /// What he makes of the topic.
    pub fn say(&mut self, topic: Topic) -> Said {
        let form = (self.form)(self.player);
        let last_form = i32::from(self.r().last_form());
        // Criticism is weighed before it is known whether he listens.
        let authority = self.authority();
        let shrugs = matches!(topic, Topic::CriticiseMildly | Topic::CriticiseSharply)
            && (self.random.below(100) as i32) < authority * 6 + 10;
        let listens = matches!(topic, Topic::Suspend | Topic::Reconcile) || !self.suspended();
        if !listens {
            return Said::Text(LAUGHS);
        }
        let text = match topic {
            Topic::Praise => {
                if form < 11 {
                    self.trainer(-3);
                    self.pick(PRAISED_POORLY)
                } else {
                    self.trainer(3);
                    self.pick(PRAISED)
                }
            }
            Topic::BestMan => {
                if !self.would_be_a_lie() {
                    self.trainer(5);
                    self.pick(BEST_MAN)
                } else if form < 5 && self.random.below(5) == 0 {
                    BEST_MAN_SHRUG
                } else {
                    self.trainer(-5);
                    self.pick(BEST_MAN_LIE)
                }
            }
            Topic::BuildUp => {
                if form < 11 && form < last_form || form < 4 {
                    self.trainer(5);
                    self.pick(BUILT_UP)
                } else {
                    self.trainer(-5);
                    self.pick(BUILT_UP_NEEDLESSLY)
                }
            }
            Topic::CriticiseMildly => {
                if form < 8 {
                    self.trainer(3);
                }
                if self.deserves_criticism() {
                    if self.age > 21 {
                        self.pick(CRITICISED_RIGHTLY_OLD)
                    } else {
                        self.pick(CRITICISED_RIGHTLY_YOUNG)
                    }
                } else if form > last_form || shrugs {
                    self.trainer(-1);
                    self.pick(CRITICISED_SHRUGGED)
                } else if self.r().traits()[1] & PHLEGMATIC != 0 {
                    self.trainer(-3);
                    self.pick(CRITICISED_PHLEGMATIC)
                } else {
                    self.trainer(-5);
                    self.pick(CRITICISED_HURT)
                }
            }
            Topic::CriticiseSharply => {
                if form < 8 && last_form > form + 2 {
                    self.trainer(5);
                }
                if self.deserves_criticism() || shrugs {
                    self.pick(SHARPLY_RIGHT)
                } else {
                    self.trainer(-5);
                    self.pick(SHARPLY_WRONG)
                }
            }
            Topic::Suspend => self.suspend(form),
            Topic::Reconcile => return Said::ChooseMate,
            Topic::Comfort => {
                if self.r().contentment()[4] < 100 {
                    let by = self.random.below(12) as i32 - 2;
                    mood::change_private(self.r_mut(), by);
                    COMFORTED[match by {
                        ..0 => 0,
                        0 => 1,
                        1..4 => 2,
                        _ => 3,
                    }]
                } else {
                    LAUGHS
                }
            }
            Topic::Hearten => {
                let r = self.r();
                if r.morale() < 80 && r.match_bits() == 0 { HEARTENED } else { NO_NEED }
            }
            Topic::Personal => {
                let flags = self.r().flags();
                if self.random.below(100) < 10 {
                    if flags & BAD_TRAINER != 0 {
                        mood::clear_reasons(self.r_mut(), BAD_TRAINER);
                        MADE_UP
                    } else {
                        let text = GREAT_TRAINER_TEXT
                            [usize::from(!(flags & GREAT_TRAINER != 0 && self.age > 23))];
                        let tables = self.tables;
                        tables.set_reasons(self.r_mut(), GREAT_TRAINER);
                        text
                    }
                } else if flags & BAD_TRAINER == 0 {
                    CHAT[usize::from(self.random.below(2) == 0)]
                } else {
                    CHAT_GRUDGE[usize::from(self.random.below(2) == 0)]
                }
            }
            Topic::Informer => {
                if self.r().shown() & INFORMER != 0 {
                    let r = self.r_mut();
                    r.set_shown(r.shown() & !INFORMER);
                    NO_MORE_INFORMER
                } else if self.willing() {
                    match self.random.below(3) {
                        0 => {
                            let r = self.r_mut();
                            r.set_shown(r.shown() | INFORMER);
                            INFORMS
                        }
                        1 => return Said::Asks(INFORMER_ASKS),
                        _ => {
                            self.trainer(-10);
                            REFUSES
                        }
                    }
                } else {
                    self.trainer(-20);
                    OUTRAGED
                }
            }
            Topic::Mole => {
                if self.r().shown() & MOLE != 0 {
                    let r = self.r_mut();
                    r.set_shown(r.shown() & !MOLE);
                    THE_MOLE
                } else {
                    self.trainer(-50);
                    NOT_THE_MOLE
                }
            }
        };
        Said::Text(text)
    }

    /// Suspends him — it goes down the worse, the less he deserved it — or
    /// gives a suspended player another chance, if luck is with him.
    fn suspend(&mut self, form: i32) -> u16 {
        if self.suspended() {
            if self.records.luck.of(self.player) < 1 && self.random.below(10) != 0 {
                return NOT_YET;
            }
            let text = self.pick(SECOND_CHANCE);
            let r = self.r_mut();
            r.set_away_kind(r.away_kind() & !SUSPENDED);
            if form < 15 {
                self.trainer(3);
            }
            let injured = self.injured;
            let r = self.r_mut();
            r.set_dumbbell(match injured {
                7.. => 0,
                1.. => 2,
                _ => 3,
            });
            return text;
        }
        let text = match self.offence() {
            3 => self.pick(SUSPENDED_BAD),
            0 => {
                let text = self.pick(SUSPENDED_UNFAIRLY);
                self.standing(-15, -3);
                self.trainer(-80);
                self.team_mood(-40);
                text
            }
            _ => {
                let text = self.pick(SUSPENDED_ROUGHLY);
                self.standing(-8, -3);
                self.trainer(-20);
                self.team_mood(-20);
                text
            }
        };
        let r = self.r_mut();
        r.set_offences(r.offences() & !0x77);
        r.set_away_kind(r.away_kind() | SUSPENDED);
        r.set_dumbbell(0);
        let club = self.club;
        if let Some(team) = team_mut(self.records, club) {
            team.set_spirit((i32::from(team.spirit()) - 2).clamp(10, 50) as u8);
        }
        text
    }

    /// The informer's price answered: a regular place, or none and no
    /// informer.
    pub fn informer_answer(&mut self, yes: bool) -> u16 {
        if yes {
            let r = self.r_mut();
            r.set_shown(r.shown() | INFORMER);
            INFORMS_FOR_A_PLACE
        } else {
            NO_PLACE_NO_INFORMER
        }
    }

    /// Reconciling him with a team-mate (`FUN_0048f2fa`): there is nothing
    /// to make up unless one shuns him or he dislikes the other; whether it
    /// works goes by the manager's level.
    pub fn reconcile(&mut self, mate: u16) -> Option<u16> {
        if mate == 0 || mate == self.player {
            return None;
        }
        let r = self.r();
        if r.hates() == mate {
            return Some(RECONCILE_HATES);
        }
        let shunned = self.records.player(mate).is_some_and(|m| m.shown() & SHUNNED != 0);
        if !(shunned || r.dislikes().contains(&mate)) {
            return Some(NOTHING_TO_RECONCILE);
        }
        let level = self.records.managers.get(self.slot).map_or(0, |m| i32::from(m.level()));
        if (self.random.below(100) as i32) < level * 3 + 20 {
            reconcile(self.records, self.player, mate);
            Some(RECONCILED)
        } else {
            Some(NOT_RECONCILED)
        }
    }
}

fn team_mut(records: &mut Records, club: Club) -> Option<&mut crate::records::Team> {
    let first = usize::try_from(records.countries.first_team(club.country.into())).ok()?;
    records.teams.get_mut(first + usize::from(club.number))
}

/// Makes two players up (`FUN_00703d61`): the one no longer hates the
/// other, and each leaves the other off those he dislikes — as the
/// original does it, which zeroes the third friend instead of the third
/// dislike when one is taken out (`FUN_00703930`).
fn reconcile(records: &mut Records, player: u16, mate: u16) {
    if let Some(r) = records.players.get_mut(usize::from(player))
        && r.hates() == mate
    {
        r.set_hates(0);
    }
    for (who, other) in [(player, mate), (mate, player)] {
        let Some(r) = records.players.get_mut(usize::from(who)) else { continue };
        let mut dislikes = r.dislikes();
        let Some(at) = dislikes.iter().take_while(|&&d| d != 0).position(|&d| d == other) else {
            continue;
        };
        for i in at..2 {
            dislikes[i] = dislikes[i + 1];
        }
        r.set_dislikes(dislikes);
        let mut friends = r.friends();
        friends[2] = 0;
        r.set_friends(friends);
    }
}
