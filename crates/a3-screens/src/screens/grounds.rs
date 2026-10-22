//! The club grounds (dialog 466, `CMapDialog`; STADION-ANSICHT.md §4–7 in
//! the RE repo): the map of the club's land and what stands on it, at three
//! zooms, with its minimap; the modes, the categories and the objects to
//! build; the cursor and what it would cost; buying, building, selling and
//! pulling down, with their questions. The tooltip and the four
//! sub-dialogs are not built yet.

use std::collections::HashMap;

use a3_core::cycle::dialogs::{GROUNDS, NOTICE, QUESTION};
use a3_core::cycle::{Fill, Request};
use a3_core::grounds::{
    self, Area, Category, Demolition, Grounds, Mode, Purchase, Sale, View as MapView, Works,
};
use a3_core::random::Random;
use a3_core::records::Team;
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::{Rect, Vec2};
use image::RgbaImage;

use super::finances::money64;
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::buildings::MapArt;
use crate::controls::bitmap::{Picture, Tiles, Tip};
use crate::pointer::{Grip, Target};
use crate::view::View;

const CAPTION_ID: u16 = 9935;
/// The map and the minimap the screen draws, over the file's frames.
const MAP: u16 = 9940;
const MINIMAP: u16 = 9941;
const MAP_AT: Rect = Rect { min: Vec2::new(2.0, 25.0), max: Vec2::new(451.0, 474.0) };
const MAP_SIZE: u32 = 449;
/// Where the map answers the pointer (`PtInRect((2,25,450,473))`).
const MAP_HIT: Rect = Rect { min: Vec2::new(2.0, 25.0), max: Vec2::new(450.0, 473.0) };
const MINIMAP_AT: Rect = Rect { min: Vec2::new(535.0, 71.0), max: Vec2::new(636.0, 172.0) };
/// Where a minimap click puts the view (`0x7c1a00`): from these, not the
/// minimap's corner, a tile left and four up of what was clicked.
const MINIMAP_CLICK: (i32, i32) = (536 - 535, 67 - 71);
const ZOOM_IN: u16 = 5969;
const ZOOM_OUT: u16 = 5970;
/// The sub-dialogs' buttons and dialogs: info, staff, fans, what is being
/// built (§8).
const SUBDIALOGS: [(u16, u16); 4] = [(5971, INFO), (5774, STAFF), (5972, FANS), (5979, WORKS)];
const INFO: u16 = 467;
const STAFF: u16 = 465;
const FANS: u16 = 468;
const WORKS: u16 = 469;
const INVESTABLE: u16 = 6258;
const COST_LABEL: u16 = 6291;
const COST: u16 = 6290;
const NO_COST: &str = "-";
/// The modes' cost labels: "Kaufkosten:", "Baukosten:", "Erlöse:",
/// "Abrißkosten:".
const MODES: u16 = 5963;
const MODE_LABELS: u16 = 4435;
const CATEGORIES: u16 = 6471;
const OBJECTS: u16 = 6470;
const OBJECT_TEXT: u16 = 5966;
/// An object's text with its weeks of building (5041 "%s\n%s\n%d
/// Woche(n)").
const WITH_WEEKS: u16 = 5041;
const CATEGORY: [Category; 3] = [Category::Landscape, Category::Club, Category::Entertainment];
/// The zoom the screen opens at.
const OPENING_ZOOM: i32 = 1;
/// The tooltip's second line: a stadium's places (5074 "Kapazität %s"),
/// or the weeks of building left ("Bauzeit %s", a literal at `0x8b0ea8`,
/// with `FUN_00578e2f`'s 4141 "0 Wochen", 4142 "eine Woche", 4143 "%d
/// Wochen").
const CAPACITY: u16 = 5074;
const BUILDING_TIME: &str = "Bauzeit %s";
const WEEKS_TEXTS: [u16; 3] = [4141, 4142, 4143];
/// How often the frames come once the renderer has caught up (the
/// thread's sleep, at least 30 ms), and the animation's step.
const FRAME_SECS: f32 = 0.03;
const ANIMATION_SECS: f32 = 0.16;
/// The modes, as the tabs of 5963 stand.
const MODE: [Mode; 4] = [Mode::Buy, Mode::Build, Mode::Sell, Mode::Demolish];
/// "Dieses Gebiet würde sie %s kosten, also zweifellos zuviel.", and the
/// questions of a purchase: land only; one building, its staff costing or
/// not; several (`FUN_007c3980`).
const TOO_DEAR: u16 = 4364;
const BUY_LAND: u16 = 4365;
const BUY_BUILDING: [u16; 2] = [4992, 4993];
const BUY_BUILDINGS: [u16; 2] = [4994, 4995];
/// "Sie können Ihr Stadion nicht verkaufen!", "Dieses Gebiet würde %s
/// einbringen. …" (`FUN_007c48a5`).
const STADIUM_UNSOLD: u16 = 5073;
const SELL: u16 = 4370;
/// A stadium cannot be pulled down, what is to be pulled down costs too
/// much, one object, several (`FUN_007c4411`).
const STADIUM_STANDS: u16 = 4369;
const DEMOLITION_TOO_DEAR: u16 = 4367;
const DEMOLISH_ONE: u16 = 4813;
const DEMOLISH_SOME: u16 = 4368;

