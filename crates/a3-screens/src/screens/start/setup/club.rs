//! The club and the national team of the trainer whose turn it is, for a
//! career with a club of one's choice.

use a3_core::cycle::dialogs::{CLUB, NATIONAL_TEAM};
use a3_core::newgame::{LEVELS, LOWER_LEVEL, Setup};
use a3_core::world::World;
use a3_files::rch::Dialog;

use crate::game::Game;
use crate::pointer::Target;
use crate::screens::start::{fill_in, fill_list};
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, ok_or_cancel};
use crate::view::View;

/// Controls by their ids in the dialog file.
const LEAGUE_LIST: u16 = 4055;
const CLUB_LIST: u16 = 4052;
const NATION_LIST: u16 = 4512;
/// "kein Land" in the list of national teams.
const NO_NATION: u16 = 3506;

/// A club for the trainer whose turn it is, for a career with a club of
/// one's choice.
pub struct Club;

impl Screen for Club {
    fn ids(&self) -> &[u16] {
        &[CLUB]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let setup = &mut cx.state.setup;
        fill_in(view, cx.dialog, setup.current + 1, &setup.name());
        let main = setup.main;
        show_leagues(view, cx.dialog, &cx.game.world, setup);
        show_clubs(view, cx.dialog, &cx.game.world, setup, (main, 0));
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        let setup = &mut cx.state.setup;
        match target {
            Target::Row { control, .. } if cx.dialog.controls[control].id == Some(LEAGUE_LIST) => {
                let picked = view
                    .list(control)
                    .and_then(|l| l.selected)
                    .and_then(|r| setup.leagues.get(r).copied());
                if let Some(league) = picked {
                    show_clubs(view, cx.dialog, &cx.game.world, setup, league);
                }
                Outcome::Stay
            }
            _ if ok => {
                let league = control(cx.dialog, LEAGUE_LIST)
                    .and_then(|i| view.list(i)?.selected)
                    .and_then(|r| setup.leagues.get(r).copied());
                let club = control(cx.dialog, CLUB_LIST)
                    .and_then(|i| view.list(i)?.selected)
                    .and_then(|r| setup.clubs.get(r).copied());
                // Without a club, the trainer goes without a national team.
                if let (Some((slot, league)), Some(club), Some(trainer)) =
                    (league, club, setup.trainers.get_mut(setup.current))
                {
                    trainer.club = Some((slot, club));
                    trainer.level = LEVELS.get(league).copied().unwrap_or(LOWER_LEVEL);
                }
                Outcome::Close(Reply::Ok)
            }
            _ if cancel => Outcome::Close(Reply::Cancel),
            _ => Outcome::Stay,
        }
    }
}

/// A national team besides the club, or none.
pub struct NationalTeam;

impl Screen for NationalTeam {
    fn ids(&self) -> &[u16] {
        &[NATIONAL_TEAM]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let setup = &mut cx.state.setup;
        fill_in(view, cx.dialog, setup.current + 1, &setup.name());
        show_national_teams(view, cx.dialog, cx.game, setup);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (ok, cancel) = ok_or_cancel(cx.dialog, target);
        if cancel {
            return Outcome::Close(Reply::Cancel);
        }
        if !ok {
            return Outcome::Stay;
        }
        let setup = &mut cx.state.setup;
        let nation = control(cx.dialog, NATION_LIST)
            .and_then(|i| view.list(i)?.selected)
            .and_then(|r| setup.nations.get(r).copied())
            .flatten();
        if let Some(trainer) = setup.trainer_mut() {
            trainer.national_team = nation;
        }
        Outcome::Close(Reply::Ok)
    }
}

/// The club screen's leagues: those of the main country, then the first
/// league of each extra country, named with its country.
fn show_leagues(view: &mut View, dialog: &Dialog, world: &World, setup: &mut Setup) {
    let mut rows = Vec::new();
    setup.leagues.clear();
    for slot in setup.countries().collect::<Vec<_>>() {
        for (index, league) in world.leagues_in_play(slot, setup.main).iter().enumerate() {
            rows.push(if slot == setup.main {
                league.name.clone()
            } else {
                let nation = world.country(slot).map_or(0, |c| c.nation);
                format!("{} ({})", league.name, world.nation(nation))
            });
            setup.leagues.push((slot, index));
        }
    }
    fill_list(view, dialog, LEAGUE_LIST, rows, &[0.0]);
}

/// The clubs of a league that no other trainer has taken, in the league's
/// order; the first is selected.
fn show_clubs(
    view: &mut View,
    dialog: &Dialog,
    world: &World,
    setup: &mut Setup,
    (slot, index): (usize, usize),
) {
    let (Some(country), Some(league)) =
        (world.country(slot), world.leagues_in_play(slot, setup.main).get(index))
    else {
        return;
    };
    setup.clubs = league.clubs.iter().copied().filter(|&c| !setup.club_taken(slot, c)).collect();
    let rows = setup.clubs.iter().map(|&c| country.clubs[c].name.clone()).collect();
    fill_list(view, dialog, CLUB_LIST, rows, &[0.0]);
}

/// "kein Land", then the national teams no other trainer coaches, by name.
fn show_national_teams(view: &mut View, dialog: &Dialog, game: &Game, setup: &mut Setup) {
    let teams = game.world.national_teams();
    let teams = teams.into_iter().filter(|&n| !setup.national_team_taken(n));
    setup.nations = std::iter::once(None).chain(teams.map(Some)).collect();
    let rows = setup
        .nations
        .iter()
        .map(|n| n.map_or(game.text(NO_NATION), |n| game.world.nation(n)).to_owned())
        .collect();
    fill_list(view, dialog, NATION_LIST, rows, &[0.0]);
}
