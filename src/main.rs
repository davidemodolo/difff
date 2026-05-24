mod diff;
mod recents;
mod renderer;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gdk, glib, prelude::*};

fn main() -> glib::ExitCode {
    let recents = Rc::new(RefCell::new(recents::Recents::load()));

    let app = gtk4::Application::builder()
        .application_id("com.opencitylabs.difff")
        .build();

    app.connect_startup(|_| load_css());
    {
        let recents = recents.clone();
        app.connect_activate(move |app| ui::build_ui(app, &recents));
    }

    app.run()
}

fn load_css() {
    let css = gtk4::CssProvider::new();
    css.load_from_string(&renderer::global_css());
    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().expect("could not connect to a display"),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    if let Some(settings) = gtk4::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
}