/// The map, the minimap, the caption.
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&GROUNDS) else { return };
    for (id, rect) in [(MINIMAP, MINIMAP_AT), (MAP, MAP_AT)] {
        let picture = Control {
            kind: Kind::Bitmap,
            rect,
            align: Align::Left,
            text: String::new(),
            id: Some(id),
            font: 0,
            ink: [0; 3],
        };
        dialog.controls.insert(0, picture);
    }
    let title = dialog.title.clone();
    dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    for id in [INFO, STAFF, FANS, WORKS] {
        let Some(dialog) = dialogs.get_mut(&id) else { continue };
        let title = dialog.title.clone();
        dialog.controls.insert(0, super::caption(dialog.frame, CAPTION_ID, title));
    }
}

#[derive(Default)]
pub struct GroundsScreen {
    grounds: Option<Grounds>,
    view: MapView,
    art: MapArt,
    /// The objects of the category shown, by kind, as the combo lists them.
    objects: Vec<usize>,
    category: usize,
    /// The pictures' numbers, new for every one drawn.
    drawn: u64,
    /// The map as last drawn, without the cursor, and whether the grounds
    /// or the view have changed since.
    base: Option<RgbaImage>,
    stale: bool,
    mode: Mode,
    /// What building mode builds (`0x1cb875c`).
    kind: i16,
    /// Whether the pointer was over the map at the last move the dialog
    /// itself was told of — not one over a button, a tab, the combo or the
    /// minimap (`0x1cb8740`); whether the button holds (`0x1cb8754`).
    over: bool,
    held: bool,
    /// The tile the frames take the pointer to be on: from wherever the
    /// pointer is, while the dialog takes it to be over the map
    /// (`0x1cb8730`).
    pointer: Option<(i32, i32)>,
    /// The cost shown, none for "-" (+0x1978).
    cost: Option<i64>,
    /// The tooltip shown, the tile it was last made for (+0x604, +0x608),
    /// and where the pointer was at the last move seen.
    tip: Option<Tip>,
    tip_tile: (i32, i32),
    last_pointer: Option<Vec2>,
    /// Time not yet taken by frames, and since the last animation step.
    clock: f32,
    animated: f32,
    asked: Option<Ask>,
}

/// What a question of the screen asks about.
enum Ask {
    Buy(Purchase),
    Sell(Sale),
    Demolish(Demolition),
}

impl Screen for GroundsScreen {
    fn ids(&self) -> &[u16] {
        &[GROUNDS]
    }

