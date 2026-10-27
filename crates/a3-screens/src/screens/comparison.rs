//! The comparison with the next opponent, man for man, the menu bar's
//! second symbol (dialog 78, `CGegenueberstellungMann`;
//! GEGENUEBERSTELLUNG.md in the RE repo): both elevens on one pitch, the
//! own at the bottom, the opponent's mirrored at the top; how the teams
//! stand in will to win, mood, condition, freshness, cohesion and respect,
//! and what they count for in goal, defence, midfield and attack. Dragging
//! one of the eleven's defenders onto a striker of the opponent's, or a
//! midfielder onto a midfielder, has him mark that player man for man; a
//! line joins the two. "Aktionsbereich" shows where each of the eleven
//! acts instead: his box on the whole pitch, his area round it, darkened;
//! a click picks a player, whose area shows the raster with handles to
//! drag. A list names who goes forward at set pieces, two boxes whether
//! the strikers and the playmakers are marked.

use std::collections::HashMap;

use bevy_math::{Rect, Vec2};
use a3_core::lineup::{self, FIELD, PARTS};
use a3_core::marking;
use a3_core::records::{Records, Team};
use a3_core::season::{ClubState, Rules};
use a3_core::tactics::{MARK_PLAYMAKERS, MARK_STRIKERS};
use a3_files::rch::{Dialog, Kind};

use super::calendar::FIXTURE_AHEAD;
use super::duties::short_name;
use super::hub::{self, COMPARISON, COMPARISON_SYMBOL, filled_box};
use super::playerinfo;
use super::{Args, Cx, Outcome, Reply, Request, Screen, control, mark_inert, set};
use crate::controls::paint::PaintState;
use crate::controls::player_box::PlayerBox;
use crate::pointer::{Grip, Target};
use crate::view::View;

/// Controls by their ids in the dialog file: the boxes of the eleven and of
/// the opponent's, by their places.
const OWN_BOXES: u16 = 5620;
const THEIR_BOXES: u16 = 5631;
/// Will to win, mood, condition, freshness, cohesion and respect: each
/// side's figures, and the strips their bars stand in.
const OWN_FIGURES: [u16; FIGURES] = [5612, 5613, 5614, 5615, 5616, 5617];
const THEIR_FIGURES: [u16; FIGURES] = [1744, 1745, 1746, 1747, 1748, 1749];
const BARS: [u16; FIGURES] = [1710, 1701, 1703, 1700, 1706, 1708];
const FIGURES: usize = 6;
/// Goal, defence, midfield and attack.
const OWN_PARTS: [u16; PARTS] = [1730, 5661, 1732, 1733];
const THEIR_PARTS: [u16; PARTS] = [1734, 1735, 1736, 1737];
/// "Manndeckung" or "Aktionsbereich".
const MODE: u16 = 5618;
const MAN_MARKING: usize = 0;
const ACTION_AREAS: usize = 1;
/// The control the screen adds for the chosen player's action area
/// (`CGegenStatic`), over the boxes.
const AREA: u16 = 9921;
/// "Offensiv bei Standards".
const SET_PIECE: u16 = 5619;
/// "Stürmer manndecken", "Spielm. manndecken".
const MARKS: [(u16, u16); 2] = [(5662, MARK_STRIKERS), (5663, MARK_PLAYMAKERS)];
/// "Gegner": the opponent's squad, in its own dialog.
const SQUAD: u16 = 5664;
const OPPONENT_SQUAD: u16 = 476;
/// Its list and caption; the rows' inks, in the eleven, on the bench, in
/// the stands (`FUN_0069675f`).
const THEIR_LIST: u16 = 5261;
const THEIR_CAPTION: u16 = 9929;
const ELEVEN_INK: [u8; 3] = [0, 0, 0];
const BENCH_INK: [u8; 3] = [10, 10, 250];
const STANDS_INK: [u8; 3] = [69, 147, 11];
/// The OK, whose place the bar takes.
const OK: u16 = 1;
const CAPTION_ID: u16 = 9920;

/// Texts of the game.
const TITLE: u16 = 4241;
const NOBODY: u16 = 1091;

