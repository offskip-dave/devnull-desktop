mod app;
mod body;
mod emoji;
mod images;
mod prefs;
mod ui;

use std::sync::{Mutex, OnceLock};

use relm4::gtk::gio::{self, prelude::*};
use relm4::RelmApp;

use app::{App, AppMsg};
use devnull_auth::APP_ID;

pub static PENDING_URI: OnceLock<Mutex<Option<String>>> = OnceLock::new();
pub static APP_SENDER: OnceLock<relm4::Sender<AppMsg>> = OnceLock::new();

fn pending() -> &'static Mutex<Option<String>> {
    PENDING_URI.get_or_init(|| Mutex::new(None))
}

fn ingest_uri(uri: &str) {
    if !uri.contains("devnull://") {
        return;
    }
    // Any process that sees the URL writes it down so the waiting window can
    // finish sign-in even when GApplication uniqueness is unavailable (typical
    // on macOS, also possible on Linux without a session bus).
    devnull_auth::write_callback_uri(uri);
    if let Ok(mut slot) = pending().lock() {
        *slot = Some(uri.to_owned());
    }
    if let Some(sender) = APP_SENDER.get() {
        sender.emit(AppMsg::OpenUri(uri.to_owned()));
        if let Ok(mut slot) = pending().lock() {
            *slot = None;
        }
    }
}

fn take_uri_from_args(args: &[std::ffi::OsString]) {
    for arg in args {
        if let Some(s) = arg.to_str() {
            ingest_uri(s);
        }
    }
}

fn main() {
    take_uri_from_args(&std::env::args_os().collect::<Vec<_>>());
    let _ = devnull_auth::register_url_handler();

    // RelmApp::new calls gtk/adw init so a display exists before CSS is loaded.
    let relm = RelmApp::new(APP_ID);
    crate::prefs::Prefs::load();
    relm4::adw::StyleManager::default().connect_dark_notify(|_| {
        crate::prefs::reload_css();
    });

    let gtk_app = relm4::main_application();
    gtk_app.set_flags(
        gtk_app.flags()
            | gio::ApplicationFlags::HANDLES_COMMAND_LINE
            | gio::ApplicationFlags::HANDLES_OPEN,
    );
    gtk_app.connect_command_line(|app, cmdline| {
        take_uri_from_args(&cmdline.arguments());
        app.activate();
        0.into()
    });
    gtk_app.connect_open(|app, files, _| {
        for file in files {
            ingest_uri(&file.uri());
        }
        app.activate();
    });

    relm.run::<App>(());
}
