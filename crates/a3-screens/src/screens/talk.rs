//! A talk with a player on the day the manager booked it (dialog 461,
//! `CEinzelgespraech`): who he is and how he stands — form, last match's
//! grade, morale — and twelve topics, one per talk, his reaction to it
//! below ([`a3_core::talk`]). "Versöhnung" first asks with whom (dialog
//! 281, `CTrainingEinzel`); asking him to inform may come back with his
//! price, a question.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::discipline::Club;
use a3_core::lineup;
use a3_core::talk::{INFORMER, Said, TOPICS, Talk as Topics};

use super::popups::QUESTION;
use super::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel, playerinfo, set};
use crate::controls::ControlState;
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

pub const TALK: u16 = a3_core::cycle::dialogs::TALK;
pub const MATES: u16 = 281;
const NAME: u16 = 5833;
const PORTRAIT: u16 = 1256;
const FORM: u16 = 5834;
/// The original shows this week's form again.
const FORM_BEFORE: u16 = 5835;
const GRADE: u16 = 5836;
const MORALE: u16 = 5837;
const REACTION: u16 = 1416;
const TOPIC_LIST: u16 = 5839;
const PLAYER_INFO: u16 = 5838;
const OK: u16 = 1;
/// Once a topic is chosen the button shows as OK, not as cancel.
const OK_LOOK: (u16, u16) = (1272, 1273);
const SUSPEND_ITEM: usize = 5;
const INFORMER_ITEM: usize = 10;
const MATES_TEXT: u16 = 3828;
const MATES_LIST: u16 = 3827;

/// Texts of the game.
const SECOND_CHANCE: u16 = 4442;
const NO_MORE_INFORMER: u16 = 4119;
const NOBODY: u16 = 1091;
const RECONCILE_WITH: u16 = 4139;

/// What the talk waits for.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Waiting {
    #[default]
    Nothing,
    Informer,
    Mate,
    Info,
}

/// The talk.
#[derive(Default)]
pub struct TalkScreen {
    player: u16,
    done: bool,
    waiting: Waiting,
}

/// The manager's club in the records' terms, his slot, and his club's
/// squad as the season follows it.
fn whose(cx: &Cx) -> Option<(Club, usize, (usize, usize))> {
    let season = cx.state.season.as_ref()?;
    let records = cx.state.records.as_ref()?;
    let key = season.manager()?.club?;
    let country = cx.game.world.country(key.0)?.nation;
    let club = Club { country, number: u16::try_from(key.1).ok()? };
    Some((club, records.human_slot(season.current)?, key))
}

impl TalkScreen {
    /// Says the topic, or what follows a question.
    fn react(
        &mut self,
        cx: &mut Cx,
        view: &mut View,
        say: impl FnOnce(&mut Topics) -> Option<Said>,
    ) -> Outcome {
        let Some((club, slot, key)) = whose(cx) else { return Outcome::Stay };
        let tables = &cx.game.rules.discipline.mood;
        let state = &mut *cx.state;
        let (Some(season), Some(records)) = (state.season.as_mut(), state.records.as_mut()) else {
            return Outcome::Stay;
        };
        let Some(squad) = season.clubs.get(&key) else { return Outcome::Stay };
        let today = season.date();
        let forms: HashMap<u16, i32> = (0..squad.players.len())
            .map(|p| (squad.status[p].id, lineup::form(lineup::fitness_of(&squad.fitness, p))))
            .collect();
        let me = squad.status.iter().position(|s| s.id == self.player);
        let eleven =
            squad.lineup.field.iter().map(|&p| p.map_or(0, |p| squad.status[p].id)).collect();
        let age = me
            .and_then(|p| squad.players[p].birthday)
            .map_or(0, |b| today.years_since(b).map_or(0, |y| y as i32));
        let injured = me.map_or(0, |p| squad.status[p].injured);
        let form = |id: u16| forms.get(&id).copied().unwrap_or(0);
        let mut talk = Topics {
            tables,
            records,
            random: &mut season.random,
            club,
            slot,
            player: self.player,
            form: &form,
            eleven,
            age,
            injured,
        };
        let said = say(&mut talk);
        super::discipline::refresh_morale(cx);
        match said {
            None => Outcome::Close(Reply::Ok),
            Some(Said::Text(text)) => {
                self.show(cx, view, text);
                Outcome::Stay
            }
            Some(Said::Asks(text)) => {
                self.waiting = Waiting::Informer;
                Outcome::Popup(Request::with(QUESTION, Args::TextFor(text, self.player)))
            }
            Some(Said::ChooseMate) => {
                self.waiting = Waiting::Mate;
                Outcome::Popup(Request::with(MATES, Args::Record(self.player)))
            }
        }
    }

    /// His reaction, and the talk over: no other topic, the button OK.
    fn show(&mut self, cx: &Cx, view: &mut View, text: u16) {
        let name =
            cx.state.records.as_ref().and_then(|r| r.known_as(self.player)).unwrap_or_default();
        set(view, cx.dialog, REACTION, cx.game.text(text).replacen("%s", &name, 1));
        self.done = true;
        if let Some(topics) = control(cx.dialog, TOPIC_LIST).and_then(|c| view.options_mut(c)) {
            topics.off = vec![true; TOPICS.len()];
        }
        if let Some(ok) = control(cx.dialog, OK)
            && let ControlState::Button(button) = &mut view.controls[ok].state
        {
            button.images = Some(OK_LOOK);
        }
    }
}

