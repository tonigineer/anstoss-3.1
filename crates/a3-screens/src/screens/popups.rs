//! The small popups the game asks and tells things in: a question to answer
//! yes or no, and a notice to take in.

pub use a3_core::cycle::dialogs::{NOTICE, QUESTION};

use crate::pointer::Target;
use crate::screens::{Args, Cx, Outcome, Reply, Screen, control, set};
use crate::view::View;

const QUESTION_TEXT: u16 = 4738;
const QUESTION_OK: u16 = 4741;
const NOTICE_TEXT: u16 = 4730;
/// The notice's caption (`CAlert`): "ANSTOSS 3 - Achtung!".
const NOTICE_CAPTION: u16 = 2502;
/// The ball of the question, playing its frames forth and back.
const QUESTION_BALL: u16 = 4740;
const QUESTION_BALL_FRAMES: std::ops::RangeInclusive<u16> = 1745..=1753;
/// The ball of the other small popups, looping.
const BALLS: [u16; 3] = [1336, 2116, 4732];
const BALL_FRAMES: std::ops::RangeInclusive<u16> = 1762..=1773;

/// A question: OK answers yes, the other button no.
pub struct Question;

impl Screen for Question {
    fn ids(&self) -> &[u16] {
        &[QUESTION]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        let text = match args {
            Args::Text(text) => Some(cx.game.text(text).to_owned()),
            Args::TextForFormation(text, formation) => {
                let manager = cx.state.season.as_ref().and_then(|s| s.manager());
                let name = manager.and_then(|m| m.formations.get(formation)).map(|f| f.0.as_str());
                Some(cx.game.text(text).replacen("%s", name.unwrap_or_default(), 1))
            }
            Args::KeepFormation(formation) => {
                let manager = cx.state.season.as_ref().and_then(|s| s.manager());
                let name = manager.and_then(|m| m.formations.get(formation)).map(|f| f.0.as_str());
                let question = &cx.game.rules.pitch.keep_question;
                Some(question.replacen("%s", name.unwrap_or_default(), 1))
            }
            _ => None,
        };
        if let Some(text) = text {
            set(view, cx.dialog, QUESTION_TEXT, text);
        }
        if let Some(ball) = control(cx.dialog, QUESTION_BALL).and_then(|b| view.anim_mut(b)) {
            ball.frames = QUESTION_BALL_FRAMES.chain(QUESTION_BALL_FRAMES.rev().skip(1)).collect();
            ball.frames.pop();
            ball.playing = true;
        }
    }

    fn input(&mut self, cx: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        let Target::Button(button) = target else { return Outcome::Stay };
        let ok = cx.dialog.controls[button].id == Some(QUESTION_OK);
        Outcome::Close(if ok { Reply::Ok } else { Reply::Cancel })
    }
}

/// A notice with a button to take it in.
pub struct Notice;

impl Screen for Notice {
    fn ids(&self) -> &[u16] {
        &[NOTICE]
    }

    fn open(&mut self, cx: &mut Cx, view: &mut View, args: Args) {
        ball(view, cx);
        let caption = cx.game.text(NOTICE_CAPTION).to_owned();
        set(view, cx.dialog, super::discipline::CAPTION_ID, caption);
        let text = match args {
            Args::Text(text) => cx.game.text(text).to_owned(),
            Args::TextFor(text, player) => {
                let name = cx.state.records.as_ref().and_then(|r| r.known_as(player));
                cx.game.text(text).replacen("%s", &name.unwrap_or_default(), 1)
            }
            _ => return,
        };
        set(view, cx.dialog, NOTICE_TEXT, text);
    }

    fn input(&mut self, _: &mut Cx, _: &mut View, target: Target, _: bool) -> Outcome {
        match target {
            Target::Button(_) => Outcome::Close(Reply::Ok),
            _ => Outcome::Stay,
        }
    }
}

/// The looping ball of a small popup, if it has one.
pub(crate) fn ball(view: &mut View, cx: &Cx) {
    for id in BALLS {
        if let Some(ball) = control(cx.dialog, id).and_then(|b| view.anim_mut(b)) {
            ball.frames = BALL_FRAMES.collect();
            ball.playing = true;
        }
    }
}
