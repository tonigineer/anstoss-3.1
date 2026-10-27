//! The long-term training, the menu bar's fourth symbol (dialog 345,
//! `CTrainingLangfristig`; TRAINING-LANG.md in the RE repo): the season's
//! weeks, twenty-five at a time, each with its template, its player in
//! individual training, its kind and the team's condition and freshness;
//! a template chosen for the selected week, who the week's player is when
//! nobody is chosen, a double click on a week asking which player to watch
//! (dialog 281), who the detective watches, a training camp's food,
//! going out and control, secret training, the co-trainer planning the
//! empty weeks, and the last item of the file strip clearing the plan of
//! the weeks ahead, after asking.

use std::collections::HashMap;

use a3_core::lineup;
use a3_core::season::{ClubState, Rules};
use a3_core::training::{CAMP_WEEK, DAYS, SECRET, Week};
use a3_files::rch::Dialog;

use super::super::duties::short_name;
use super::super::hub::{self, LONG_TRAINING, LONG_TRAINING_SYMBOL};
use super::super::talk::PLAYER_CHOICE;
use super::super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use super::change as change_training;
use super::change_week;
use crate::controls::options::NONE;
use crate::pointer::{Grip, Target};
use crate::view::View;
use a3_core::cycle::dialogs::{NOTICE, QUESTION};

/// Controls by their ids in the dialog file.
const WEEKS: u16 = 4407;
const PAGES: u16 = 4403;
const TEMPLATES: u16 = 5035;
const RULE: u16 = 4405;
const DETECTIVE: u16 = 4406;
/// The camp's food, going out and control.
const CAMP: [u16; 3] = [4408, 4409, 4410];
const SECRET_BUTTON: u16 = 5344;
const CO_PLAN: u16 = 6465;
/// Saving, loading and restoring the templates and the plan.
const FILES: u16 = 5652;
/// Its last item, "Trainingsplan löschen"; its five others read and write
/// files, which the replica does not have yet.
const PLAN_ITEM: usize = 5;
/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9928;

/// Texts of the game.
const CAPTION: u16 = 3652;
const NOBODY: u16 = 5032;
const CAMP_WEEK_TAKEN: u16 = 3606;
const NO_CO_TRAINER: u16 = 3131;
const CO_TRAINER_WEEK: u16 = 4231;
/// "Soll der Trainingsplan wirklich gelöscht werden?"
const PLAN_DELETE: u16 = 3885;
/// The weeks' kinds: preparation, winter, no match, one, two or more.
const KINDS: [u16; 5] = [1439, 1440, 1441, 1442, 1443];
const NO_KIND: &str = "-";
const IN_CAMP: u16 = 4638;
/// The camp's own weeks' names, by template number above 100.
const CAMP_NAMES: [u16; 2] = [2686, 2687];

/// The weeks a page shows, and how many the season has.
const PAGE: usize = 25;
const SEASON_WEEKS: usize = 50;
/// A year calendar's camp days.
const CAMP_DAY: u8 = 1;
/// The camp's sliders run from 0 to 4.
const CAMP_MOST: i32 = 4;
/// The current week's text colour.
const CURRENT: [u8; 3] = [255, 0, 0];
/// The selected week's (`FUN_005a0dc0`).
const SELECTION: [u8; 3] = [0, 255, 0];
/// Players of the game, not the managers'.
const LAST_PLAYER: u16 = 17000;
/// The co-trainer plans the week himself (bit 0 of his settings).
const CO_TRAINER_PLANS: u16 = 1;
/// The secret training's states: secret, public.
const PUBLIC: usize = 1;

/// The bar takes the OK's place; the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&LONG_TRAINING) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    dialog.controls.insert(0, super::super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

#[derive(Default)]
pub struct LongTraining {
    /// The page shown, the week selected.
    page: usize,
    selected: usize,
    /// The players the detective's list names after "keinen", by id.
    watched: Vec<u16>,
    /// Whether the whistle asks.
    whistle: bool,
    /// The week the double click asked a player for, while 281 is up.
    pick: Option<usize>,
    /// Whether the strip's last item asked about clearing the plan.
    clearing: bool,
}