impl Screen for TalkScreen {
    fn ids(&self) -> &[u16] {
        &[TALK]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Record(player) = args else { return };
        *self = Self { player, ..Self::default() };
        let (game, dialog) = (cx.game, cx.dialog);
        let records = cx.state.records.as_ref();
        let record = records.and_then(|r| r.player(player));
        let name = records.and_then(|r| r.known_as(player)).unwrap_or_default();
        set(view, dialog, NAME, name);
        let portrait = record.and_then(|r| game.rules.words.portrait(r.face()));
        if let Some(c) = control(dialog, PORTRAIT) {
            view.set_picture(c, portrait.map(Picture::graphic));
        }
        let form = cx.state.season.as_ref().and_then(|season| {
            let club = season.clubs.get(&season.manager()?.club?)?;
            let p = club.status.iter().position(|s| s.id == player)?;
            Some(lineup::form(lineup::fitness_of(&club.fitness, p)))
        });
        let form = form.map_or_else(String::new, |f| f.to_string());
        set(view, dialog, FORM, form.clone());
        set(view, dialog, FORM_BEFORE, form);
        let grade = record.map_or(0, |r| r.grade() & 0xf);
        let grade =
            if grade == 0 { String::new() } else { format!("{:.1}", f32::from(grade) / 2.0) };
        set(view, dialog, GRADE, grade);
        set(view, dialog, MORALE, record.map_or_else(String::new, |r| r.morale().to_string()));
        set(view, dialog, REACTION, String::new());
        if let Some(topics) = control(dialog, TOPIC_LIST).and_then(|c| view.options_mut(c)) {
            topics.actions = true;
            if record.is_some_and(|r| r.away_kind() & a3_core::discipline::SUSPENDED != 0) {
                topics.labels[SUSPEND_ITEM] = game.text(SECOND_CHANCE).to_owned();
            }
            if record.is_some_and(|r| r.shown() & INFORMER != 0) {
                topics.labels[INFORMER_ITEM] = game.text(NO_MORE_INFORMER).to_owned();
            }
        }
        if let Some(ok) = control(dialog, OK)
            && let ControlState::Button(button) = &mut view.controls[ok].state
        {
            button.ready = vec![OK_LOOK.0, OK_LOOK.1];
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if ok || cancel {
            return Outcome::Close(Reply::Ok);
        }
        match target {
            Target::Button(b) if cx.dialog.controls[b].id == Some(PLAYER_INFO) => {
                let season = cx.state.season.as_ref();
                let club = season.and_then(|s| s.clubs.get(&s.manager()?.club?));
                let p = club.and_then(|c| c.status.iter().position(|s| s.id == self.player));
                let Some(p) = p else { return Outcome::Stay };
                self.waiting = Waiting::Info;
                Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Player(p)))
            }
            Target::Option { control: c, item } if cx.dialog.controls[c].id == Some(TOPIC_LIST) => {
                if self.done {
                    return Outcome::Stay;
                }
                let Some(&topic) = TOPICS.get(item) else { return Outcome::Stay };
                self.react(cx, view, |talk| Some(talk.say(topic)))
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        match std::mem::take(&mut self.waiting) {
            Waiting::Informer => self
                .react(cx, view, |talk| Some(Said::Text(talk.informer_answer(reply == Reply::Ok)))),
            Waiting::Mate => match reply {
                // Cancelled: the topics stay open.
                Reply::Cancel => Outcome::Stay,
                Reply::Choice(mate) => {
                    let mate = u16::try_from(mate).unwrap_or(0);
                    self.react(cx, view, |talk| talk.reconcile(mate).map(Said::Text))
                }
                _ => Outcome::Close(Reply::Ok),
            },
            // The player info goes on to its other pages.
            Waiting::Info => match reply {
                Reply::Goto(next) if playerinfo::PAGES_IN_ORDER.contains(&next) => {
                    self.waiting = Waiting::Info;
                    Outcome::Popup(Request::new(next))
                }
                _ => Outcome::Stay,
            },
            Waiting::Nothing => Outcome::Stay,
        }
    }
}

/// With whom to reconcile the player: nobody, or one of his team-mates
/// (dialog 281); a row chosen closes it with that one.
#[derive(Default)]
pub struct Mates {
    ids: Vec<u16>,
}

impl Screen for Mates {
    fn ids(&self) -> &[u16] {
        &[MATES]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let Args::Record(player) = args else { return };
        let records = cx.state.records.as_ref();
        let name = |id: u16| records.and_then(|r| r.known_as(id)).unwrap_or_default();
        set(
            view,
            cx.dialog,
            MATES_TEXT,
            cx.game.text(RECONCILE_WITH).replacen("%s", &name(player), 1),
        );
        let squad = whose(cx)
            .and_then(|(club, _, _)| records?.team(club.country, club.number))
            .map_or_else(Vec::new, |t| t.players());
        self.ids = std::iter::once(0).chain(squad.into_iter().filter(|&id| id != player)).collect();
        let rows = self
            .ids
            .iter()
            .map(|&id| if id == 0 { cx.game.text(NOBODY).to_owned() } else { name(id) })
            .collect();
        if let Some(list) = control(cx.dialog, MATES_LIST).and_then(|c| view.list_mut(c)) {
            *list = Default::default();
            list.rows = rows;
        }
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if ok {
            return Outcome::Close(Reply::Choice(0));
        }
        if let Target::Row { control: c, row } = target
            && cx.dialog.controls[c].id == Some(MATES_LIST)
        {
            let scroll = view.list(c).map_or(0, |l| l.scroll);
            let id = self.ids.get(scroll + row).copied().unwrap_or(0);
            return Outcome::Close(Reply::Choice(usize::from(id)));
        }
        Outcome::Stay
    }
}