    /// `FUN_007c128c`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        self.grounds = None;
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let mut grounds = Grounds::of(records, team.country(), i16::from(team.number()));
        let investable =
            a3_core::finances::Balance::of(team, records.globals.fund_prices(), &records.agreed())
                .investable();
        if let Some(g) = grounds.as_mut() {
            if let Some(stadiums) = &cx.state.grounds_stadiums {
                g.set_looks(&stadiums.looks);
            }
            g.set_investable(investable);
            self.view = g.view().zoomed(OPENING_ZOOM);
        } else {
            self.view = MapView::default().zoomed(OPENING_ZOOM);
        }
        self.grounds = grounds;
        if let Some(c) = control(dialog, MODES) {
            view.set_chosen(c, 0);
        }
        set(view, dialog, COST, NO_COST.to_owned());
        set(view, dialog, COST_LABEL, game.text(MODE_LABELS).to_owned());
        set(view, dialog, INVESTABLE, money64(game, investable));
        if let Some(c) = control(dialog, CATEGORIES) {
            view.set_chosen(c, 0);
        }
        self.category = 0;
        self.fill_objects(cx, view);
        if let Some(c) = control(dialog, MINIMAP).and_then(|c| view.bitmap_mut(c)) {
            c.tiles =
                Some(Tiles { size: 1.0, offset: Vec2::ZERO, first: (0, 0), area: MINIMAP_AT });
        }
        (self.mode, self.over, self.held, self.pointer, self.cost, self.asked) =
            (Mode::Buy, false, false, None, None, None);
        (self.tip, self.tip_tile, self.last_pointer) = (None, (-1, -1), None);
        (self.clock, self.animated) = (0.0, 0.0);
        view.arrows_scroll = true;
        // The first frame animates at once.
        self.animate(cx);
        self.draw(cx, view);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        // The button pressed over the map holds the cursor's start; let go,
        // the mode acts if the pointer is still over the map — which the
        // dialog follows wherever it goes while the button holds.
        match target {
            // The move there first, then the button down hides the
            // tooltip (0x7c1f82).
            Target::Tile { control: c, x, y } if id(c) == MAP => {
                self.held = false;
                self.moved(cx, view);
                self.over = true;
                self.tip = None;
                self.follow(view);
                self.pointer = self.pointer.or(Some((x as i32, y as i32)));
                self.frame();
                self.held = true;
                self.frame();
                self.show(cx, view);
                return Outcome::Stay;
            }
            Target::Drop { from: Grip::Tile { control: c, .. }, to, .. } if id(c) == MAP => {
                let on_map = matches!(to, Some(Grip::Tile { control, .. }) if control == c);
                self.over = view.pointer.map_or(on_map, in_map);
                self.follow(view);
                if let Some(Grip::Tile { control, x, y }) = to
                    && control == c
                    && view.pointer.is_none()
                {
                    self.pointer = Some((x as i32, y as i32));
                }
                self.frame();
                self.held = false;
                if !self.over {
                    self.frame();
                    self.show(cx, view);
                    return Outcome::Stay;
                }
                return self.act(cx, view);
            }
            Target::Drop { .. } | Target::Menu(_) => return Outcome::Stay,
            _ => {}
        }
        let outcome = match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.close(cx);
                return Outcome::Close(Reply::Ok);
            }
            // A sub-dialog sees the grounds as they are (`0x7c2a2e` …).
            Target::Button(c) if SUBDIALOGS.iter().any(|&(button, _)| button == id(c)) => {
                let (_, sub) = SUBDIALOGS.iter().find(|&&(button, _)| button == id(c)).unwrap();
                cx.state.grounds_shown = self.grounds.clone();
                Outcome::Popup(Request::new(*sub))
            }
            Target::Button(c) if matches!(id(c), ZOOM_IN | ZOOM_OUT) => {
                let zoom = self.view.zoom + if id(c) == ZOOM_IN { 1 } else { -1 };
                if (0..=2).contains(&zoom) {
                    self.view = self.view.zoomed(zoom);
                    self.stale = true;
                }
                Outcome::Stay
            }
            Target::Tile { control: c, x, y } if id(c) == MINIMAP => {
                let n = self.view.tiles;
                let x = (x as i32 - MINIMAP_CLICK.0) * 32 - n * 16;
                let y = (y as i32 - MINIMAP_CLICK.1) * 32 - n * 16;
                self.view.scroll = (x, y);
                self.view.clamp();
                self.stale = true;
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == MODES => {
                self.mode = MODE.get(item).copied().unwrap_or_default();
                set(view, dialog, COST_LABEL, game.text(MODE_LABELS + item as u16).to_owned());
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == CATEGORIES => {
                if item != self.category && item < CATEGORY.len() {
                    self.category = item;
                    self.fill_objects(cx, view);
                }
                Outcome::Stay
            }
            Target::Row { control: c, .. } if id(c) == OBJECTS => {
                self.show_object(cx, view);
                Outcome::Stay
            }
            _ => Outcome::Stay,
        };
        // The next frame takes what changed: the view, the mode, the
        // object.
        self.follow(view);
        self.frame();
        self.show(cx, view);
        outcome
    }

    /// A question or notice answered: the frames go on with the pointer
    /// where the popup left it.
    fn answer(&mut self, cx: &mut Cx, view: &mut View, reply: Reply) -> Outcome {
        // Where the popup left the pointer is no move the dialog saw.
        self.last_pointer = view.pointer;
        if let Some(ask) = self.asked.take()
            && reply == Reply::Ok
        {
            self.answered(cx, view, ask);
        }
        self.follow(view);
        self.frame();
        self.show(cx, view);
        Outcome::Stay
    }

    /// The pointer moving moves the cursor: over the map, or anywhere
    /// once the dialog takes it to be over the map.
    fn tick(&mut self, cx: &mut Cx, view: &mut View, seconds: f32) -> Outcome {
        let map = control(cx.dialog, MAP);
        let was = (self.pointer, self.tip.clone());
        // The dialog sees the moves over the map and over itself — all of
        // them while the button holds —, not those over its controls.
        let seen = self.held
            || match view.hovered {
                Some(Target::Tile { control, .. }) => Some(control) == map,
                Some(_) => false,
                None => true,
            };
        if seen && view.pointer != self.last_pointer {
            self.moved(cx, view);
        } else if seen && view.hovered.is_none() {
            self.over = false;
            self.tip = None;
        }
        // The frames' own work: scrolling at the screen's edges and by the
        // arrows, the animation.
        self.clock = (self.clock + seconds).min(FRAME_SECS * 4.0);
        if self.clock >= FRAME_SECS {
            self.clock -= FRAME_SECS;
            self.animated += FRAME_SECS;
            if let Some(g) = self.grounds.as_mut() {
                let at = view.pointer.unwrap_or(Vec2::new(320.0, 240.0));
                let [l, r, u, d] = view.arrows;
                let pushed =
                    [at.x <= 0.0 || l, at.x >= 639.0 || r, at.y <= 0.0 || u, at.y >= 479.0 || d];
                let before = self.view.scroll;
                if g.glide(&mut self.view, pushed) {
                    self.tip = None;
                }
                self.stale |= self.view.scroll != before;
            }
            if self.animated >= ANIMATION_SECS {
                self.animated = 0.0;
                self.animate(cx);
            }
        }
        self.follow(view);
        if (self.pointer, self.tip.clone()) != was || self.stale {
            self.frame();
            self.show(cx, view);
        }
        if let Some(g) = &self.grounds {
            let volumes = g.ambience(&self.view);
            let ambience: Vec<_> = grounds::SOUNDS.iter().copied().zip(volumes).collect();
            if view.ambience != ambience {
                view.ambience = ambience;
            }
        }
        Outcome::Stay
    }
}