impl Screen for LongTraining {
    fn ids(&self) -> &[u16] {
        &[LONG_TRAINING]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        *self = Self::default();
        hub::open_bar(cx, view, LONG_TRAINING_SYMBOL);
        // On the week's last day the short-term training closes too.
        if hub::last_day(cx) {
            hub::switch_off(cx, view, hub::SHORT_TRAINING_SYMBOL);
        }
        let Some(season) = cx.state.season.as_ref() else { return };
        set(view, dialog, CAPTION_ID, hub::manager_title(game, season, CAPTION));
        let Some(manager) = season.manager() else { return };
        let Some(club) = own(cx) else { return };
        if let Some(combo) = control(dialog, RULE).and_then(|c| view.combo_mut(c)) {
            let texts = &game.rules.training.rule_texts;
            combo.items = texts.iter().map(|&t| game.text(t).to_owned()).collect();
            combo.selected = Some(usize::from(manager.individual_rule));
        }
        self.fill_detective(cx, view);
        let day = season.day;
        self.page = day / (PAGE * DAYS);
        self.selected = day / DAYS;
        if let Some(c) = control(dialog, PAGES) {
            view.set_chosen(c, self.page);
        }
        if let Some(list) = control(dialog, WEEKS).and_then(|c| view.list_mut(c)) {
            list.selection = Some(SELECTION);
        }
        if let Some(c) = control(dialog, TEMPLATES) {
            if let Some(options) = view.options_mut(c) {
                for (i, label) in options.labels.iter_mut().enumerate() {
                    *label = manager.templates.get(i).map_or_else(String::new, |t| t.name.clone());
                }
            }
            view.set_chosen(c, NONE);
        }
        if let Some(c) = control(dialog, FILES) {
            view.set_chosen(c, NONE);
        }
        for (&id, &rule) in CAMP.iter().zip(&manager.camp_rules) {
            if let Some(value) = control(dialog, id).and_then(|c| view.value_mut(c)) {
                value.range = (0, CAMP_MOST);
                value.value = i32::from(rule);
            }
        }
        let secret = club.tactics.marks(SECRET);
        if let Some(button) =
            control(dialog, SECRET_BUTTON).and_then(|c| view.controls[c].state.as_button_mut())
        {
            button.step = if secret { 0 } else { PUBLIC };
        }
        self.fill(cx, view);
        let works = |target: Target| match target {
            Target::Button(c) => {
                [SECRET_BUTTON, CO_PLAN].contains(&dialog.controls[c].id.unwrap_or(0))
            }
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(TEMPLATES) => true,
            Target::Option { control: c, .. } if dialog.controls[c].id == Some(PAGES) => true,
            _ => hub::on_bar(dialog, target).is_some_and(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, double: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, LONG_TRAINING_SYMBOL);
            if !matches!(outcome, Outcome::Stay) {
                keep_secret(cx, view);
            }
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        match target {
            // A week not over yet is selected; one that is keeps the
            // selection where it was. Its double click asks which player to
            // watch that week (dialog 281, `LBN_DBLCLK`).
            Target::Row { control: c, row } if id(c) == WEEKS => {
                let week = self.page * PAGE + row;
                let open = week_entry(cx, week).is_none_or(|w| !w.done);
                if open {
                    self.selected = week;
                }
                self.select(view);
                if double && open {
                    self.pick = Some(week);
                    return Outcome::Popup(Request::with(PLAYER_CHOICE, Args::WeekPlayer));
                }
            }
            Target::Option { control: c, item } if id(c) == PAGES => {
                self.page = item;
                self.fill(cx, view);
            }
            Target::Option { control: c, item } if id(c) == TEMPLATES => {
                view.set_chosen(c, NONE);
                return self.choose(cx, view, item);
            }
            // The strip's last item asks whether the plan may really go; its
            // choice falls back to none either way. Its five others save and
            // load to files, which the replica does not read yet.
            Target::Option { control: c, item } if id(c) == FILES => {
                view.set_chosen(c, NONE);
                if item == PLAN_ITEM {
                    self.clearing = true;
                    return Outcome::Popup(Request::with(QUESTION, Args::Text(PLAN_DELETE)));
                }
            }
            Target::Row { control: c, .. } if id(c) == RULE => {
                let rule = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0) as u8;
                change_manager(cx, |m| m.individual_rule = rule);
            }
            Target::Row { control: c, .. } if id(c) == DETECTIVE => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let watched = row.checked_sub(1).and_then(|r| self.watched.get(r)).copied();
                change_club(cx, |club| club.detective = watched.unwrap_or(0));
            }
            Target::SliderStep { control: c, .. } if CAMP.contains(&id(c)) => {
                let rules = CAMP.map(|id| {
                    control(dialog, id).and_then(|c| view.value(c)).map_or(0, |v| v.value as u8)
                });
                change_manager(cx, |m| m.camp_rules = rules);
            }
            Target::Button(c) | Target::Menu(Grip::Control(c)) if id(c) == SECRET_BUTTON => {
                if let Some(button) = view.controls[c].state.as_button_mut() {
                    button.step_on(2, matches!(target, Target::Menu(_)));
                }
            }
            Target::Button(c) if id(c) == CO_PLAN => return self.co_plan(cx, view),
            _ => {}
        }
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn. 281's choice names the
    /// week's player — "keiner" as 0 — and the weeks are filled again; a
    /// cancellation leaves the week as it was. The strip's question clears
    /// the templates of the weeks ahead of the current one, which keeps its
    /// own and every week's player (`FUN_0059ffc1`).
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if let Some(week) = self.pick.take() {
            if let Reply::Choice(player) = reply {
                change_week(cx, week, |w| w.player = u16::try_from(player).unwrap_or(0));
                self.fill(cx, view);
            }
            return Outcome::Stay;
        }
        if std::mem::take(&mut self.clearing) && reply == Reply::Ok {
            let current = cx.state.season.as_ref().map(|s| s.day / DAYS);
            if let Some(current) = current {
                for week in current + 1..SEASON_WEEKS {
                    change_week(cx, week, |w| w.template = 0);
                }
            }
            self.fill(cx, view);
            return Outcome::Stay;
        }
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

impl LongTraining {
    /// The page's weeks (`FUN_0059ee7e`): a week over shows what it had;
    /// the current one, in red, the team's figures as they are, which it
    /// keeps.
    fn fill(&mut self, cx: &mut Cx, view: &mut View) {
        let game = cx.game;
        let (Some(season), Some(club)) = (cx.state.season.as_ref(), own(cx)) else { return };
        let current = season.day / DAYS;
        let live = (
            lineup::average(club, lineup::condition) as u8,
            lineup::average(club, lineup::freshness) as u8,
        );
        let co_trainer = club.co_trainer
            && club.line_up_flags & CO_TRAINER_PLANS != 0
            && club.co_training.focus.iter().any(|&f| f > 0)
            && club.co_training.units > 0;
        let co_week = game
            .text(CO_TRAINER_WEEK)
            .replacen("%d", &club.co_training.units.to_string(), 1)
            .replacen("%d", &club.co_training.days.to_string(), 1);
        let (mut rows, mut inks, mut keys) = (Vec::new(), Vec::new(), Vec::new());
        let mut current_entry = None;
        for row in 0..PAGE {
            let k = self.page * PAGE + row;
            let entry = week_entry(cx, k).unwrap_or_default();
            let (template, player, figures) = if entry.done {
                let figures = (entry.condition, entry.freshness);
                (entry.template_name.clone(), entry.player_name.clone(), figures)
            } else {
                let p = club.status.iter().position(|s| s.id == entry.player && s.id != 0);
                let player =
                    p.map_or_else(String::new, |p| short_name(cx, &club.players[p], entry.player));
                let manager = season.manager();
                let template = match entry.template {
                    t if t > CAMP_WEEK => {
                        let name = CAMP_NAMES.get((t - CAMP_WEEK - 1) as usize);
                        game.text(name.copied().unwrap_or(0)).to_owned()
                    }
                    t if t > 0 => manager
                        .and_then(|m| m.templates.get((t - 1) as usize))
                        .map_or_else(String::new, |t| t.name.clone()),
                    _ if k == current && co_trainer => co_week.clone(),
                    _ => String::new(),
                };
                let figures = if k == current { live } else { (0, 0) };
                if k == current {
                    current_entry = Some(Week {
                        condition: live.0,
                        freshness: live.1,
                        template_name: if entry.template == 0 && co_trainer {
                            co_week.chars().take(29).collect()
                        } else {
                            entry.template_name.clone()
                        },
                        ..entry.clone()
                    });
                }
                (template, player, figures)
            };
            let mut kind = KINDS
                .get(usize::from(entry.kind))
                .map_or_else(|| NO_KIND.to_owned(), |&t| game.text(t).to_owned());
            if (0..DAYS).any(|d| in_camp(cx, k * DAYS + d)) {
                kind.push_str(game.text(IN_CAMP));
            }
            rows.push(format!(
                "{}\t{template}\t{player}\t{kind}\t{}\t{}\t",
                k + 1,
                figures.0,
                figures.1
            ));
            inks.push((k == current).then_some(CURRENT));
            keys.push(k);
        }
        if let Some(entry) = current_entry {
            change_week(cx, current, |w| *w = entry);
        }
        let Some(list) = control(cx.dialog, WEEKS).and_then(|c| view.list_mut(c)) else { return };
        list.rows = rows;
        list.inks = inks;
        list.keys = keys;
        self.select(view);
    }

