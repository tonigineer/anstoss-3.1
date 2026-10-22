//! The club grounds (dialog 466, `CMapDialog`; STADION-ANSICHT.md §4–7 in
//! the RE repo): the map of the club's land and what stands on it, at three
//! zooms, with its minimap; the modes, the categories and the objects to
//! build. Buying, building, selling and pulling down, and the four
//! sub-dialogs, are not built yet.

use std::collections::HashMap;

use a3_core::cycle::dialogs::GROUNDS;
use a3_core::grounds::{self, Category, Grounds, View as MapView};
use a3_files::rch::{Align, Control, Dialog, Kind};
use bevy_math::{Rect, Vec2};

use super::finances::money64;
use super::playerinfo::money;
use super::squad::own_team;
use super::{Args, Cx, Outcome, Reply, Screen, control, mark_inert, set};
use crate::buildings::MapArt;
use crate::controls::bitmap::Picture;
use crate::pointer::Target;
use crate::view::View;

const CAPTION_ID: u16 = 9935;
/// The map and the minimap the screen draws, over the file's frames.
const MAP: u16 = 9940;
const MINIMAP: u16 = 9941;
const MAP_AT: Rect = Rect { min: Vec2::new(2.0, 25.0), max: Vec2::new(451.0, 474.0) };
const MAP_SIZE: u32 = 449;
const MINIMAP_AT: Rect = Rect { min: Vec2::new(535.0, 71.0), max: Vec2::new(636.0, 172.0) };
/// Where a minimap click puts the view (`0x7c1a00`): from these, not the
/// minimap's corner, a tile left and four up of what was clicked.
const MINIMAP_CLICK: (i32, i32) = (536 - 535, 67 - 71);
const ZOOM_IN: u16 = 5969;
const ZOOM_OUT: u16 = 5970;
/// The sub-dialogs: info, staff, fans, what is being built — not built.
const SUBDIALOGS: [u16; 4] = [5971, 5774, 5972, 5979];
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
            if let Some(looks) = cx.state.grounds_looks {
                g.set_looks(&looks);
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
            c.cells = Some(1.0);
        }
        self.draw(cx, view);
        // The sub-dialogs are not built.
        let works = |t: Target| !matches!(t, Target::Button(c) if dialog.controls[c].id.is_some_and(|id| SUBDIALOGS.contains(&id)));
        mark_inert(view, dialog, works);
    }

    fn input(&mut self, cx: &mut Cx, view: &mut View, target: Target, _: bool) -> Outcome {
        let (game, dialog) = (cx.game, cx.dialog);
        let id = |c: usize| dialog.controls[c].id.unwrap_or(0);
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&dialog.controls[c]) => {
                self.close(cx);
                Outcome::Close(Reply::Ok)
            }
            Target::Button(c) if matches!(id(c), ZOOM_IN | ZOOM_OUT) => {
                let zoom = self.view.zoom + if id(c) == ZOOM_IN { 1 } else { -1 };
                if (0..=2).contains(&zoom) {
                    self.view = self.view.zoomed(zoom);
                    self.draw(cx, view);
                }
                Outcome::Stay
            }
            Target::Cell { control: c, row, column } if id(c) == MINIMAP => {
                let n = self.view.tiles;
                let x = (column as i32 - MINIMAP_CLICK.0) * 32 - n * 16;
                let y = (row as i32 - MINIMAP_CLICK.1) * 32 - n * 16;
                self.view.scroll = (x, y);
                self.view.clamp();
                self.draw(cx, view);
                Outcome::Stay
            }
            Target::Option { control: c, item } if id(c) == MODES => {
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
        }
    }
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

    /// The object chosen: its name, price and, for a building, its weeks
    /// (`FUN_007c1870`).
    fn show_object(&self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let row = control(dialog, OBJECTS).and_then(|c| view.combo(c)).and_then(|c| c.selected);
        let Some(&k) = row.and_then(|r| self.objects.get(r)) else { return };
        let kind = &grounds::TYPES[k];
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

    /// The map and the minimap as the view stands.
    fn draw(&mut self, cx: &Cx, view: &mut View) {
        let (game, dialog) = (cx.game, cx.dialog);
        let Some(grounds) = &self.grounds else { return };
        let map = self.art.map(game, grounds, &self.view, MAP_SIZE);
        let mini = crate::buildings::minimap(grounds, &self.view);
        for (id, image) in [(MAP, map), (MINIMAP, mini)] {
            self.drawn += 1;
            if let Some(bitmap) = control(dialog, id).and_then(|c| view.bitmap_mut(c)) {
                bitmap.picture = Some(Picture::drawn(self.drawn, image));
            }
        }
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