/// Where the boxes stand (`OnInitDialog`, `FUN_004ae502`): a spot's x from
/// this; a spot's y makes a row of the pitch, seven pixels for every eight,
/// down from the own row's top and up from the opponent's; the opponent's
/// spots mirrored across the pitch's width.
const LEFT: i32 = 14;
const TOP: i32 = 24;
const OWN_ROW: i32 = 210;
const THEIR_ROW: i32 = 196;
const ACROSS: i32 = 396;
const BOX: (i32, i32) = (99, 24);
/// The bars: the own red on the left, the opponent's blue on the right, as
/// high as the strip for the most.
const BAR_HEIGHT: i32 = 48;
const BAR_WIDTH: i32 = 5;
const OWN_INK: u32 = 0x0000c8;
const THEIR_INK: u32 = 0xc8_0000;
const LINE_INK: [u8; 3] = [0, 0, 0];
/// The action areas (`FUN_004af3f3`, `OnMouseMove_004b0265`): the eleven
/// stand on rows of five pixels for every three of a spot's y, from this;
/// an area reaches from the middle of the box, its corners kept on the
/// pitch; a handle moves a side only once dragged 30 pixels past the box,
/// and at most 240 from it; the reaches are kept in bytes, 160 across and
/// 240 along at most. The other players' areas are darkened.
const AREA_ROW: i32 = 39;
const RIGHT_MOST: i32 = 99 + 380;
const BOTTOM_MOST: i32 = 400;
const CORNER: i32 = 16;
const POINTER_RIGHT_MOST: i32 = 99 + ACROSS;
const PAST_THE_BOX: i32 = 30;
const FARTHEST: i32 = 240;
const MOST_ACROSS: u8 = 0xa0;
const MOST_ALONG: u8 = 0xf0;
const SHADE: u8 = 0x30;

/// The table of rows (0x6ab8 / 0x6cac of the dialog): the y of a spot's y.
fn rows(y: i16) -> i32 {
    let y = i32::from(y);
    y - y.div_euclid(8)
}

fn own_at((x, y): (i16, i16)) -> (i32, i32) {
    (i32::from(x) + LEFT, OWN_ROW + rows(y) + TOP)
}

fn their_at((x, y): (i16, i16)) -> (i32, i32) {
    (ACROSS - i32::from(x) + LEFT, THEIR_ROW - rows(y) + TOP)
}

/// A box's top left with the action areas, from the pitch's.
fn area_row(y: i16) -> i32 {
    i32::from(y) * 5 / 3 + AREA_ROW
}

fn area_box((x, y): (i16, i16)) -> (i32, i32) {
    (i32::from(x) + LEFT, area_row(y) + TOP)
}

/// An action area on the pitch, without the dialog's offset: left, top,
/// right, bottom.
fn area_rect([left, up, right, down]: marking::Area, (x, y): (i16, i16)) -> [i32; 4] {
    let (cx, cy) = (i32::from(x) + BOX.0 / 2, area_row(y) + BOX.1 / 2);
    let x0 = (cx - i32::from(left)).max(0);
    let y0 = (cy - i32::from(up)).max(0);
    let x1 = (cx + i32::from(right) - CORNER).min(RIGHT_MOST) + CORNER;
    let y1 = (cy + i32::from(down) - CORNER).min(BOTTOM_MOST) + CORNER;
    [x0, y0, x1, y1]
}

/// The bar takes the OK's place; the caption, the boxes and the bars'
/// strips are the screen's.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>, rules: &Rules) {
    let Some(dialog) = dialogs.get_mut(&COMPARISON) else { return };
    dialog.controls.retain(|c| c.id != Some(OK));
    for control in &mut dialog.controls {
        let Some(id) = control.id else { continue };
        if (OWN_BOXES..THEIR_BOXES + FIELD as u16).contains(&id) {
            control.kind = Kind::Player;
            control.rect = Rect::new(0.0, 0.0, BOX.0 as f32, BOX.1 as f32);
        } else if BARS.contains(&id) {
            control.kind = Kind::Paint;
        }
    }
    let mut area = dialog.controls.iter().find(|c| c.id == Some(OWN_BOXES)).cloned();
    if let Some(area) = area.as_mut() {
        area.id = Some(AREA);
    }
    dialog.controls.extend(area);
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, String::new()));
    hub::add_bar(dialog, rules);
}

/// The opponent's squad's caption, like every window's.
pub(crate) fn compose_opponent(dialogs: &mut HashMap<u16, Dialog>) {
    if let Some(dialog) = dialogs.get_mut(&OPPONENT_SQUAD) {
        dialog.controls.insert(0, super::caption(dialog.frame, THEIR_CAPTION, String::new()));
    }
}

#[derive(Default)]
pub struct Comparison {
    /// The opponent as the screen shows it, and its eleven.
    their_club: Option<ClubState>,
    theirs: Option<Side>,
    /// The ids of the players "Offensiv bei Standards" lists after
    /// "keiner".
    set_pieces: Vec<u16>,
    /// Whether the whistle asks.
    whistle: bool,
    /// Man-marking or the action areas, and the player whose area shows.
    mode: usize,
    chosen: usize,
}

