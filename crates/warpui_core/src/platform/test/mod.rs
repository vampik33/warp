mod delegate;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "tui"))] {
        mod app;
        mod gui;
        pub use app::App;
    }
}

pub(crate) use delegate::WindowManager;
pub use delegate::{AppDelegate, FontDB, IntegrationTestDelegate};