/// Weeks as the game words them (`FUN_00578e2f`).
fn weeks_text(cx: &Cx, weeks: i32) -> String {
    let text = cx.game.text(WEEKS_TEXTS[weeks.clamp(0, 2) as usize]);
    text.replacen("%d", &weeks.to_string(), 1)
}

/// Whether a point lies on the map, as `PtInRect` takes it.
fn in_map(at: Vec2) -> bool {
    at.x >= MAP_HIT.min.x && at.x < MAP_HIT.max.x && at.y >= MAP_HIT.min.y && at.y < MAP_HIT.max.y
}

/// The manager's team, to book to, and the game's random numbers.
fn team_and_random<'a>(cx: &'a mut Cx) -> Option<(&'a mut Team, &'a mut Random)> {
    let (country, number) = own_team(cx).map(|t| (t.country(), u16::from(t.number())))?;
    let state = &mut *cx.state;
    let team = state.records.as_mut()?.team_mut(country.into(), number)?;
    Some((team, &mut state.season.as_mut()?.random))
}

/// A name as `_stricoll` in the C locale orders it: ASCII letters by
/// their small form, the others by their code page byte, after them.
fn order_key(name: &str) -> Vec<u8> {
    name.chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            a3_files::bitmap_font::to_cp1252(c).unwrap_or(0xff)
        })
        .collect()
}

impl GroundsScreen {
    /// The category's objects, sorted, the first chosen (`FUN_007c16d5`).
    fn fill_objects(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let category = CATEGORY[self.category];
        let mut objects: Vec<usize> = (0..grounds::TYPES.len())
            .filter(|&k| grounds::TYPES[k].category == Some(category))
            .collect();
        objects.sort_by_key(|&k| order_key(game.text(grounds::TYPES[k].text)));
        self.objects = objects;
        if let Some(combo) = control(dialog, OBJECTS).and_then(|c| view.combo_mut(c)) {
            combo.items = self
                .objects
                .iter()
                .map(|&k| game.text(grounds::TYPES[k].text).to_owned())
                .collect();
            combo.selected = Some(0);
            combo.scroll = 0;
        }
        self.show_object(cx, view);
    }

    /// The object chosen in the combo, by kind.
    fn chosen(&self, cx: &Cx, view: &View) -> Option<i16> {
        let row = control(cx.dialog, OBJECTS).and_then(|c| view.combo(c)).and_then(|c| c.selected);
        row.and_then(|r| self.objects.get(r)).and_then(|&k| i16::try_from(k).ok())
    }

    /// The object chosen: its name, price and, for a building, its weeks
    /// (`FUN_007c1870`).
    fn show_object(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(k) = self.chosen(cx, view) else { return };
        self.kind = k;
        let kind = &grounds::TYPES[k as usize];
        let name = game.text(kind.text);
        let price = money(game, i64::from(kind.price / 100 * 100));
        let text = if self.category != 0 && kind.weeks > 0 {
            super::finances::printf(
                &game.text(WITH_WEEKS).replace("\\n", "\n"),
                &[&name, &price, &kind.weeks],
            )
        } else {
            format!("{name}\n{price}")
        };
        set(view, dialog, OBJECT_TEXT, text);
    }