    /// The selected week's row, on its own page.
    fn select(&self, view: &mut View) {
        let Some(list) = view.controls.iter_mut().find_map(|c| c.state.as_list_mut()) else {
            return;
        };
        list.selected = (self.selected / PAGE == self.page).then_some(self.selected % PAGE);
    }

    /// A template for the selected week, which the club's plan takes if it
    /// is one of the next six; then the next week is selected
    /// (`FUN_0059f654`).
    fn choose(&mut self, cx: &mut Cx, view: &mut View, template: usize) -> Outcome {
        let Some(season) = cx.state.season.as_ref() else { return Outcome::Stay };
        let (current, today) = (season.day / DAYS, season.day % DAYS);
        let mut week = self.selected;
        if week_entry(cx, week).is_some_and(|w| w.template > CAMP_WEEK) {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(CAMP_WEEK_TAKEN)));
        }
        let sessions = season.manager().and_then(|m| m.templates.get(template)).map(|t| t.week);
        change_week(cx, week, |w| w.template = template as i16 + 1);
        if let (Some(ahead), Some(sessions)) = (week.checked_sub(current), sessions)
            && ahead < a3_core::training::WEEKS
        {
            change_training(cx, |t| t.apply(ahead, &sessions, today));
        }
        for _ in 0..PAGE {
            week += 1;
            if week >= SEASON_WEEKS {
                week = PAGE.max(current);
                break;
            }
            if week >= current {
                break;
            }
        }
        self.selected = week;
        let page = usize::from(week >= PAGE);
        if page != self.page {
            self.page = page;
            if let Some(c) = control(cx.dialog, PAGES) {
                view.set_chosen(c, page);
            }
        }
        self.fill(cx, view);
        Outcome::Stay
    }

    /// The co-trainer fills the weeks from this one on that have no
    /// template, by his kind's odds; this one's plan from today
    /// (`FUN_0074ccbd`).
    fn co_plan(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let game = cx.game;
        let Some(club) = own(cx) else { return Outcome::Stay };
        if !club.co_trainer {
            return Outcome::Popup(Request::with(NOTICE, Args::Text(NO_CO_TRAINER)));
        }
        let records = cx.state.records.as_ref();
        let kind = own_key(cx)
            .and_then(|key| team_of(cx, key))
            .and_then(|t| records?.persons.get(usize::try_from(t.co_trainer()).ok()?))
            .map_or(0, |p| p.training_kind());
        let Some(season) = cx.state.season.as_mut() else { return Outcome::Stay };
        let (current, today) = (season.day / DAYS, season.day % DAYS);
        let tables = &game.rules.training;
        let mut first = true;
        let mut planned = Vec::new();
        for week in current..SEASON_WEEKS {
            let taken =
                season.manager().and_then(|m| m.weeks.get(week)).is_none_or(|w| w.template != 0);
            if taken {
                first = false;
                continue;
            }
            let template = tables.co_template(kind, season.random.below(tables.co_draw()));
            planned.push((week, template, first));
            first = false;
        }
        for (week, template, first) in planned {
            change_week(cx, week, |w| w.template = template as i16 + 1);
            let sessions = cx
                .state
                .season
                .as_ref()
                .and_then(|s| s.manager())
                .and_then(|m| m.templates.get(template))
                .map(|t| t.week);
            if let (true, Some(sessions)) = (first, sessions) {
                change_training(cx, |t| t.apply(0, &sessions, today));
            }
        }
        self.fill(cx, view);
        Outcome::Stay
    }

    /// "keinen Überwachen", then the squad's players in its order, the one
    /// watched selected.
    fn fill_detective(&mut self, cx: &Cx, view: &mut View) {
        let Some(club) = own(cx) else { return };
        let Some(c) = control(cx.dialog, DETECTIVE) else { return };
        let mut items = vec![cx.game.text(NOBODY).to_owned()];
        self.watched.clear();
        for (p, player) in club.players.iter().enumerate() {
            let id = club.status.get(p).map_or(0, |s| s.id);
            if id <= LAST_PLAYER {
                items.push(short_name(cx, player, id));
                self.watched.push(id);
            }
        }
        let selected = self.watched.iter().position(|&id| id == club.detective && id != 0);
        if let Some(combo) = view.combo_mut(c) {
            combo.items = items;
            combo.selected = Some(selected.map_or(0, |row| row + 1));
        }
    }
}

