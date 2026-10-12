//! What the remastered look draws behind a screen's controls where the
//! classic one has it painted into the picture: on the line-up the pitch and
//! the panels, on the week the days before they have run, the panels and the
//! strip along the bottom.

use bevy::prelude::*;

use a3_screens::controls::bitmap::Picture;
use a3_screens::screens::calendar::{
    DAY_TILE_SIZE, DAY_TILE_TOPS, PANEL_LEFT, PANEL_TOPS, WEEK_BACKGROUNDS, WEEKS,
};
use a3_screens::screens::hub::LINEUP;

/// A part of the game's artwork, a card, or the band along a card's top — in
/// classic pixels.
pub(super) enum Decoration {
    /// A picture, stretched over its place or fitted into it.
    Picture {
        picture: Picture,
        at: Rect,
        stretch: bool,
    },
    Card(Rect),
    Band(Rect),
}

/// The pitch in the line-up's picture, the panel that compares the teams
/// and the box of the cohesion beside the column titles.
const PITCH: Rect = Rect { min: Vec2::new(8.0, 22.0), max: Vec2::new(516.0, 265.0) };
const PITCH_PICTURE: u16 = 1010;
const COMPARISON_PANEL: Rect = Rect { min: Vec2::new(452.0, 300.0), max: Vec2::new(634.0, 372.0) };
const COHESION_BOX: Rect = Rect { min: Vec2::new(454.0, 268.0), max: Vec2::new(516.0, 299.0) };
/// A week panel: the band of its title, the boxes of its charts, of the
/// next match and of the news, from its top left corner.
const PANEL_SIZE: Vec2 = Vec2::new(507.0, 100.0);
const PANEL_BAND: f32 = 18.0;
const PANEL_BOXES: [Rect; 4] = [
    Rect { min: Vec2::new(2.0, 52.0), max: Vec2::new(174.0, 82.0) },
    Rect { min: Vec2::new(178.0, 17.0), max: Vec2::new(222.0, 96.0) },
    Rect { min: Vec2::new(229.0, 17.0), max: Vec2::new(273.0, 96.0) },
    Rect { min: Vec2::new(279.0, 22.0), max: Vec2::new(497.0, 90.0) },
];
/// The week screen's bottom: the strip with the weather, and the news box.
const WEATHER_STRIP: Rect = Rect { min: Vec2::new(106.0, 436.0), max: Vec2::new(402.0, 466.0) };
const NEWS_BOX: Rect = Rect { min: Vec2::new(405.0, 416.0), max: Vec2::new(575.0, 472.0) };

/// The decorations of a screen.
pub(super) fn decorations(id: u16) -> Vec<Decoration> {
    if id == LINEUP {
        return line_up();
    }
    WEEKS.iter().position(|&w| w == id).map_or_else(Vec::new, week)
}

/// The pitch from the game's picture, and cards for the panels.
fn line_up() -> Vec<Decoration> {
    let size = PITCH.size();
    let part = [PITCH.min.x as u32, PITCH.min.y as u32, size.x as u32, size.y as u32];
    let pitch = Picture { part: Some(part), ..Picture::graphic(PITCH_PICTURE) };
    vec![
        Decoration::Picture { picture: pitch, at: PITCH, stretch: true },
        Decoration::Card(COMPARISON_PANEL),
        Decoration::Card(COHESION_BOX),
    ]
}

/// The days as they stand before they have run, a card for every one of
/// `count` panels with the band of its title and its boxes, and the strip
/// along the bottom.
fn week(count: usize) -> Vec<Decoration> {
    let (w, h) = DAY_TILE_SIZE;
    let mut decorations: Vec<Decoration> = DAY_TILE_TOPS
        .iter()
        .map(|&top| {
            let part = [0, top as u32, w, h];
            let at = Rect::new(0.0, top, w as f32, top + h as f32);
            let picture = Picture { part: Some(part), ..Picture::graphic(WEEK_BACKGROUNDS[0]) };
            Decoration::Picture { picture, at, stretch: false }
        })
        .collect();
    for &top in PANEL_TOPS[count] {
        let origin = Vec2::new(PANEL_LEFT, top);
        decorations.push(Decoration::Card(Rect::from_corners(origin, origin + PANEL_SIZE)));
        let band = Vec2::new(PANEL_SIZE.x, PANEL_BAND);
        decorations.push(Decoration::Band(Rect::from_corners(origin, origin + band)));
        decorations.extend(
            PANEL_BOXES
                .iter()
                .map(|b| Decoration::Card(Rect::from_corners(origin + b.min, origin + b.max))),
        );
    }
    decorations.extend([Decoration::Band(WEATHER_STRIP), Decoration::Card(NEWS_BOX)]);
    decorations
}