    /// The map and the minimap as the view stands, the map's tiles
    /// answering the pointer.
    fn draw(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(grounds) = &self.grounds else { return };
        self.base = Some(self.art.map(game, grounds, &self.view, MAP_SIZE));
        let mini = crate::buildings::minimap(grounds, &self.view);
        self.drawn += 1;
        if let Some(bitmap) = control(dialog, MINIMAP).and_then(|c| view.bitmap_mut(c)) {
            bitmap.picture = Some(Picture::drawn(self.drawn, mini));
        }
        let ((ox, oy), (offx, offy)) = self.view.origin();
        if let Some(bitmap) = control(dialog, MAP).and_then(|c| view.bitmap_mut(c)) {
            bitmap.tiles = Some(Tiles {
                size: self.view.tile as f32,
                offset: Vec2::new(offx as f32, offy as f32),
                first: (ox as usize, oy as usize),
                area: MAP_HIT,
            });
        }
        self.stale = false;
        self.compose(cx, view);
    }

    /// The map with the cursor over it, and the cost; the map drawn anew
    /// first if it has changed.
    fn show(&mut self, cx: &Cx, view: &mut View) {
        if self.stale || self.base.is_none() {
            self.draw(cx, view);
        } else {
            self.compose(cx, view);
        }
    }

