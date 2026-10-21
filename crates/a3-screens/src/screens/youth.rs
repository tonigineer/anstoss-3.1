//! The youth on the transfer market: other clubs' youths without a
//! pre-contract (dialog 107, `CJugendspielerAbwerben`) and the
//! pre-contract their parents agree to (dialog 325,
//! `CVorvertragAbschliessen`; VERLAENGERUNG-ABWERBEN.md §3–4 in the RE
//! repo).

use std::collections::HashMap;

use a3_core::cycle::Request;
use a3_core::cycle::dialogs::{NOTICE, PRE_CONTRACT, YOUTH_POACHING};
use a3_core::youth;
use a3_files::rch::Dialog;

use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
const POACHING_LIST: u16 = 2124;
/// A human club's youths show four weeks after they were entered.
const HUMAN_WAIT: i32 = 28;
/// The main positions, keeper to striker, in the list's order.
const POSITIONS: std::ops::RangeInclusive<u8> = 1..=10;
/// The youth classes' names, C to A; the talents', huge to none.
const CLASSES: [u16; 3] = [1674, 1675, 1676];
const TALENTS: u16 = 4074;
const BLACK: [u8; 3] = [0, 0, 0];
const FULL_TEXT: u16 = 2168;
const PRE_CONTRACT_TEXT: u16 = 4189;
const AGREE: u16 = 4190;
const DISAGREE: u16 = 4191;

/// Both keep their file's title.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    for id in [YOUTH_POACHING, PRE_CONTRACT] {
        if let Some(dialog) = dialogs.get_mut(&id) {
            let caption = super::caption(dialog.frame, CAPTION_ID, dialog.title.clone());
            dialog.controls.insert(0, caption);
        }
    }
}

/// Other clubs' youths of the country without a pre-contract, by main
/// position; a click offers one a pre-contract. The list has no row menu.
#[derive(Default)]
pub struct Poaching {
    /// The youth 325 asks about, once it closes.
    asked: Option<u16>,
}

impl Screen for Poaching {
    fn ids(&self) -> &[u16] {
        &[YOUTH_POACHING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        self.asked = None;
        fill(cx, view);
        mark_inert(view, cx.dialog, |_| true);
    }

    /// `OnSelChange_IDC_JUGENDABWERBEN_LISTE`: the alerts keep the row
    /// selected; 325 refills the list.
    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let notice = |args| Outcome::Popup(Request::with(NOTICE, args));
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Row { control: c, row } if dialog.controls[c].id == Some(POACHING_LIST) => {
                let Some(slot) = view.list(c).and_then(|l| l.keys.get(l.scroll + row).copied())
                else {
                    return Outcome::Stay;
                };
                let (Some(records), Some(team)) = (cx.state.records.as_ref(), own_team(cx)) else {
                    return Outcome::Stay;
                };
                let Some(prospect) = youth::prospect_in(records, slot) else {
                    return notice(Args::Message(youth::CANNOT_TALK));
                };
                let (youths, more) = youth::youths(records, team.country(), team.number().into());
                if youths.len() + more >= youth::FULL {
                    return notice(Args::Text(FULL_TEXT));
                }
                if records.player(prospect.id).is_none_or(|r| r.club()[0] == 0) {
                    return notice(Args::Message(youth::WONT_MOVE));
                }
                self.asked = Some(prospect.id);
                Outcome::Popup(Request::with(PRE_CONTRACT, Args::Record(prospect.id)))
            }
            _ => Outcome::Stay,
        }
    }

    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        let Some(id) = self.asked.take() else { return Outcome::Stay };
        if reply == Reply::Ok
            && let Some(salary) = pre_contract(cx, id)
            && let Some(club) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))
            && let Some(records) = cx.state.records.as_mut()
        {
            youth::sign(records, id, club, salary);
        }
        fill(cx, view);
        Outcome::Stay
    }
}

