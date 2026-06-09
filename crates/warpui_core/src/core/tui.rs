//! TUI-backend-specific `App`/`AppContext` API.
//!
//! This module holds the `#[cfg(feature = "tui")]` methods and trait impls that
//! were previously scattered through `app.rs` behind per-item cfgs. The single
//! `#[cfg(feature = "tui")] mod tui;` guard in `core/mod.rs` now gates the
//! entire TUI fork; nothing here changes behavior or the public TUI API.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use anyhow::{anyhow, Result};

use super::{
    ActionType, AddWindowOptions, AnyTuiView, App, AppContext, BackendView, TuiReadView,
    TuiTypedActionView, TuiUpdateView, TuiView, TuiViewAsRef, TuiViewContext, TuiViewHandle,
    ViewType, Window,
};
use crate::{Element, EntityId, WindowId};

impl TuiViewAsRef for App {
    // Unimplemented for the same borrow-gymnastics reason as the GUI
    // [`ViewAsRef`] impl above; use [`App::read`] / [`TuiReadView`] instead.
    fn tui_view<T: TuiView>(&self, _handle: &TuiViewHandle<T>) -> &T {
        unimplemented!("Read via TuiReadView instead");
    }

    fn try_tui_view<T: TuiView>(&self, _handle: &TuiViewHandle<T>) -> Option<&T> {
        unimplemented!("Read via TuiReadView instead");
    }
}

impl TuiReadView for App {
    fn read_tui_view<T, F, S>(&self, handle: &TuiViewHandle<T>, read: F) -> S
    where
        T: TuiView,
        F: FnOnce(&T, &AppContext) -> S,
    {
        let state = self.0.borrow();
        state.read_tui_view(handle, read)
    }
}

impl TuiUpdateView for App {
    fn update_tui_view<T, F, S>(&mut self, handle: &TuiViewHandle<T>, update: F) -> S
    where
        T: TuiView,
        F: FnOnce(&mut T, &mut TuiViewContext<T>) -> S,
    {
        self.as_mut().update_tui_view(handle, update)
    }
}

/// TUI-backend view lifecycle. The `add_*`/`update_*` entry points are thin
/// wrappers that supply the TUI-specific pieces (concrete `TuiViewContext`
/// construction, `Box<T> -> Box<dyn AnyTuiView>` erasure, and typed-handle/
/// typed-action-handler construction) to the shared, backend-neutral
/// `insert_view_inner`/`insert_typed_action_view_inner`/`update_view_inner`
/// helpers that the GUI flows also use.
impl AppContext {
    /// TUI stub: the GUI presenter/render path is inert on a TUI build (the real
    /// TUI render path arrives in a later milestone via `B::Presenter`). This
    /// keeps the GUI presenter module type-checking without producing elements.
    pub fn render_view(
        &self,
        _window_id: WindowId,
        _view_id: EntityId,
    ) -> Result<Box<dyn Element>> {
        Err(anyhow!("render_view is not supported on the TUI backend"))
    }

    /// TUI stub mirroring [`render_view`]; see its note.
    pub fn render_views(
        &self,
        _window_id: WindowId,
    ) -> Result<HashMap<EntityId, Box<dyn Element>>> {
        Ok(HashMap::new())
    }

    /// Adds a [`TuiView`] to the given window, returning a strong handle to it.
    pub fn add_tui_view<T, F>(&mut self, window_id: WindowId, build_view: F) -> TuiViewHandle<T>
    where
        T: TuiView,
        F: FnOnce(&mut TuiViewContext<T>) -> T,
    {
        self.add_option_tui_view(window_id, |ctx| Some(build_view(ctx)))
            .unwrap()
    }