    fn compose(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(grounds), Some(base)) = (&self.grounds, &self.base) else { return };
        let mut map = base.clone();
        self.art.cursor(game, grounds, &self.view, &mut map);
        self.drawn += 1;
        if let Some(bitmap) = control(dialog, MAP).and_then(|c| view.bitmap_mut(c)) {
            bitmap.picture = Some(Picture::drawn(self.drawn, map));
            bitmap.tip = self.tip.clone();
        }
        let cost = self.cost.map_or(NO_COST.to_owned(), |c| money64(game, c));
        set(view, dialog, COST, cost);
    }

    /// A move the dialog sees (`OnMouseMove 0x7c1a78`): whether the pointer
    /// is over the map; the tooltip of what stands on the tile, made anew
    /// only for another tile, hidden off the map, over nothing and while
    /// the button holds.
    fn moved(&mut self, cx: &Cx, view: &View) {
        let Some(at) = view.pointer else { return };
        self.last_pointer = Some(at);
        self.over = in_map(at);
        if self.held || !self.over {
            self.tip = None;
            return;
        }
        let tile = self.tile_at(at);
        if tile == self.tip_tile {
            return;
        }
        let Some(g) = &self.grounds else { return };
        let i = usize::try_from(tile.1 * grounds::SIZE as i32 + tile.0).unwrap_or(usize::MAX);
        let Some(k) = g.at_index(i) else {
            self.tip = None;
            return;
        };
        let game = cx.game;
        let e = g.entry(k);
        let stadiums = cx.state.grounds_stadiums.as_ref();
        let stadium = usize::try_from(e.kind - grounds::FIRST_STADIUM)
            .ok()
            .filter(|&s| s < grounds::STADIUMS);
        let lines = match stadium {
            Some(s) => {
                let name = stadiums.map(|st| st.names[s].clone()).unwrap_or_default();
                let capacity = stadiums.map_or(0, |st| st.capacities[s]);
                let places = super::finances::count(capacity);
                vec![name, game.text(CAPACITY).replacen("%s", &places, 1)]
            }
            None => {
                let name = grounds::TYPES.get(e.kind as usize).map_or("", |t| game.text(t.text));
                let mut lines = vec![name.to_owned()];
                if e.weeks > 0 {
                    lines.push(BUILDING_TIME.replacen("%s", &weeks_text(cx, e.weeks.into()), 1));
                }
                lines
            }
        };
        self.tip = Some(Tip { at, lines });
        self.tip_tile = tile;
    }

    /// A step of the animation, from the game's random numbers; the map
    /// drawn anew.
    fn animate(&mut self, cx: &mut Cx) {
        let (Some(g), Some(season)) = (self.grounds.as_mut(), cx.state.season.as_mut()) else {
            return;
        };
        g.animate(&mut season.random);
        self.stale = true;
    }

    /// The tile under a point of the dialog, by the view's tiles.
    fn tile_at(&self, at: Vec2) -> (i32, i32) {
        let ((ox, oy), (offx, offy)) = self.view.origin();
        let t = self.view.tile.max(1);
        let (x, y) = (at.x.floor() as i32, at.y.floor() as i32);
        ((x - 2 + offx) / t + ox, (y - 25 + offy) / t + oy)
    }

    /// The tile the frames take the pointer to be on (0x7ba74e): from the
    /// pointer's place in the dialog, in whole pixels, by the view's tiles.
    fn follow(&mut self, view: &View) {
        self.pointer = if self.over { view.pointer.map(|at| self.tile_at(at)) } else { None };
    }

    /// The cursor as a frame sets it (`FUN_007ba408`, 0x7ba7cb): none off
    /// the map; the tile pointed at — in building mode the object's
    /// footprint — and while the button holds, from where it was pressed
    /// to the tile pointed at, in building mode by whole footprints; what
    /// that costs while held. Kaufen and Abriß keep their cost while only
    /// pointing, the others show none.
    fn frame(&mut self) {
        let (mode, kind) = (self.mode, self.kind);
        let Some(g) = self.grounds.as_mut() else { return };
        let Some((x, y)) = self.pointer else {
            g.set_cursor(None, mode, kind);
            self.cost = None;
            return;
        };
        let (w, _, d) = g.size(kind);
        if !self.held {
            let area = match mode {
                Mode::Build => Area { x, y, dx: w - 1, dy: d - 1 },
                _ => Area::tile(x, y),
            };
            g.set_cursor(Some(area), mode, kind);
            if matches!(mode, Mode::Build | Mode::Sell) {
                self.cost = None;
            }
            return;
        }
        let (x0, y0) = g.cursor_start();
        let (mut dx, mut dy) = (x - x0, y - y0);
        if mode == Mode::Build {
            let whole = |v: i32, size: i32| {
                let s = v.signum();
                if v == 0 { size - 1 } else { (v / size.max(1) + s) * size - s }
            };
            (dx, dy) = (whole(dx, w), whole(dy, d));
        }
        g.set_cursor(Some(Area { x: x0, y: y0, dx, dy }), mode, kind);
        self.cost = match mode {
            Mode::Buy => g.purchase_cost(),
            Mode::Build => g.build_cost(kind),
            Mode::Sell => g.sale_value(),
            Mode::Demolish => g.demolition_cost(),
        };
        // The car parks' preview lays them.
        self.stale |= mode == Mode::Build && kind == grounds::PARKING && self.cost > Some(0);
    }

    /// The button let go of over the map (`OnLButtonUp 0x7c2004`): what
    /// the mode does with the cursor's area — at once in building mode, by
    /// a question in the others.
    fn act(&mut self, cx: &mut Cx, view: &mut View) -> Outcome {
        let (mode, kind) = (self.mode, self.kind);
        let Some(g) = self.grounds.as_mut() else { return Outcome::Stay };
        let investable = g.investable();
        let ask = |text, fills| Outcome::Popup(Request::with(QUESTION, Args::Printf(text, fills)));
        let tell = |text, fills| Outcome::Popup(Request::with(NOTICE, Args::Printf(text, fills)));
        let outcome = match mode {
            Mode::Build => {
                let Some((team, random)) = team_and_random(cx) else { return Outcome::Stay };
                match g.works(kind, team, random) {
                    Works::Build { cost, .. } if cost >= investable => {
                        tell(TOO_DEAR, [Fill::Money64(cost), Fill::None])
                    }
                    Works::Build { positions, .. } => {
                        g.build(kind, &positions, team, random);
                        g.set_cursor(None, mode, kind);
                        self.stale = true;
                        self.refresh(cx, view);
                        Outcome::Stay
                    }
                    works => {
                        self.stale |= works != Works::Nothing;
                        g.set_cursor(None, mode, kind);
                        Outcome::Stay
                    }
                }
            }
            Mode::Buy => match g.purchase() {
                Some(p) if p.sum == 0 => Outcome::Stay,
                Some(p) if p.sum > investable => tell(TOO_DEAR, [Fill::Money64(p.sum), Fill::None]),
                Some(p) => {
                    let texts = match p.buildings {
                        ..=0 => [BUY_LAND; 2],
                        1 => BUY_BUILDING,
                        _ => BUY_BUILDINGS,
                    };
                    let text = texts[usize::from(p.wages <= 0)];
                    let fills = [Fill::Money64(p.sum), Fill::Money(p.wages.into())];
                    self.asked = Some(Ask::Buy(p));
                    ask(text, fills)
                }
                None => Outcome::Stay,
            },
            Mode::Sell => match g.sale() {
                Some(s) if s.stadium => tell(STADIUM_UNSOLD, [Fill::None; 2]),
                Some(s) if s.sum == 0 => Outcome::Stay,
                Some(s) => {
                    let fills = [Fill::Money64(s.sum), Fill::None];
                    self.asked = Some(Ask::Sell(s));
                    ask(SELL, fills)
                }
                None => Outcome::Stay,
            },
            Mode::Demolish => match g.demolition() {
                Some(d) if d.stadium => tell(STADIUM_STANDS, [Fill::None; 2]),
                Some(d) if d.count == 0 => Outcome::Stay,
                Some(d) if d.cost > investable => {
                    tell(DEMOLITION_TOO_DEAR, [Fill::Money64(d.cost), Fill::None])
                }
                Some(d) => {
                    let fills = if d.count == 1 {
                        (DEMOLISH_ONE, [Fill::Money64(d.cost), Fill::None])
                    } else {
                        (DEMOLISH_SOME, [Fill::Number(d.count), Fill::Money64(d.cost)])
                    };
                    self.asked = Some(Ask::Demolish(d));
                    ask(fills.0, fills.1)
                }
                None => Outcome::Stay,
            },
        };
        // The frames go on under a question: the cursor where the pointer
        // is, the button no longer held.
        self.frame();
        self.show(cx, view);
        outcome
    }

    /// A question answered: OK buys, sells or pulls down and books it.
    fn answered(&mut self, cx: &mut Cx, view: &mut View, ask: Ask) {
        let Some((team, _)) = team_and_random(cx) else { return };
        let Some(g) = self.grounds.as_mut() else { return };
        match ask {
            Ask::Buy(p) => {
                g.buy(&p);
                grounds::book(team, p.sum);
            }
            Ask::Sell(s) => {
                g.sell(&s);
                grounds::book(team, -s.sum);
            }
            Ask::Demolish(d) => {
                g.demolish(&d, team);
                grounds::book(team, d.cost);
            }
        }
        self.stale = true;
        self.refresh(cx, view);
    }

    /// What the club may spend, taken anew and shown (`FUN_007c639d`).
    fn refresh(&mut self, cx: &Cx, view: &mut View) {
        let (Some(team), Some(records)) = (own_team(cx), cx.state.records.as_ref()) else { return };
        let investable =
            a3_core::finances::Balance::of(team, records.globals.fund_prices(), &records.agreed())
                .investable();
        if let Some(g) = self.grounds.as_mut() {
            g.set_investable(investable);
        }
        set(view, cx.dialog, INVESTABLE, money64(cx.game, investable));
    }

    /// OK (`EndDialog(1)`, then 464, STADION.md §5.4): the view kept, the
    /// stadiums found on the map given their place, the grounds saved.
    fn close(&mut self, cx: &mut Cx) {
        let Some(mut grounds) = self.grounds.take() else { return };
        grounds.set_view(&self.view);
        let Some(records) = cx.state.records.as_mut() else { return };
        grounds.store(records);
    }
}