/// The rows (`FUN_004d6bba`): the list's youths of other clubs of the
/// country, a human club's after four weeks, stable by main position;
/// each row keyed by its slot.
fn fill(cx: &Cx, view: &mut View) {
    let (game, dialog) = (cx.game, cx.dialog);
    let (Some(records), Some(team), Some(season)) =
        (cx.state.records.as_ref(), own_team(cx), cx.state.season.as_ref())
    else {
        return;
    };
    let today = i32::from(records.globals.day());
    let own = (team.country(), i16::from(team.number()));
    let human = |country: u8, number: i16| {
        u16::try_from(number)
            .ok()
            .and_then(|n| records.team(country.into(), n))
            .is_some_and(|t| t.human() != 0)
    };
    let shown: Vec<_> = youth::prospects(records)
        .into_iter()
        .filter(|p| (p.country, p.number) != own && p.country == team.country())
        .filter(|p| !human(p.country, p.number) || i32::from(p.day) <= today - HUMAN_WAIT)
        .filter_map(|p| Some((p, records.player(p.id)?.positions()[0])))
        .collect();
    let (mut rows, mut keys) = (Vec::new(), Vec::new());
    for position in POSITIONS {
        for &(p, _) in shown.iter().filter(|(_, main)| *main == position) {
            let Some(r) = records.player(p.id) else { continue };
            let Some((player, _, status)) = a3_core::load::any_player(records, &game.data, p.id)
            else {
                continue;
            };
            let talent = TALENTS + u16::try_from(r.talent()).unwrap_or(0);
            rows.push(
                [
                    game.text(CLASSES[youth::class(records, p.id)]).to_owned(),
                    super::duties::short_name(cx, &player, p.id),
                    player.age(season.date()).to_string(),
                    super::hub::abbreviation(game, position),
                    r.strength().to_string(),
                    game.text(talent).to_owned(),
                    game.world
                        .nation_codes
                        .get(usize::from(status.nations[0]))
                        .cloned()
                        .unwrap_or_default(),
                    game.club_name(Some(records), p.country, p.number),
                ]
                .join("\t"),
            );
            keys.push(p.slot);
        }
    }
    if let Some(list) = control(dialog, POACHING_LIST).and_then(|c| view.list_mut(c)) {
        list.inks = vec![Some(BLACK); rows.len()];
        list.rows = rows;
        list.keys = keys;
        list.scroll = 0;
        list.selected = None;
    }
}

/// The salary a youth's parents agree to with the manager's club
/// (`FUN_0066b648`), by his age today.
fn pre_contract(cx: &Cx, id: u16) -> Option<i32> {
    let records = cx.state.records.as_ref()?;
    let (player, ..) = a3_core::load::any_player(records, &cx.game.data, id)?;
    let age = player.age(cx.state.season.as_ref()?.date());
    Some(youth::salary(id, i32::try_from(age).ok()?, own_team(cx)?.level()))
}

/// The pre-contract a youth's parents agree to (dialog 325).
pub struct PreContract;

impl Screen for PreContract {
    fn ids(&self) -> &[u16] {
        &[PRE_CONTRACT]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Args::Record(id) = args {
            let name = cx.state.records.as_ref().and_then(|r| r.known_as(id)).unwrap_or_default();
            let salary = pre_contract(cx, id).unwrap_or(0);
            let text = control(dialog, PRE_CONTRACT_TEXT).map(|c| dialog.controls[c].text.clone());
            let text = text.unwrap_or_default().replacen("%s", &name, 1).replacen(
                "%s",
                &super::playerinfo::money(game, salary.into()),
                1,
            );
            set(view, dialog, PRE_CONTRACT_TEXT, text);
        }
        super::market::question_ball(view, dialog);
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if cx.dialog.controls[c].id == Some(AGREE) => {
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) if cx.dialog.controls[c].id == Some(DISAGREE) => {
                Outcome::Close(Reply::Cancel)
            }
            _ => Outcome::Stay,
        }
    }
}