    /// Adds a [`TuiView`] that may decline to be created.
    pub fn add_option_tui_view<T, F>(
        &mut self,
        window_id: WindowId,
        build_view: F,
    ) -> Option<TuiViewHandle<T>>
    where
        T: TuiView,
        F: FnOnce(&mut TuiViewContext<T>) -> Option<T>,
    {
        self.insert_view_inner(
            window_id,
            |app, view_id| {
                let mut ctx = TuiViewContext::new(app, window_id, view_id);
                build_view(&mut ctx).map(|view| BackendView::into_any_view(Box::new(view)))
            },
            |app, window_id, view_id| TuiViewHandle::new(window_id, view_id, &app.ref_counts),
        )
    }

    /// Adds a [`TuiTypedActionView`], registering its typed-action handler.
    pub fn add_typed_action_tui_view<V, F>(
        &mut self,
        window_id: WindowId,
        build_view: F,
    ) -> TuiViewHandle<V>
    where
        V: TuiTypedActionView,
        F: FnOnce(&mut TuiViewContext<V>) -> V,
    {
        self.add_typed_action_tui_view_internal(window_id, build_view, None)
    }

    pub(crate) fn add_typed_action_tui_view_with_parent<V, F>(
        &mut self,
        window_id: WindowId,
        build_view: F,
        parent_view_id: EntityId,
    ) -> TuiViewHandle<V>
    where
        V: TuiTypedActionView,
        F: FnOnce(&mut TuiViewContext<V>) -> V,
    {
        self.add_typed_action_tui_view_internal(window_id, build_view, Some(parent_view_id))
    }

    fn add_typed_action_tui_view_internal<V, F>(
        &mut self,
        window_id: WindowId,
        build_view: F,
        parent_view_id: Option<EntityId>,
    ) -> TuiViewHandle<V>
    where
        V: TuiTypedActionView,
        F: FnOnce(&mut TuiViewContext<V>) -> V,
    {
        self.insert_typed_action_view_inner(
            window_id,
            parent_view_id,
            |app, view_id| {
                let mut ctx = TuiViewContext::new(app, window_id, view_id);
                let view = build_view(&mut ctx);
                BackendView::into_any_view(Box::new(view))
            },
            // The TUI backend has no presenter to mirror structural parentage onto.
            |_app, _view_id, _parent_view_id| {},
            |app| app.add_typed_action_tui::<V>(),
            |app, window_id, view_id| TuiViewHandle::new(window_id, view_id, &app.ref_counts),
        )
    }