/// An eleven: its players' ids and the kinds of position under their
/// spots, and the spots, by place.
struct Side {
    ids: Vec<(u16, u8)>,
    spots: [(i16, i16); FIELD],
}

impl Side {
    fn of(pitch: &lineup::Tables, club: &ClubState) -> Self {
        let ids = (0..FIELD)
            .map(|i| {
                let id = club.lineup.field[i].and_then(|p| club.status.get(p)).map_or(0, |s| s.id);
                (id, pitch.kind_under(club.lineup.spots[i]))
            })
            .collect();
        Self { ids, spots: club.lineup.spots }
    }
}

impl Screen for Comparison {
    fn ids(&self) -> &[u16] {
        &[COMPARISON]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        hub::open_bar(cx, view, COMPARISON_SYMBOL);
        (self.mode, self.chosen) = (MAN_MARKING, 1);
        self.their_club = opponent(cx);
        self.theirs = self.their_club.as_ref().map(|club| Side::of(&game.rules.pitch, club));
        // The areas keep within their bounds as the screen opens.
        change(cx, |club| marking::normalise(&mut club.areas));
        let Some(own) = own(cx) else { return };
        let records = cx.state.records.as_ref();
        let own_team = records.and_then(|r| team(cx, r, own_key(cx)?));
        let their_team = records.and_then(|r| team(cx, r, next_opponent(cx)?));

        set(view, dialog, CAPTION_ID, caption(cx, own_team, their_team));
        let today = cx.state.season.as_ref().map(|s| s.date());
        let list_view =
            cx.state.season.as_ref().and_then(|s| s.manager()).is_some_and(|m| m.list_view);
        let own_parts = match own.strengths {
            Some(stored) if list_view => stored,
            _ => game.rules.pitch.parts(&own.lineup, &own.players, &own.fitness),
        };
        let own_cohesion = today.map_or(0, |d| a3_core::cohesion::of(&game.rules.pitch, own, d));
        let own_figures = figures(own, own_team, own_cohesion);
        let mut bars: Vec<PaintState> = vec![PaintState::default(); FIGURES];
        show_figures(view, dialog, &OWN_FIGURES, &own_figures, &mut bars, 0, OWN_INK);
        for (&id, part) in OWN_PARTS.iter().zip(own_parts) {
            set(view, dialog, id, tenths(part));
        }
        if let Some(club) = &self.their_club {
            let cohesion = their_team.map_or(0, |t| i32::from(t.cohesion()));
            let their_figures = figures(club, their_team, cohesion);
            show_figures(
                view,
                dialog,
                &THEIR_FIGURES,
                &their_figures,
                &mut bars,
                BAR_WIDTH,
                THEIR_INK,
            );
            let parts = game.rules.pitch.parts(&club.lineup, &club.players, &club.fitness);
            for (&id, part) in THEIR_PARTS.iter().zip(parts) {
                set(view, dialog, id, tenths(part));
            }
        } else {
            for id in THEIR_PARTS {
                set(view, dialog, id, "0.0".into());
            }
        }
        for (&id, strip) in BARS.iter().zip(bars) {
            if let Some(paint) = control(dialog, id).and_then(|c| view.paint_mut(c)) {
                *paint = strip;
            }
        }

        self.fill_set_pieces(cx, view);
        for (id, bit) in MARKS {
            if let Some(check) = control(dialog, id).and_then(|c| view.check_mut(c)) {
                check.checked = own.tactics.marks(bit);
            }
        }
        if let Some(c) = control(dialog, MODE) {
            view.set_chosen(c, MAN_MARKING);
        }
        if let Some(c) = control(dialog, SQUAD) {
            view.set_disabled(c, self.theirs.is_none());
        }
        self.show_boxes(cx, view);

        let squad = self.their_club.is_some();
        let works = move |target: Target| match target {
            Target::Button(c) => dialog.controls[c].id != Some(SQUAD) || squad,
            _ => hub::on_bar(dialog, target).is_none_or(hub::bar_works),
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let dialog = cx.dialog;
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        if let Some(item) = hub::on_bar(dialog, target) {
            let outcome = hub::bar_click(cx, view, item, COMPARISON_SYMBOL);
            if matches!(outcome, Outcome::Close(_)) {
                keep_marks(cx, view);
            }
            self.whistle = matches!(outcome, Outcome::Popup(_));
            return outcome;
        }
        let own_box =
            |c: usize| id(c).checked_sub(OWN_BOXES).map(usize::from).filter(|&i| i < FIELD);
        let pressed = match target {
            Target::Drop { from: Grip::Control(c), moved, .. } => own_box(c).map(|p| (p, moved)),
            Target::Control(c) => own_box(c).map(|p| (p, (0, 0))),
            _ => None,
        };
        let playing = |place: usize| own(cx).is_some_and(|c| c.lineup.field[place].is_some());
        if let Some((place, moved)) = pressed.filter(|&(p, _)| playing(p)) {
            // A press on one of the eleven picks him for the areas; with
            // the areas, it shows his.
            if place > 0 {
                self.chosen = place;
            }
            if self.mode == ACTION_AREAS {
                self.show_boxes(cx, view);
            } else {
                // A box let go of where it was taken hold of, or anywhere:
                // the marker marks whom it covers, or nobody.
                self.drop(cx, view, place, moved);
            }
            return Outcome::Stay;
        }
        match target {
            Target::Option { control: c, item } if id(c) == MODE => {
                self.mode = item;
                self.show_boxes(cx, view);
            }
            // "Gegner": the opponent's own squad.
            Target::Button(c) if id(c) == SQUAD => {
                return Outcome::Popup(Request::new(OPPONENT_SQUAD));
            }
            // A handle of the area dragged: its sides follow the pointer.
            Target::Drop { from: Grip::Row { control: c, row }, at, moved, .. }
                if id(c) == AREA =>
            {
                let pointer = (i32::from(at.0 + moved.0), i32::from(at.1 + moved.1));
                self.drag_handle(cx, row, pointer);
                self.show_boxes(cx, view);
            }
            // A right click on one of the eleven: his info, paging through
            // the eleven.
            Target::Menu(Grip::Control(c)) => {
                if let Some(place) = own_box(c) {
                    let Some(club) = own(cx) else { return Outcome::Stay };
                    let Some(player) = club.lineup.field[place] else { return Outcome::Stay };
                    cx.state.paging = Some(club.lineup.field.iter().flatten().copied().collect());
                    return Outcome::Popup(Request::with(
                        playerinfo::CURRENT,
                        Args::Player(player),
                    ));
                }
                // The opponent's eleven likewise: the original hands over
                // his eleven's record ids, so the paging stays with him
                // (`OnRButtonUp`).
                let Some(place) = id(c).checked_sub(THEIR_BOXES).map(usize::from).filter(|&i| i < FIELD) else {
                    return Outcome::Stay;
                };
                let Some(side) = &self.theirs else { return Outcome::Stay };
                let record = side.ids[place].0;
                if record == 0 {
                    return Outcome::Stay;
                }
                cx.state.paging_ids =
                    Some(side.ids.iter().map(|&(id, _)| id).filter(|&id| id != 0).collect());
                return Outcome::Popup(Request::with(playerinfo::CURRENT, Args::Record(record)));
            }
            Target::Row { control: c, .. } if id(c) == SET_PIECE => {
                let row = view.combo(c).and_then(|combo| combo.selected).unwrap_or(0);
                let player = row.checked_sub(1).and_then(|r| self.set_pieces.get(r)).copied();
                change(cx, |club| club.set_piece = player.unwrap_or(0));
                view.lines_on_top = true;
            }
            // The list put up again: the dialog paints where it lay, and
            // the original draws its lines anew over the whole screen,
            // over the boxes, which stay as they were (until a box is let
            // go of).
            Target::Combo(c) if view.combo(c).is_some_and(|combo| !combo.open) => {
                view.lines_on_top = true;
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// The whistle's question: a yes ends the turn.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        if std::mem::take(&mut self.whistle) && reply == Reply::Ok {
            keep_marks(cx, view);
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}

impl Comparison {
    /// Both elevens in their boxes, and the lines of the pairs that stand;
    /// or the eleven on the whole pitch with their action areas.
    fn show_boxes(&mut self, cx: &mut Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let pitch = &game.rules.pitch;
        let areas = self.mode == ACTION_AREAS;
        let Some(own) = own(cx) else { return };
        let ours = Side::of(pitch, own);
        // Each box gets the duty its place has in the own eleven — the
        // opponent's too (`FUN_004ad7b2` reads the own club for both).
        let duties = a3_core::duties::on_pitch(&own.players, &own.duties, &own.lineup.field);
        let records = cx.state.records.as_ref();
        let boxes = |club: &ClubState, at: fn((i16, i16)) -> (i32, i32)| -> Vec<PlayerBox> {
            (0..FIELD)
                .map(|i| {
                    let spot = club.lineup.spots[i];
                    let position = pitch.position(spot);
                    let (x, y) = at(spot);
                    let at = Some(Vec2::new(x as f32, y as f32));
                    let Some(p) = club.lineup.field[i] else {
                        let name = pitch.position_names.get(usize::from(position)).copied();
                        let name = game.text(name.unwrap_or(0) as u16).to_owned();
                        return PlayerBox { name, at, ..PlayerBox::default() };
                    };
                    let fitness = lineup::fitness_of(&club.fitness, p);
                    let mut shown = filled_box(game, &club.players[p], fitness, position);
                    // A player whose strength the game does not know yet
                    // counts 1 (`FUN_00706a7e`).
                    let id = club.status.get(p).map_or(0, |s| s.id);
                    if records.and_then(|r| r.player(id)).is_some_and(|r| r.picks() >> 6 == 1) {
                        shown.strength = tenths(1);
                    }
                    let duty = duties[i].and_then(|d| pitch.duty_icon(d));
                    PlayerBox { duty, at, ..shown }
                })
                .collect()
        };
        let own_boxes = boxes(own, if areas { area_box } else { own_at });
        for (i, shown) in own_boxes.iter().enumerate() {
            if let Some(c) = control(dialog, OWN_BOXES + i as u16) {
                view.set_player(c, shown.clone());
            }
        }
        let their_boxes = self.their_club.as_ref().map(|club| boxes(club, their_at));
        for i in 0..FIELD {
            let Some(c) = control(dialog, THEIR_BOXES + i as u16) else { continue };
            view.controls[c].hidden = areas || their_boxes.is_none();
            if let Some(shown) = their_boxes.as_ref().map(|b| b[i].clone()) {
                view.set_player(c, shown);
            }
        }
        // The chosen player's area over the boxes, the others' darkened
        // under them (`FUN_004ae502`, `FUN_004ae3f0`).
        let on_pitch = |[x0, y0, x1, y1]: [i32; 4]| [x0 + LEFT, y0 + TOP, x1 + LEFT, y1 + TOP];
        let rect = |i: usize| on_pitch(area_rect(own.areas[i], own.lineup.spots[i]));
        view.shades = if areas {
            let others = (1..FIELD).filter(|&i| i != self.chosen);
            others.map(rect).map(|[x0, y0, x1, y1]| ([x0, y0, x1 - x0, y1 - y0], SHADE)).collect()
        } else {
            Vec::new()
        };
        if let Some(c) = control(dialog, AREA) {
            view.controls[c].hidden = !areas;
            if areas && let Some(shown) = own_boxes.get(self.chosen) {
                let [x0, y0, x1, y1] = rect(self.chosen).map(|v| v as f32);
                let area = Some(Rect::new(x0, y0, x1, y1));
                view.set_player(c, PlayerBox { area, ..shown.clone() });
            }
        }
        if areas {
            view.lines.clear();
            view.lines_on_top = false;
        } else {
            self.show_lines(cx, view, &ours);
        }
    }

    /// A handle of the chosen player's area dragged to where the pointer
    /// is: each of its sides there, if past the box, and no farther than
    /// the game lets it (`OnMouseMove_004b0265`).
    fn drag_handle(&self, cx: &mut Cx, handle: usize, (x, y): (i32, i32)) {
        let Some(own) = own(cx) else { return };
        let place = self.chosen;
        let spot = own.lineup.spots[place];
        let [mut left, mut top, mut right, mut bottom] = area_rect(own.areas[place], spot);
        // The handles from the top left, row by row, the middle left out.
        let (row, col) = match handle {
            h @ 0..=3 => (h / 3, h % 3),
            h => ((h + 1) / 3, (h + 1) % 3),
        };
        let (x, y) = ((x - LEFT).min(POINTER_RIGHT_MOST), y - TOP);
        let (bx, by) = (i32::from(spot.0), area_row(spot.1));
        if col == 0 {
            if x <= bx - PAST_THE_BOX {
                left = x;
            }
            left = left.max(bx - FARTHEST);
        }
        if row == 0 {
            if y <= by - PAST_THE_BOX {
                top = y;
            }
            top = top.max(by - FARTHEST);
        }
        if col == 2 {
            if x >= bx + PAST_THE_BOX + BOX.0 {
                right = x;
            }
            right = right.min(bx + FARTHEST);
        }
        if row == 2 {
            if y >= by + PAST_THE_BOX + BOX.1 {
                bottom = y;
            }
            bottom = bottom.min(by + FARTHEST);
        }
        // Kept in bytes, as the game keeps them.
        let (cx_, cy) = (bx + BOX.0 / 2, by + BOX.1 / 2);
        let byte = |v: i32, most: u8| (v as u8).min(most);
        let area = [
            byte(cx_ - left, MOST_ACROSS),
            byte(cy - top, MOST_ALONG),
            byte(right - cx_, MOST_ACROSS),
            byte(bottom - cy, MOST_ALONG),
        ];
        change(cx, |club| club.areas[place] = area);
    }

    /// The lines from each marker's box to the box of whom he marks; the
    /// pairs that no longer stand go.
    fn show_lines(&self, cx: &mut Cx, view: &mut View, ours: &Side) {
        view.lines.clear();
        view.lines_on_top = false;
        let Some(theirs) = &self.theirs else { return };
        let mut lines = Vec::new();
        change(cx, |club| lines = club.marking.standing(&ours.ids, &theirs.ids));
        let half = BOX.0 / 2;
        for (o, t) in lines {
            let (x0, y0) = own_at(ours.spots[o]);
            let (x1, y1) = their_at(theirs.spots[t]);
            view.lines.push(([x0 + half, y0, x1 + half, y1 + BOX.1], LINE_INK));
        }
    }

    /// One of the eleven let go of after the pointer moved so far: a
    /// defender marks the opponent's striker, a midfielder the midfielder,
    /// his box covers more than half of; anywhere else, nobody
    /// (`OnLButtonUp`).
    fn drop(&mut self, cx: &mut Cx, view: &mut View, place: usize, (dx, dy): (i16, i16)) {
        let pitch = &cx.game.rules.pitch;
        let Some(own) = own(cx) else { return };
        let ours = Side::of(pitch, own);
        let (marker, kind) = ours.ids[place];
        if marker == 0 || !(marking::defender(kind) || marking::midfielder(kind)) {
            return;
        }
        let Some(theirs) = &self.theirs else { return };
        let spot = own.lineup.spots[place];
        // Where the box went, as the game reckons it: 14 pixels short
        // across, and mirrored into the opponent's spots.
        let x = (i32::from(spot.0) + i32::from(dx) - LEFT).max(0);
        let y = (OWN_ROW + rows(spot.1) + i32::from(dy)).max(0);
        let (x, y) = (ACROSS - x, (THEIR_ROW - y) * 8 / 7);
        let target = covered(theirs, (x, y)).filter(|&t| t > 0);
        match target {
            Some(t) if marking::drops_on(kind, theirs.ids[t].1) => {
                let marked = theirs.ids[t].0;
                change(cx, |club| club.marking.set(marker, marked));
            }
            _ => change(cx, |club| club.marking.remove(marker)),
        }
        self.show_lines(cx, view, &ours);
    }

    /// "Offensiv bei Standards": "keiner", then the eleven's defenders, the
    /// one chosen selected.
    fn fill_set_pieces(&mut self, cx: &Cx, view: &mut View) {
        let Some(own) = own(cx) else { return };
        let pitch = &cx.game.rules.pitch;
        let mut items = vec![cx.game.text(NOBODY).to_owned()];
        let mut selected = 0;
        self.set_pieces.clear();
        for i in 0..FIELD {
            let Some(p) = own.lineup.field[i] else { continue };
            if !marking::defender(pitch.kind_under(own.lineup.spots[i])) {
                continue;
            }
            let id = own.status.get(p).map_or(0, |s| s.id);
            items.push(short_name(cx, &own.players[p], id));
            self.set_pieces.push(id);
            if id == own.set_piece {
                selected = self.set_pieces.len();
            }
        }
        if let Some(combo) = control(cx.dialog, SET_PIECE).and_then(|c| view.combo_mut(c)) {
            combo.items = items;
            combo.selected = Some(selected);
        }
    }
}

/// The opponent's place whose box the dragged box covers more than half
/// of, in the opponent's spots (`FUN_004b0d55`).
fn covered(theirs: &Side, (x, y): (i32, i32)) -> Option<usize> {
    let (w, h) = BOX;
    let unit = w * h / 100;
    (0..FIELD).find(|&i| {
        if theirs.ids[i].0 == 0 {
            return false;
        }
        let (c, r) = theirs.spots[i];
        let (c, r) = (i32::from(c), i32::from(r));
        let area = if c <= x && x < c + w && r <= y && y < r + h {
            (c + w - x) * (r + h - y)
        } else if x < c && c <= x + w && r <= y && y < r + h {
            (x + w - c) * (r + h - y)
        } else if c <= x && x < c + w && r <= y + 1 + h && y < r {
            (c + w - x) * (y + h - r)
        } else if x < c && c <= x + w && r <= y + 1 + h && y < r {
            (x + w - c) * (y + h - r)
        } else {
            0
        };
        let share = if area == 0 {
            0
        } else if area < unit {
            1
        } else {
            area / unit
        };
        share > 50
    })
}

/// The caption: both clubs, the own below and the opponent's above; or the
/// dialog's title and the own club.
fn caption(cx: &Cx, own: Option<&Team>, theirs: Option<&Team>) -> String {
    let game = cx.game;
    let name = |team: Option<&Team>, key: Option<(usize, usize)>| {
        team.map(Team::name).or_else(|| {
            let (slot, c) = key?;
            Some(game.world.country(slot)?.clubs.get(c)?.name.clone())
        })
    };
    let own_name = name(own, own_key(cx)).unwrap_or_default();
    match next_opponent(cx) {
        Some(key) => {
            let their_name = name(theirs, Some(key)).unwrap_or_default();
            game.text(TITLE).replacen("%s", &own_name, 1).replacen("%s", &their_name, 1)
        }
        None => format!("{} {own_name}", cx.dialog.title),
    }
}

/// Will to win, mood, condition, freshness, cohesion (in tenths) and
/// respect, as the screen shows them, each with its bar's height.
fn figures(club: &ClubState, team: Option<&Team>, cohesion: i32) -> [(String, i32); FIGURES] {
    let will = team.map_or(0, |t| i32::from(t.will()));
    let mood = team.map_or(0, |t| i32::from(t.mood()));
    let respect = team.map_or(0, |t| i32::from(t.respect()));
    let condition = lineup::average(club, lineup::condition);
    let freshness = lineup::average(club, lineup::freshness);
    let bar = |value: i32, most: i32| (value * BAR_HEIGHT / most).max(1);
    [
        ((will / 10).to_string(), bar(will, 200)),
        ((mood / 10).to_string(), bar(mood, 200)),
        (condition.to_string(), bar(condition, 100)),
        (freshness.to_string(), bar(freshness, 100)),
        (format!("{:2.1}", cohesion as f32 / 10.0), bar(cohesion, 200)),
        ((respect / 10).to_string(), bar(respect, 200)),
    ]
}

fn show_figures(
    view: &mut View,
    dialog: &Dialog,
    ids: &[u16; FIGURES],
    figures: &[(String, i32); FIGURES],
    bars: &mut [PaintState],
    x: i32,
    ink: u32,
) {
    for (i, (&id, (text, height))) in ids.iter().zip(figures).enumerate() {
        set(view, dialog, id, text.clone());
        bars[i].bar(x, BAR_HEIGHT - height, BAR_WIDTH, *height, ink);
    }
}

/// The marking boxes' ticks into the club's passing, as the screen is
/// left (`FUN_004b1631`).
fn keep_marks(cx: &mut Cx, view: &View) {
    let dialog = cx.dialog;
    let ticks = MARKS.map(|(id, bit)| {
        (bit, control(dialog, id).and_then(|c| view.check(c)).is_some_and(|c| c.checked))
    });
    change(cx, |club| {
        for (bit, on) in ticks {
            club.tactics.set_marks(bit, on);
        }
    });
}

/// The manager's club, as the season follows it, and its key.
fn own<'a>(cx: &'a Cx) -> Option<&'a ClubState> {
    let season = cx.state.season.as_ref()?;
    season.clubs.get(&season.manager()?.club?)
}

fn own_key(cx: &Cx) -> Option<(usize, usize)> {
    cx.state.season.as_ref()?.manager()?.club
}

/// A club's team record by its key.
fn team<'a>(cx: &Cx, records: &'a Records, (slot, club): (usize, usize)) -> Option<&'a Team> {
    let nation = cx.game.world.country(slot)?.nation;
    records.team(nation, u16::try_from(club).ok()?)
}

