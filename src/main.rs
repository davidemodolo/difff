mod diff;
mod renderer;
mod ui;

use gtk4::{gdk, glib, prelude::*};

fn main() -> glib::ExitCode {
    let app = gtk4::Application::builder()
        .application_id("com.opencitylabs.difff")
        .build();

    app.connect_startup(|_| load_css());
    app.connect_activate(ui::build_ui);

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