/// The info (467, `CMapInfoDialog`, §8.1): the land, the buildings
/// finished and being built, their upkeep, the investments, the staff's
/// wages.
pub struct GroundsInfo;

const INFO_TITLE: u16 = 6250;
const INFO_TITLE_TEXT: u16 = 4433;
const HECTARES: u16 = 6251;
const HECTARES_TEXT: u16 = 5051;
const FINISHED: u16 = 6252;
const BEING_BUILT: u16 = 6253;
const UPKEEP: u16 = 6254;
const INVESTED_FOOTBALL: u16 = 6255;
const INVESTED_OTHER: u16 = 6256;
const INVESTED_ALL: u16 = 6257;
const WAGES: u16 = 6289;
/// The quarter hectares a tile is.
const TILES_A_HECTARE: i32 = 4;

impl Screen for GroundsInfo {
    fn ids(&self) -> &[u16] {
        &[INFO]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let club = own_team(cx).map(a3_core::records::Team::name).unwrap_or_default();
        set(view, dialog, INFO_TITLE, game.text(INFO_TITLE_TEXT).replacen("%s", &club, 1));
        let Some(grounds) = &cx.state.grounds_shown else { return };
        let info = grounds.info();
        let hectares = info.area / TILES_A_HECTARE;
        set(
            view,
            dialog,
            HECTARES,
            game.text(HECTARES_TEXT).replacen("%d", &hectares.to_string(), 1),
        );
        set(view, dialog, FINISHED, info.finished.to_string());
        set(view, dialog, BEING_BUILT, info.building.to_string());
        set(view, dialog, UPKEEP, money(game, info.upkeep.into()));
        let (football, other) = grounds.invested();
        set(view, dialog, INVESTED_FOOTBALL, money64(game, football));
        set(view, dialog, INVESTED_OTHER, money64(game, other));
        set(view, dialog, INVESTED_ALL, money64(game, football.wrapping_add(other)));
        set(view, dialog, WAGES, money64(game, grounds.wages()));
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        closed(cx, target)
    }
}