/// The club the own plays next, within the weeks the week screen looks
/// ahead; in a loaded game as the save has it.
fn next_opponent(cx: &Cx) -> Option<(usize, usize)> {
    let key = own_key(cx)?;
    if let Some(records) = cx.state.records.as_ref() {
        let own = team(cx, records, key)?;
        let number = usize::try_from(own.next_number()).ok()?;
        if own.next_country() == 0 {
            return None;
        }
        return Some((cx.game.world.slot_of(own.next_country().into())?, number));
    }
    let (game, season) = (cx.game, cx.state.season.as_ref()?);
    let m = season
        .next_match(&game.world, &game.calendar, key)
        .filter(|m| m.day < season.day + FIXTURE_AHEAD)?;
    Some(if m.home == key { m.away } else { m.home })
}

/// The opponent as the game has it: in a loaded game its record, else the
/// data files' squad in the first formation.
fn opponent(cx: &Cx) -> Option<ClubState> {
    let key = next_opponent(cx)?;
    let (game, records) = (cx.game, cx.state.records.as_ref());
    if let Some(club) = records.and_then(|r| a3_core::load::club(r, &game.data, key)) {
        return Some(club);
    }
    let players = game.world.country(key.0)?.clubs.get(key.1)?.players.clone();
    let lineup = game.rules.pitch.line_up(&players, 0);
    Some(ClubState::new(players, lineup, Vec::new(), 0))
}

