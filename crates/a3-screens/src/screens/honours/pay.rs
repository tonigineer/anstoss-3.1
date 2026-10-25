//! What the manager's seasons have paid him (dialog 84
//! `CGehaltsentwicklungDialog`; ERFOLGE.md §A.2): a row per season, the
//! running one at its end, and below the average and the total — the
//! average per year, as `FUN_004b3bd1` computes it, despite its label.

use std::collections::HashMap;

use a3_files::rch::Dialog;

use crate::pointer::Target;
use crate::screens::playerinfo::money;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, caption, control, mark_inert, set};
use crate::view::View;

pub(crate) const PAY_DIALOG: u16 = 84;
const CAPTION_ID: u16 = 9942;
const LIST: u16 = 5248;
const AVERAGE: u16 = 5591;
const TOTAL: u16 = 1860;
/// The seasons behind him and what each paid, four bytes apiece; the
/// running season's pay grows in the slot after them (`FUN_004b3bd1`:
/// mgr +0x1758 and +0x175c, in the record +0x18a8 and +0x18ac).
const SEASONS_AT: usize = 0x18a8;
const PAY_AT: usize = 0x18ac;
/// Twenty seasons fit; at the twentieth the oldest drops out.
const KEPT: usize = 19;

/// The window's own caption bar, which the original fills with the
/// manager and the date after the title (ERFOLGE.md §A.2).
pub(crate) fn compose(dialogs: &mut HashMap<u16, Dialog>) {
    let Some(dialog) = dialogs.get_mut(&PAY_DIALOG) else { return };
    dialog.controls.insert(0, caption(dialog.frame, CAPTION_ID, String::new()));
}

/// The screen.
pub struct Pay;

impl Screen for Pay {
    fn ids(&self) -> &[u16] {
        &[PAY_DIALOG]
    }

    /// `FUN_004b3bd1`.
    fn open(&mut self, cx: &mut Cx, view: &mut View, _: Args) {
        let (game, dialog) = (cx.game, cx.dialog);
        if let Some(season) = cx.state.season.as_ref() {
            let name = season
                .manager()
                .map_or(String::new(), |m| format!("{} {}", m.first_name, m.last_name));
            let date = crate::screens::hub::date_text(game, season.date());
            set(view, dialog, CAPTION_ID, format!("{} {}, {}", dialog.title, name, date));
        }
        let Some(manager) = crate::screens::kalkulation::own_manager(cx) else { return };
        let Some(records) = cx.state.records.as_ref() else { return };
        let [_, month, year] = records.globals.date();
        let seasons = i32::from(manager.get::<i16>(SEASONS_AT)).clamp(0, KEPT as i32);
        let mut rows = Vec::new();
        let mut sum: f32 = 0.0;
        for k in 0..=seasons {
            let season = i32::from(year) - seasons + k;
            let pay = manager.get::<i32>(PAY_AT + 4 * usize::try_from(k).unwrap_or(0));
            sum += pay as f32;
            rows.push(format!("{season:04}/{:04}\t{}", season + 1, money(game, pay.into())));
        }
        // The seasons count whole, the running one by its months since July.
        let months = 12 * seasons
            + if month >= 7 { i32::from(month) - 7 } else { i32::from(month) + 5 };
        set(view, dialog, TOTAL, money(game, sum as i64));
        set(view, dialog, AVERAGE, money(game, (sum / months.max(1) as f32 * 12.0) as i64));
        if let Some(list) = control(dialog, LIST).and_then(|c| view.list_mut(c)) {
            list.rows = rows;
            list.selected = None;
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
