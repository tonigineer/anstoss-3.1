//! The manager's abilities (dialog 378 `CEigenschaftenAnzeige`, ERFOLGE.md
//! §B.2): his level and the seven abilities he has at it.

use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, mark_inert, set};
use crate::view::View;

pub(crate) const ABILITIES_DIALOG: u16 = 378;
const LEVEL: u16 = 4727;
/// The seven abilities' statics, in the record's order.
const FIGURES: [u16; 7] = [4695, 4696, 4697, 4698, 4699, 4700, 5611];
/// The manager's record: his level and the bits that raise it.
const LEVEL_AT: usize = 0x23;
const BONUS_AT: usize = 0x1f39;
/// What each bit is worth, and the highest a bonus reaches.
const BONUSES: [(u8, i32); 7] =
    [(0x40, 1), (0x80, 2), (0x20, 2), (0x10, 1), (0x08, 3), (0x04, 3), (0x02, -1)];
const WITHOUT_BONUS: i32 = 12;
const HIGHEST: i32 = 15;

/// The manager's standing (`FUN_006a61e5`): his level, raised by the bits
/// he has earned — only the one that lowers it counts from twelve on.
pub(crate) fn standing(manager: &a3_core::records::Manager) -> i32 {
    let v = i32::from(manager.get::<u8>(LEVEL_AT));
    let bits = manager.get::<u8>(BONUS_AT);
    if v >= WITHOUT_BONUS {
        let down = if bits & 0x02 != 0 { -1 } else { 0 };
        return (v + down).clamp(0, HIGHEST);
    }
    let by: i32 = BONUSES.iter().filter(|&&(bit, _)| bits & bit != 0).map(|&(_, by)| by).sum();
    (v + by).min(WITHOUT_BONUS).clamp(0, HIGHEST)
}

/// The screen.
pub struct Abilities;

impl Screen for Abilities {
    fn ids(&self) -> &[u16] {
        &[ABILITIES_DIALOG]
    }

    /// `0x48a5d0`'s init.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let dialog = cx.dialog;
        let Some(manager) = crate::screens::kalkulation::own_manager(cx) else { return };
        let level = standing(manager);
        set(view, dialog, LEVEL, level.to_string());
        let row = manager.abilities()[usize::try_from(level).unwrap_or(0).min(15)];
        for (id, value) in FIGURES.iter().zip(row) {
            set(view, dialog, *id, value.to_string());
        }
        mark_inert(view, dialog, |_| true);
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(c) if crate::controls::button::is_ok(&cx.dialog.controls[c]) => {
                Outcome::Close(Reply::Ok)
            }
            _ => Outcome::Stay,
        }
    }
}
