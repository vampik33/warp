//! Backend router for the `elements` module. The module path is ungated and
//! stable; its contents are backend-routed. M8 adds the `tui` arm
//! (`elements/tui/`) when the TUI element library is absorbed into this crate.
cfg_if::cfg_if! {
    if #[cfg(not(feature = "tui"))] {
        mod gui;
        pub use gui::*;
    }
}