/// Only the OK does anything in the sub-dialogs.
fn closed(cx: &Cx, target: Target) -> Outcome {
    match target {
        Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
            Outcome::Close(Reply::Ok)
        }
        _ => Outcome::Stay,
    }
}

/// The staff (465, `CMapAngestellteDialog`, §8.2): a row a job with
/// anyone in it, its count and their wages, sorted.
pub struct GroundsStaff;

const STAFF_LIST: u16 = 5261;
/// The most jobs the list takes (`FUN_007b877a`).
const MOST_JOBS: usize = 60;

impl Screen for GroundsStaff {
    fn ids(&self) -> &[u16] {
        &[STAFF]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(grounds) = &cx.state.grounds_shown else { return };
        let mut rows: Vec<String> = (0..grounds::JOBS.len())
            .filter(|&job| grounds.staff(job) > 0)
            .take(MOST_JOBS)
            .map(|job| {
                let (text, wage) = grounds::JOBS[job];
                let n = grounds.staff(job);
                let wages = money(game, i64::from(n.wrapping_mul(wage)));
                format!("{}\t{n}\t{wages}", game.text(text))
            })
            .collect();
        rows.sort_by_key(|row| order_key(row));
        if let Some(list) = control(dialog, STAFF_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        closed(cx, target)
    }
}

/// The fans' opinion (468, `CMapFanDialog`, §8.3): their complaints, or
/// that they have none.
pub struct GroundsFans;

const FAN_LIST: u16 = 5996;
/// "Die Fans sind wunschlos glücklich!", a literal of the program
/// (`0x8b0f4c`).
const NO_COMPLAINTS: &str = "Die Fans sind wunschlos glücklich!";
/// The most rows the list takes (`FUN_007c6c7b`).
const MOST_COMPLAINTS: usize = 40;

impl Screen for GroundsFans {
    fn ids(&self) -> &[u16] {
        &[FANS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        use chrono::Datelike;
        let (game, dialog) = (cx.game, cx.dialog);
        let (Some(grounds), Some(stadiums)) = (&cx.state.grounds_shown, &cx.state.grounds_stadiums)
        else {
            return;
        };
        let Some(sub) = &stadiums.sub else { return };
        let team = own_team(cx);
        let month = cx
            .state
            .season
            .as_ref()
            .map_or(0, |s| a3_core::calendar::date(s.year, s.day).month() as i32);
        let fans = grounds::Fans {
            look: grounds.look(stadiums.played.min(grounds::STADIUMS - 1)),
            stadium: sub,
            country: team.map_or(0, a3_core::records::Team::country),
            level: team.map_or(0, a3_core::records::Team::level),
            month,
        };
        let mut rows: Vec<String> = grounds
            .complaints(&fans)
            .into_iter()
            .take(MOST_COMPLAINTS)
            .map(|text| game.text(text).to_owned())
            .collect();
        if rows.is_empty() {
            rows.push(NO_COMPLAINTS.to_owned());
        }
        if let Some(list) = control(dialog, FAN_LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        closed(cx, target)
    }
}

/// What is being built (469, `CMapImBau`, §8.4): weeks left, the
/// building, what it costs; or that nothing is.
pub struct GroundsWorks;

const WORKS_LIST: u16 = 5261;
/// "%d Wo.\t%s\t%s" and "Derzeit wird nicht gebaut!".
const WORKS_ROW: u16 = 4363;
const NOTHING_BUILT: u16 = 4362;

impl Screen for GroundsWorks {
    fn ids(&self) -> &[u16] {
        &[WORKS]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(grounds) = &cx.state.grounds_shown else { return };
        let mut rows: Vec<String> = grounds
            .works_in_progress()
            .into_iter()
            .map(|(weeks, kind, price)| {
                let name = grounds::TYPES.get(kind as usize).map_or("", |t| game.text(t.text));
                super::finances::printf(
                    &game.text(WORKS_ROW).replace("\\t", "\t"),
                    &[&weeks, &name, &money(game, price)],
                )
            })
            .collect();
        // The line that nothing is has one column across the row
        // (`FUN_007a94db(row, 1, 0x1cb87a8)`).
        let nothing = rows.is_empty();
        if nothing {
            rows.push(game.text(NOTHING_BUILT).to_owned());
        }
        if let Some(list) = control(dialog, WORKS_LIST).and_then(|c| view.list_mut(c)) {
            list.spans = vec![nothing];
            list.rows = rows;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        closed(cx, target)
    }
}
