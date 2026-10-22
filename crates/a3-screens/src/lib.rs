//! The game's dialogs and what they draw from: the installation's dialog
//! definitions, texts and graphics (see [`game`]), the state of an open
//! dialog (see [`view`]) made of its controls (see [`controls`]), where it
//! answers the pointer (see [`pointer`](mod@pointer)), and the classic
//! look drawn from it, pixel for pixel (see [`classic`]). The screens (see
//! [`screens`]) fill the dialogs and answer the game cycle's requests; the
//! [`host`] keeps them open, and [`script`]s play them without a window.

pub mod buildings;
pub mod classic;
pub mod controls;
pub mod fonts;
pub mod game;
pub mod host;
pub mod original;
pub mod pointer;
pub mod screens;
pub mod script;
pub mod view;