/// Changes the manager's club, and its records with it: the marking, the
/// areas, who goes forward at set pieces, the passing.
fn change(cx: &mut Cx, change: impl FnOnce(&mut ClubState)) {
    let game = cx.game;
    let state = &mut *cx.state;
    let Some(season) = state.season.as_mut() else { return };
    let Some(key) = season.manager().and_then(|m| m.club) else { return };
    let Some(club) = season.clubs.get_mut(&key) else { return };
    change(club);
    let Some(records) = state.records.as_mut() else { return };
    let Some(nation) = game.world.country(key.0).map(|c| c.nation) else { return };
    let Some(team) = u16::try_from(key.1).ok().and_then(|n| records.team_mut(nation, n)) else {
        return;
    };
    club.marking.write(team);
    team.set_areas(club.areas);
    team.set_set_piece(club.set_piece);
    team.set_passing(club.tactics.passing);
}

/// Tenths as the game prints them: "7.3".
fn tenths(value: i32) -> String {
    format!("{:.1}", value as f32 / 10.0)
}

/// The opponent's squad, "Gegner" (dialog 476, `CSpielerkaderGegen`): the
/// opponent in his squad's own order, the eleven in black, the bench blue,
/// the rest in the stands green; his info is not built yet.
#[derive(Default)]
pub struct OpponentSquad;