/// What the manager planned for a week of the season.
fn week_entry(cx: &Cx, week: usize) -> Option<Week> {
    cx.state.season.as_ref()?.manager()?.weeks.get(week).cloned()
}

/// Whether a day lies in a training camp of the club's year calendar: a
/// camp day, or after the last one marked as starting one (`FUN_0072f096`).
fn in_camp(cx: &Cx, day: usize) -> bool {
    let Some(records) = cx.state.records.as_ref() else { return false };
    let calendar = own_key(cx)
        .and_then(|key| team_of(cx, key))
        .and_then(|t| usize::from(t.human()).checked_sub(1))
        .and_then(|slot| records.calendars.get(slot));
    let Some(calendar) = calendar else { return false };
    if calendar.day(day).kind == CAMP_DAY {
        return true;
    }
    (0..day)
        .rev()
        .find(|&d| calendar.day(d).kind == CAMP_DAY)
        .is_some_and(|d| calendar.day(d).player != 0)
}

/// The secret training as the button has it, into the team's passing
/// word, as the screen is left (`FUN_0059edf6`).
fn keep_secret(cx: &mut Cx, view: &View) {
    let step = control(cx.dialog, SECRET_BUTTON)
        .and_then(|c| view.controls[c].state.as_button())
        .map_or(PUBLIC, |b| b.step);
    let game = cx.game;
    change_club(cx, |club| club.tactics.set_marks(SECRET, step != PUBLIC));
    let state = &mut *cx.state;
    let (Some(season), Some(records)) = (state.season.as_ref(), state.records.as_mut()) else {
        return;
    };
    let Some((slot, number)) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get(&(slot, number)) else { return };
    let Some(nation) = game.world.country(slot).map(|c| c.nation) else { return };
    if let Some(team) = u16::try_from(number).ok().and_then(|n| records.team_mut(nation, n)) {
        club.tactics.write(team);
    }
}

