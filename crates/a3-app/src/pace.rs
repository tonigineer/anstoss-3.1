//! Frame pacing. On Wayland with the NVIDIA driver, presenting a frame does
//! not wait for the display, so the app would render as fast as it can —
//! well over a thousand frames a second, with uneven gaps between them. This
//! holds every frame to the refresh rate of the monitor the window is on;
//! where vsync does wait, it never has to.

use std::time::{Duration, Instant};

use bevy::ecs::system::NonSendMarker;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;

const FALLBACK_HZ: f64 = 60.0;
/// How often to look up which monitor the window is on.
const RECHECK: Duration = Duration::from_secs(1);
/// Sleeping wakes up late by up to a millisecond or so; the last stretch
/// before a frame is due is spent spinning instead.
const SPIN: Duration = Duration::from_micros(1500);

pub fn plugin(app: &mut App) {
    app.insert_resource(Pace { due: Instant::now(), hz: FALLBACK_HZ, checked: Instant::now() })
        .add_systems(Last, wait);
}

#[derive(Resource)]
struct Pace {
    /// When the current frame may end.
    due: Instant,
    hz: f64,
    /// When the refresh rate is to be looked up again.
    checked: Instant,
}

fn wait(mut pace: ResMut<Pace>, window: Single<Entity, With<PrimaryWindow>>, _main: NonSendMarker) {
    let now = Instant::now();
    if now >= pace.checked {
        pace.checked = now + RECHECK;
        // Winit's windows live on the main thread, which `NonSendMarker` keeps
        // this system on.
        let hz = WINIT_WINDOWS.with_borrow(|windows| {
            let monitor = windows.get_window(*window)?.current_monitor()?;
            Some(f64::from(monitor.refresh_rate_millihertz()?) / 1000.0)
        });
        pace.hz = hz.filter(|&hz| hz >= 30.0).unwrap_or(FALLBACK_HZ);
    }

    // After a slow frame start afresh rather than racing to catch up.
    let frame = Duration::from_secs_f64(1.0 / pace.hz);
    let due = (pace.due + frame).max(now);
    pace.due = due;
    if let Some(sleep) = due.checked_duration_since(now).and_then(|d| d.checked_sub(SPIN)) {
        std::thread::sleep(sleep);
    }
    while Instant::now() < due {
        std::hint::spin_loop();
    }
}
