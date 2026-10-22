//! The telephone ("Kontakte", dialog 404 `CTelefonManager`; KONTAKTE.md in
//! the RE repo): nine buttons, each calling up a sub-dialog — the
//! co-trainers' and goalkeeper trainers' market, fan service, lawyers,
//! rejecting a referee, a cooperation contract, the aliens' office, a
//! reporter, an enquiry at another club. The button stays pressed while
//! its sub-dialog or alert is up. The sub-dialogs are not built yet.

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{CONTACTS, NOTICE};
use a3_core::season::Rules;
use a3_files::rch::Dialog;

use super::hub::{self, CONTACTS_SYMBOL};
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::controls::options::NONE;
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
/// "ANSTOSS 3 - Telefon", with the manager, his club and the date.
const CAPTION: u16 = 2523;
const OK: u16 = 1;
const BUTTONS: u16 = 4975;
/// The fourth symbol the week's last day switches off besides 3 and 5.
const SHORT_TRAINING: usize = 4;
/// The cooperation contract's button and its refusals (§3.1): only in the
/// main country, not for an amateur department, not while one runs
/// ("… mit v1. Dieser läuft noch bis zum %s.").
const COOPERATION: usize = 5;
const NOT_MAIN_COUNTRY: u16 = 4458;
const AMATEURS: u16 = 4967;
const RUNNING: u16 = 4456;
/// The partner's number in the main country and the season the contract
/// ends in (team +0x92a, +0x92c); no partner.
const PARTNER: usize = 0x92a;
const UNTIL: usize = 0x92c;
const NO_PARTNER: u16 = 0xffff;

pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&CONTACTS) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The menu.
#[derive(Default)]
pub struct Contacts {
    /// Whether the whistle asks.
    whistle: bool,
}

impl Screen for Contacts {
    fn ids(&self) -> &[u16] {
        &[CONTACTS]
    }

    /// `FUN_0058cef6`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, CONTACTS_SYMBOL);
        if hub::last_day(cx) {
            hub::switch_off(cx, view, SHORT_TRAINING);
        }
        if let Some(season) = cx.state.season.as_ref() {
            set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        }
        if let Some(c) = control(dialog, BUTTONS) {
            view.set_chosen(c, NONE);
        }
        // Only the cooperation's refusals are built.
        let works = |target: Target| match target {
            Target::Option { control: c, item } if dialog.controls[c].id == Some(BUTTONS) => {
                item == COOPERATION && cooperation_refused(cx).is_some()
            }
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        if let Some(item) = hub::on_bar(cx.dialog, target) {
            let outcome = hub::bar_click(cx, view, item, CONTACTS_SYMBOL);
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        self.whistle = false;
        let Target::Option { control: c, item } = target else { return Outcome::Stay };
        if cx.dialog.controls[c].id != Some(BUTTONS) {
            return Outcome::Stay;
        }
        // The button stays pressed while what it calls up is shown. The
        // cooperation's check lets an expired contract go.
        if item == COOPERATION {
            expire_cooperation(cx);
        }
        if item == COOPERATION
            && let Some(args) = cooperation_refused(cx)
        {
            return Outcome::Popup(Request::with(NOTICE, args));
        }
        view.set_chosen(c, NONE);
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn. Anything else that
    /// closes lets the button up.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        if let Some(c) = control(cx.dialog, BUTTONS) {
            view.set_chosen(c, NONE);
        }
        Outcome::Stay
    }
}

/// Why the cooperation contract cannot be made, as its alert's text
/// (`FUN_0058d846`): not in the main country, an amateur department, a
/// contract still running — with its partner and the day it ends.
fn cooperation_refused(cx: &Cx) -> Option<Args> {
    let team = own_team(cx)?;
    let records = cx.state.records.as_ref()?;
    if i16::from(team.country()) != records.globals.main_country() {
        return Some(Args::Text(NOT_MAIN_COUNTRY));
    }
    if team.linked()[0] >= 0 {
        return Some(Args::Text(AMATEURS));
    }
    let seasons = i16::try_from(records.globals.seasons()).unwrap_or(i16::MAX);
    let until = team.get::<i16>(UNTIL);
    let partner = team.get::<u16>(PARTNER);
    if partner == NO_PARTNER || seasons >= until {
        return None;
    }
    Some(Args::Cooperation { partner, year: cx.game.world.start_year + i32::from(until - seasons) })
}

/// A contract whose last season has come is ended (`FUN_0069d8b2`).
fn expire_cooperation(cx: &mut Cx) {
    let seasons = cx.state.records.as_ref().map_or(0, |r| r.globals.seasons());
    let seasons = i16::try_from(seasons).unwrap_or(i16::MAX);
    if let Some(team) = super::squad::own_team_mut(cx)
        && team.get::<u16>(PARTNER) != NO_PARTNER
        && seasons >= team.get::<i16>(UNTIL)
    {
        team.put(PARTNER, NO_PARTNER);
    }
}

/// The running contract's alert: the partner with its article, the last
/// day "30.6." of the year it ends.
pub(crate) fn running_text(cx: &Cx, partner: u16, year: i32) -> String {
    let date =
        hub::date_text(cx.game, chrono::NaiveDate::from_ymd_opt(year, 6, 30).unwrap_or_default());
    let text = cx.game.text(RUNNING).replacen("%s", &date, 1);
    let main = cx.state.records.as_ref().map_or(0, |r| r.globals.main_country());
    let slot = u16::try_from(main).ok().and_then(|n| cx.game.world.slot_of(n)).unwrap_or(0);
    cx.game.with_club(&text, (slot, usize::from(partner)))
}