/// The manager's club, as the season follows it, and its key.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

fn own_key(cx: &Cx) -> Option<(usize, usize)> {
    cx.state.season.as_ref()?.manager()?.club
}

fn team_of<'a>(cx: &'a Cx, (slot, number): (usize, usize)) -> Option<&'a a3_core::records::Team> {
    let nation = cx.game.world.country(slot)?.nation;
    cx.state.records.as_ref()?.team(nation, u16::try_from(number).ok()?)
}

/// Changes the current manager, and his record with it: the rule for the
/// week's player, the camp's conditions.
fn change_manager(cx: &mut Cx, change: impl FnOnce(&mut a3_core::season::Manager)) {
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let current = season.current;
    let Some(manager) = season.managers.get_mut(current) else { return };
    change(manager);
    let (rule, camp) = (manager.individual_rule, manager.camp_rules);
    if let Some(records) = state.records.as_mut()
        && let Some(slot) = records.human_slot(current)
        && let Some(record) = records.managers.get_mut(slot)
    {
        record.set_individual_rule(rule);
        record.set_camp_rules(camp);
    }
}

/// Changes the club, and its extra data with it: the detective.
fn change_club(cx: &mut Cx, change: impl FnOnce(&mut ClubState)) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    change(club);
    let detective = club.detective;
    let Some(records) = state.records.as_mut() else { return };
    let Some(nation) = game.world.country(key.0).map(|c| c.nation) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    let human = usize::from(team.human()).checked_sub(1);
    if let Some(extra) = human.and_then(|slot| records.extras.get_mut(slot)) {
        extra.set_detective(detective);
    }
}
