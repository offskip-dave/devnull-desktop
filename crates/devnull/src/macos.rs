//! Quartz window chrome, dock icon, cursors, and renderer defaults. No-ops on Linux.

#![allow(unexpected_cfgs)]

use relm4::adw;
use relm4::gtk;
use relm4::gtk::gio;
use relm4::gtk::gio::prelude::{ActionMapExt, ApplicationExt};
use relm4::gtk::prelude::*;

use devnull_auth::APP_ID;

/// Must run before GTK is initialized.
pub fn prepare_backend() {
    // GTK picks ngl/gl on Quartz. Forcing cairo here blanked the window.
}

pub fn install_app_actions() {
    let app = relm4::main_application();
    if app.lookup_action("quit").is_none() {
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate(|_, _| {
            relm4::main_application().quit();
        });
        app.add_action(&quit);
    }
    app.set_accels_for_action("app.quit", &["<Primary>q"]);
    app.set_accels_for_action("win.close", &["<Primary>w"]);
    app.set_accels_for_action("win.fullscreen", &["<Meta><Control>f", "F11"]);

    #[cfg(target_os = "macos")]
    {
        let menubar = gio::Menu::new();
        let app_menu = gio::Menu::new();
        app_menu.append(Some("Quit /dev/null"), Some("app.quit"));
        menubar.append_submenu(Some("/dev/null"), &app_menu);
        let view = gio::Menu::new();
        view.append(Some("Enter Full Screen"), Some("win.fullscreen"));
        menubar.append_submenu(Some("View"), &view);
        app.set_menubar(Some(&menubar));
    }
}

pub fn decorate_window(window: &adw::ApplicationWindow) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
    }
    #[cfg(target_os = "macos")]
    {
        window.set_decorated(true);
        window.set_resizable(true);
        window.set_hide_on_close(false);

        let fullscreen = gio::SimpleAction::new("fullscreen", None);
        let win = window.clone();
        fullscreen.connect_activate(move |_, _| {
            if win.is_fullscreen() {
                win.unfullscreen();
            } else {
                win.fullscreen();
            }
        });
        window.add_action(&fullscreen);
        track_cursors(window);
    }
}

/// GTK's Quartz backend often never maps `GtkWidget:cursor` onto `NSCursor`.
/// Follow the pointer and apply the native hand / I-beam ourselves.
pub fn track_cursors(widget: &impl IsA<gtk::Widget>) {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = widget;
    }
    #[cfg(target_os = "macos")]
    {
        install_cursor_tracker(widget.as_ref());
    }
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CursorKind {
    Arrow,
    Pointer,
    Text,
}

#[cfg(any(test, target_os = "macos"))]
fn cursor_kind(name: Option<&str>) -> CursorKind {
    match name {
        Some("pointer" | "hand" | "hand2") => CursorKind::Pointer,
        Some("text" | "xterm" | "ibeam") => CursorKind::Text,
        _ => CursorKind::Arrow,
    }
}

#[cfg(target_os = "macos")]
fn install_cursor_tracker(widget: &gtk::Widget) {
    if widget.has_css_class("dn-cursor-track") {
        return;
    }
    widget.add_css_class("dn-cursor-track");
    let motion = gtk::EventControllerMotion::new();
    motion.set_propagation_phase(gtk::PropagationPhase::Capture);
    motion.connect_enter(apply_cursor_at);
    motion.connect_motion(apply_cursor_at);
    motion.connect_leave(|_| set_nscursor(CursorKind::Arrow));
    widget.add_controller(motion);
}

#[cfg(target_os = "macos")]
fn apply_cursor_at(controller: &gtk::EventControllerMotion, x: f64, y: f64) {
    let Some(root) = controller.widget() else {
        return;
    };
    let kind = root
        .pick(x, y, gtk::PickFlags::DEFAULT)
        .map_or(CursorKind::Arrow, |picked| cursor_kind_for(&picked));
    set_nscursor(kind);
}

#[cfg(target_os = "macos")]
fn cursor_kind_for(widget: &gtk::Widget) -> CursorKind {
    let mut node = Some(widget.clone());
    while let Some(current) = node {
        if let Some(cursor) = current.cursor() {
            return cursor_kind(cursor.name().as_deref());
        }
        if let Ok(label) = current.clone().downcast::<gtk::Label>() {
            if label.current_uri().is_some() {
                return CursorKind::Pointer;
            }
            if label.is_selectable() {
                return CursorKind::Text;
            }
        }
        if is_pointer_widget(&current) {
            return CursorKind::Pointer;
        }
        if is_text_widget(&current) {
            return CursorKind::Text;
        }
        node = current.parent();
    }
    CursorKind::Arrow
}