impl Screen for OpponentSquad {
    fn ids(&self) -> &[u16] {
        &[OPPONENT_SQUAD]
    }

    /// The rows in the club's own order, sorted by nothing (`FUN_005652cb`).
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(club) = opponent(cx) else { return };
        let key = next_opponent(cx);
        let name = cx
            .state
            .records
            .as_ref()
            .and_then(|r| key.and_then(|k| team(cx, r, k)))
            .map(Team::name)
            .or_else(|| {
                let (slot, c) = key?;
                Some(game.world.country(slot)?.clubs.get(c)?.name.clone())
            })
            .unwrap_or_default();
        set(view, dialog, THEIR_CAPTION, format!("{} {}", dialog.title, name));
        let Some(today) = cx.state.season.as_ref().map(|s| s.date()) else { return };
        let mut rows = Vec::new();
        let mut inks = Vec::new();
        for p in 0..club.players.len() {
            // The game's row leaves out the Zuf. field altogether: Ko.
            // and Fr. move a column left into the (383, 413) cells.
            let mut row = super::squad::squad_row(cx, &club, p, today).to_vec();
            row.remove(8);
            rows.push(row.join("\t"));
            let ink = if club.lineup.field.contains(&Some(p)) {
                ELEVEN_INK
            } else if club.lineup.bench.iter().any(|&b| b == Some(p)) {
                BENCH_INK
            } else {
                STANDS_INK
            };
            inks.push(Some(ink));
        }
        if let Some(list) = control(dialog, THEIR_LIST).and_then(|c| view.list_mut(c)) {
            let visible = control(dialog, THEIR_LIST)
                .map_or(0, |l| crate::controls::list::visible_rows(&dialog.controls[l]));
            list.scroll = list.scroll.min(rows.len().saturating_sub(visible));
            list.selected = None;
            list.rows = rows;
            list.inks = inks;
            list.keys = (0..club.players.len()).collect();
        }
        let works = |target: Target| match target {
            Target::Button(c) => crate::controls::button::is_ok(&dialog.controls[c]),
            // A row would ask for the player's info: not built yet.
            Target::Row { .. } | Target::Cell { .. } | Target::Menu(_) => false,
            _ => true,
        };
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        if let Target::Button(c) = target
            && crate::controls::button::is_ok(&cx.dialog.controls[c])
        {
            return Outcome::Close(Reply::Ok);
        }
        Outcome::Stay
    }
}
