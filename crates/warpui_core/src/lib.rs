#[macro_use]
extern crate num_derive;

pub mod accessibility;
pub mod actions;
mod app_focus_telemetry;
pub mod assets;
pub mod r#async;
pub mod clipboard;
pub mod clipboard_utils;
mod core;
pub mod elements;
pub mod event;
pub mod fonts;
pub mod image_cache;
pub mod keymap;
pub mod modals;
pub mod notification;
pub mod platform;
pub mod prelude;
pub mod telemetry;
#[cfg(test)]
mod test;
pub mod text;
pub mod text_selection_utils;
pub mod time;
pub mod traces;
pub mod units;
pub mod util;
pub mod windowing;
pub mod zoom;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "tui"))] {
        mod debug;
        pub mod integration;
        pub mod presenter;
        pub mod rendering;
        pub mod scene;
        pub mod text_layout;
        pub mod ui_components;
    }
}

pub use assets::AssetProvider;
pub use clipboard::Clipboard;
pub use event::Event;
pub use pathfinder_color as color;
// Keep `geometry` as its own public module alias alongside `color`.
pub use pathfinder_geometry as geometry;
pub use zoom::ZoomFactor;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "tui"))] {
        pub use elements::Element;
        pub use presenter::{
            AfterLayoutContext, EventContext, LayoutContext, PaintContext, Presenter,
            SizeConstraint,
        };
        pub use scene::{ClipBounds, Scene};
    }
}

pub use crate::core::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gradient {
    pub start: color::ColorU,
    pub end: color::ColorU,
}
