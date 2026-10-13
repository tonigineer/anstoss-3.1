//! The game itself, without a window: the football world of a new game and
//! the data it starts from, the calendar, the first season with its job
//! offers and line-ups, the random numbers they draw, and the game cycle
//! that asks for the screens (see [`cycle`]).

pub mod calendar;
pub mod cycle;
pub mod data;
pub mod jobs;
pub mod lineup;
pub mod load;
pub mod newgame;
pub mod random;
pub mod records;
pub mod season;
pub mod state;
pub mod world;