#[cfg(target_os = "macos")]
fn is_pointer_widget(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::Button>()
        || widget.is::<gtk::CheckButton>()
        || widget.is::<gtk::MenuButton>()
        || widget.is::<gtk::Switch>()
        || widget.is::<gtk::DropDown>()
        || widget.is::<adw::SplitButton>()
}

#[cfg(target_os = "macos")]
fn is_text_widget(widget: &gtk::Widget) -> bool {
    widget.is::<gtk::TextView>()
        || widget.is::<gtk::Entry>()
        || widget.is::<gtk::SearchEntry>()
        || widget.is::<gtk::PasswordEntry>()
}

#[cfg(target_os = "macos")]
fn set_nscursor(kind: CursorKind) {
    thread_local! {
        static LAST: std::cell::Cell<CursorKind> = const { std::cell::Cell::new(CursorKind::Arrow) };
    }
    if LAST.with(|last| last.get() == kind) {
        return;
    }
    LAST.with(|last| last.set(kind));
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let class = class!(NSCursor);
        let cursor: *mut Object = match kind {
            CursorKind::Pointer => msg_send![class, pointingHandCursor],
            CursorKind::Text => msg_send![class, IBeamCursor],
            CursorKind::Arrow => msg_send![class, arrowCursor],
        };
        if !cursor.is_null() {
            let _: () = msg_send![cursor, set];
        }
    }
}

pub fn set_dock_icon() {
    gtk::Window::set_default_icon_name(APP_ID);
    #[cfg(target_os = "macos")]
    {
        let Some(pixbuf) = dock_icon_pixbuf() else {
            return;
        };
        let png = std::env::temp_dir().join("sh.devnull.Client-dock.png");
        if pixbuf.savev(&png, "png", &[]).is_err() {
            return;
        }
        set_ns_application_icon(&png);
    }
}

/// Render the Mark into a 256² canvas with ~10% transparent margin so Dock /
/// Cmd-Tab optical size matches other macOS icons (full-bleed SVG looks huge).
#[cfg(target_os = "macos")]
fn dock_icon_pixbuf() -> Option<gtk::gdk_pixbuf::Pixbuf> {
    use gtk::gdk_pixbuf::{Colorspace, Pixbuf};

    const CANVAS: i32 = 256;
    // Apple's icon grid leaves the squircle inside the canvas; ~80% content.
    const CONTENT: i32 = 204;

    let svg = crate::ui::app_icon_path();
    let source = Pixbuf::from_file_at_scale(svg, CONTENT, CONTENT, true).ok()?;
    let canvas = Pixbuf::new(Colorspace::Rgb, true, 8, CANVAS, CANVAS)?;
    canvas.fill(0x0000_0000);
    let w = source.width().min(CONTENT);
    let h = source.height().min(CONTENT);
    let x = (CANVAS - w) / 2;
    let y = (CANVAS - h) / 2;
    source.copy_area(0, 0, w, h, &canvas, x, y);
    Some(canvas)
}

#[cfg(target_os = "macos")]
fn set_ns_application_icon(png: &std::path::Path) {
    use std::ffi::CString;

    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    let Some(path) = png.to_str() else {
        return;
    };
    let Ok(c_path) = CString::new(path) else {
        return;
    };
    unsafe {
        let ns_path: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: c_path.as_ptr()];
        if ns_path.is_null() {
            return;
        }
        let img_alloc: *mut Object = msg_send![class!(NSImage), alloc];
        let img: *mut Object = msg_send![img_alloc, initWithContentsOfFile: ns_path];
        if img.is_null() {
            return;
        }
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, setApplicationIconImage: img];
        let _: () = msg_send![img, release];
    }
}

#[cfg(test)]
mod tests {
    use super::{cursor_kind, CursorKind};

    #[test]
    fn maps_css_cursor_names() {
        assert_eq!(cursor_kind(Some("pointer")), CursorKind::Pointer);
        assert_eq!(cursor_kind(Some("hand2")), CursorKind::Pointer);
        assert_eq!(cursor_kind(Some("text")), CursorKind::Text);
        assert_eq!(cursor_kind(Some("xterm")), CursorKind::Text);
        assert_eq!(cursor_kind(Some("default")), CursorKind::Arrow);
        assert_eq!(cursor_kind(None), CursorKind::Arrow);
    }
}