    /// Registers the handler that dispatches to [`TuiTypedActionView::handle_action`]
    /// for the given view + action combination. Keyed by `(ActionType, ViewType)`
    /// in the same `typed_actions` registry the GUI backend uses.
    fn add_typed_action_tui<V>(&mut self)
    where
        V: TuiTypedActionView,
    {
        let handler = Box::new(
            |view: &mut (dyn AnyTuiView + 'static),
             action: &dyn Any,
             app: &mut AppContext,
             window_id: WindowId,
             view_id: EntityId| {
                let action = action
                    .downcast_ref()
                    .expect("Handlers are hashed by action type");
                let view = view
                    .as_any_mut()
                    .downcast_mut()
                    .expect("Handlers are hashed by view type");
                let mut ctx = TuiViewContext::new(app, window_id, view_id);
                V::handle_action(view, action, &mut ctx);
            },
        );

        self.typed_actions
            .entry(ActionType::of::<V::Action>())
            .or_default()
            .entry(ViewType::of::<V>())
            .or_insert(handler);
    }

    /// Renders the registered [`TuiView`] with the given id to its type-erased
    /// output (a `Box<dyn Any>` wrapping the view's `RenderOutput`), or `None`
    /// if no such view is registered. This is the TUI analogue of
    /// [`render_view`](Self::render_view): the `warpui_tui` presenter calls it to
    /// resolve a (root or child) view and downcasts the result back to the
    /// concrete boxed element to lay out and paint it.
    pub fn render_tui_view(&self, view_id: EntityId) -> Option<Box<dyn Any>> {
        let window_id = *self.view_to_window.get(&view_id)?;
        let view = self.windows.get(&window_id)?.views.get(&view_id)?;
        Some(AnyTuiView::render_tui(view.as_ref(), self))
    }

    /// Returns a handle to the window's root [`TuiView`], if it is of type `T`.
    pub fn root_view_tui<T: TuiView>(&self, window_id: WindowId) -> Option<TuiViewHandle<T>> {
        self.windows
            .get(&window_id)
            .and_then(|window| window.root_view.as_ref())
            .and_then(|root_view| root_view.clone().downcast_tui::<T>())
    }

    /// Returns all the [`TuiView`]s of type `T` within `window_id`.
    pub fn views_of_type_tui<T: TuiView>(
        &self,
        window_id: WindowId,
    ) -> Option<Vec<TuiViewHandle<T>>> {
        let ref_counts = &self.ref_counts;
        self.windows.get(&window_id).map(|window| {
            window
                .views
                .iter()
                .filter(|(_, v)| (*v).as_any().type_id() == TypeId::of::<T>())
                .map(|(view_id, _)| TuiViewHandle::new(window_id, *view_id, ref_counts))
                .collect::<Vec<TuiViewHandle<T>>>()
        })
    }

    /// Creates a new window whose root view is the [`TuiTypedActionView`] returned
    /// by `build_root_view`. Reuses the backend-agnostic window machinery.
    pub fn add_tui_window<T, F>(
        &mut self,
        options: AddWindowOptions,
        build_root_view: F,
    ) -> (WindowId, TuiViewHandle<T>)
    where
        T: TuiTypedActionView,
        F: FnOnce(&mut TuiViewContext<T>) -> T,
    {
        let (window_id, _root_view_id) =
            self.insert_window_internal(None, options, |window_id, ctx| {
                ctx.windows.insert(window_id, Window::default());
                let root_handle = ctx.add_typed_action_tui_view(window_id, build_root_view);
                let root_view_id = root_handle.id();
                ctx.windows
                    .get_mut(&window_id)
                    .expect("this window was just inserted and should still exist")
                    .root_view = Some(root_handle.into());
                root_view_id
            });
        (
            window_id,
            self.root_view_tui(window_id)
                .expect("should have just inserted a window and root view"),
        )
    }
}

impl TuiViewAsRef for AppContext {
    fn tui_view<T: TuiView>(&self, handle: &TuiViewHandle<T>) -> &T {
        let window_id = handle.window_id(self);
        if let Some(window) = self.windows.get(&window_id) {
            if let Some(view) = window.views.get(&handle.id()) {
                view.as_any()
                    .downcast_ref()
                    .expect("downcast should be type safe")
            } else {
                panic!(
                    "circular view reference for view type {}",
                    std::any::type_name::<T>()
                );
            }
        } else {
            panic!("window does not exist");
        }
    }

    fn try_tui_view<T: TuiView>(&self, handle: &TuiViewHandle<T>) -> Option<&T> {
        let window_id = handle.window_id(self);
        self.windows
            .get(&window_id)?
            .views
            .get(&handle.id())?
            .as_any()
            .downcast_ref()
    }
}

impl TuiReadView for AppContext {
    fn read_tui_view<T, F, S>(&self, handle: &TuiViewHandle<T>, read: F) -> S
    where
        T: TuiView,
        F: FnOnce(&T, &AppContext) -> S,
    {
        read(self.tui_view(handle), self)
    }
}

impl TuiUpdateView for AppContext {
    fn update_tui_view<T, F, S>(&mut self, handle: &TuiViewHandle<T>, update: F) -> S
    where
        T: TuiView,
        F: FnOnce(&mut T, &mut TuiViewContext<T>) -> S,
    {
        let window_id = handle.window_id(self);
        let view_id = handle.id();
        self.update_view_inner(window_id, view_id, |view, app| {
            let mut ctx = TuiViewContext::new(app, window_id, view_id);
            update(
                view.as_any_mut()
                    .downcast_mut()
                    .expect("Downcast is type safe"),
                &mut ctx,
            )
        })
    }
}
