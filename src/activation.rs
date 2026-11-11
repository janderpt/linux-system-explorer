//! xdg-activation on the native window's existing Wayland connection. The token
//! comes from the requesting application; the compositor still controls focus.
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    backend::{Backend, ObjectId},
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_surface::WlSurface},
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

pub fn startup_token() -> String {
    choose_startup_token(
        std::env::var("XDG_ACTIVATION_TOKEN").ok(),
        std::env::var("DESKTOP_STARTUP_ID").ok(),
    )
}

fn choose_startup_token(wayland: Option<String>, desktop: Option<String>) -> String {
    [wayland, desktop]
        .into_iter()
        .flatten()
        .find(|token| !token.is_empty() && token.len() <= 4096 && !token.contains('\0'))
        .unwrap_or_default()
}

pub struct Activation {
    connection: Connection,
    queue: EventQueue<State>,
    manager: XdgActivationV1,
    surface: WlSurface,
}

impl Activation {
    pub fn attach(cc: &eframe::CreationContext<'_>) -> Option<Self> {
        let RawDisplayHandle::Wayland(display) = cc.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(window) = cc.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: eframe owns the display and surface for the lifetime of this App.
        // This wrapper does not own/disconnect the display or dispatch winit's queue.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        let id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), window.surface.as_ptr().cast()) }
                .ok()?;
        let surface = WlSurface::from_id(&connection, id).ok()?;
        let (globals, queue) = registry_queue_init::<State>(&connection).ok()?;
        let manager = globals
            .bind::<XdgActivationV1, _, _>(&queue.handle(), 1..=1, ())
            .ok()?;
        Some(Self {
            connection,
            queue,
            manager,
            surface,
        })
    }

    pub fn activate(&mut self, token: &str) {
        if !token.is_empty() && !token.contains('\0') {
            let _ = self.queue.dispatch_pending(&mut State);
            self.manager.activate(token.to_owned(), &self.surface);
            let _ = self.connection.flush();
        }
    }
}

impl Drop for Activation {
    fn drop(&mut self) {
        self.manager.destroy();
        let _ = self.connection.flush();
    }
}

struct State;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
wayland_client::delegate_noop!(State: ignore XdgActivationV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_launches_prefer_valid_wayland_tokens_and_fall_back_to_startup_ids() {
        assert_eq!(
            choose_startup_token(Some("wayland-token".into()), Some("x11-id".into())),
            "wayland-token"
        );
        for unusable in [
            None,
            Some(String::new()),
            Some("bad\0token".into()),
            Some("x".repeat(4097)),
        ] {
            assert_eq!(
                choose_startup_token(unusable, Some("desktop-token".into())),
                "desktop-token"
            );
        }
        assert!(choose_startup_token(None, None).is_empty());
    }
}
