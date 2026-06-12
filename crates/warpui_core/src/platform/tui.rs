//! TUI-backend platform items.

/// TUI counterpart of the GUI `FontDBExt` extension trait: an empty marker
/// with a blanket impl, so every [`FontDB`](super::FontDB) implementor
/// satisfies the routed supertrait bound without any glyph-rasterization or
/// text-layout machinery.
pub trait FontDBExt {}

impl<T: ?Sized> FontDBExt for T {}
