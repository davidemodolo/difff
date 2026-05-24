use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::{gdk, gio, glib, prelude::*};

use crate::diff::{process_diff, DiffRow, Stats};
use crate::recents::Recents;
use crate::renderer::{populate_views, Side};

// ── Shared application state ──────────────────────────────────────────────────

#[derive(Default)]
struct AppState {
    text_a: Option<String>,
    text_b: Option<String>,
    filename_a: String,
    filename_b: String,
    rows: Vec<DiffRow>,
    stats: Stats,
    stack: Option<gtk4::Stack>,
    recents: Option<Rc<RefCell<Recents>>>,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn build_ui(app: &gtk4::Application, recents: &Rc<RefCell<Recents>>) {
    let state: Rc<RefCell<AppState>> = Rc::new(RefCell::new(AppState::default()));
    let recents = recents.clone();

    // ── Stack for welcome / diff pages ─────────────────────────────────────
    let stack = gtk4::Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    {
        let mut s = state.borrow_mut();
        s.stack = Some(stack.clone());
        s.recents = Some(recents.clone());
    }

    // ── Window ──────────────────────────────────────────────────────────────
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("difff")
        .default_width(1400)
        .default_height(900)
        .css_classes(vec!["main-window"])
        .build();

    // ── Root layout ─────────────────────────────────────────────────────────
    let root = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    // Defer: root goes into a Stack below, after welcome page is built.

    // ── Toolbar ─────────────────────────────────────────────────────────────
    let toolbar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(8)
        .css_classes(vec!["toolbar"])
        .build();
    root.append(&toolbar);

    let btn_left = gtk4::Button::with_label("Load Left");
    let label_left = gtk4::Label::builder()
        .label("—")
        .css_classes(vec!["filename-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::Start)
        .max_width_chars(28)
        .build();

    let btn_right = gtk4::Button::with_label("Load Right");
    let label_right = gtk4::Label::builder()
        .label("—")
        .css_classes(vec!["filename-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::Start)
        .max_width_chars(28)
        .build();

    let spacer_left = gtk4::Box::builder().hexpand(true).build();
    let spacer_right = gtk4::Box::builder().hexpand(true).build();

    let btn_both = gtk4::Button::with_label("Open Both…");

    toolbar.append(&btn_left);
    toolbar.append(&label_left);
    toolbar.append(&spacer_left);
    toolbar.append(&btn_both);
    toolbar.append(&spacer_right);
    toolbar.append(&label_right);
    toolbar.append(&btn_right);

    // ── Stats bar ────────────────────────────────────────────────────────────
    let stats_bar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(16)
        .css_classes(vec!["stats-bar"])
        .build();
    root.append(&stats_bar);

    let lbl_added = stat_label("added");
    let lbl_removed = stat_label("removed");
    let lbl_modified = stat_label("modified");
    let lbl_format = stat_label("format");
    let lbl_chars = stat_label("chars");

    stats_bar.append(&lbl_added);
    stats_bar.append(&lbl_removed);
    stats_bar.append(&lbl_modified);
    stats_bar.append(&lbl_format);
    stats_bar.append(&lbl_chars);

    // ── Diff panels ─────────────────────────────────────────────────────────
    let paned = gtk4::Paned::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .vexpand(true)
        .build();
    root.append(&paned);

    // Left panel wrapper
    let left_panel = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .vexpand(true)
        .build();

    let header_left = gtk4::Label::builder()
        .label("− Left")
        .css_classes(vec!["panel-header-left"])
        .xalign(0.0)
        .build();
    left_panel.append(&header_left);

    let (scroll_l, view_l) = make_panel();
    left_panel.append(&scroll_l);

    // Right panel wrapper
    let right_panel = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .vexpand(true)
        .build();

    let header_right = gtk4::Label::builder()
        .label("+ Right")
        .css_classes(vec!["panel-header-right"])
        .xalign(0.0)
        .build();
    right_panel.append(&header_right);

    let (scroll_r, view_r) = make_panel();
    right_panel.append(&scroll_r);

    paned.set_start_child(Some(&left_panel));
    paned.set_end_child(Some(&right_panel));

    // Set initial pane position after the window has been allocated.
    let paned_c = paned.clone();
    window.connect_map(move |w| {
        let w_width = w.width();
        if w_width > 0 {
            paned_c.set_position(w_width / 2);
        }
    });

    // ── Scroll synchronisation ───────────────────────────────────────────────
    // Cell<bool> is cheaper than RefCell<bool> for a plain Copy flag.
    let lock: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let adj_l = scroll_l.vadjustment();
    let adj_r = scroll_r.vadjustment();

    {
        let adj_r = adj_r.clone();
        let lock = lock.clone();
        adj_l.connect_value_changed(move |adj| {
            if lock.get() { return; }
            lock.set(true);
            adj_r.set_value(adj.value());
            lock.set(false);
        });
    }
    {
        let adj_l = adj_l.clone();
        let lock = lock.clone();
        adj_r.connect_value_changed(move |adj| {
            if lock.get() { return; }
            lock.set(true);
            adj_l.set_value(adj.value());
            lock.set(false);
        });
    }

    // ── Drag-and-drop on each panel (with capture phase for Wayland) ────────
    setup_drop_target(&left_panel, Side::Left, &window, &state, &view_l, &view_r,
                      &label_left, &label_right, &header_left, &header_right,
                      &lbl_added, &lbl_removed,
                      &lbl_modified, &lbl_format, &lbl_chars);
    setup_drop_target(&right_panel, Side::Right, &window, &state, &view_l, &view_r,
                      &label_left, &label_right, &header_left, &header_right,
                      &lbl_added, &lbl_removed,
                      &lbl_modified, &lbl_format, &lbl_chars);

    // ── Ctrl+V paste (clipboard file URIs) ──────────────────────────────────
    setup_clipboard_paste(
        &window, &state, &view_l, &view_r,
        &label_left, &label_right, &header_left, &header_right,
        &lbl_added, &lbl_removed,
        &lbl_modified, &lbl_format, &lbl_chars,
    );

    // ── File chooser buttons ─────────────────────────────────────────────────
    let connect_btn = |btn: &gtk4::Button, side: Side| {
        let window = window.clone();
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        let label_left = label_left.clone();
        let label_right = label_right.clone();
        let header_left = header_left.clone();
        let header_right = header_right.clone();
        let lbl_added = lbl_added.clone();
        let lbl_removed = lbl_removed.clone();
        let lbl_modified = lbl_modified.clone();
        let lbl_format = lbl_format.clone();
        let lbl_chars = lbl_chars.clone();
        btn.connect_clicked(move |_| {
            open_file_dialog(
                &window, side, &state, &view_l, &view_r,
                &label_left, &label_right, &header_left, &header_right,
                &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
        });
    };
    connect_btn(&btn_left, Side::Left);
    connect_btn(&btn_right, Side::Right);

    // ── "Open Both…" button ──────────────────────────────────────────────────
    {
        let window = window.clone();
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        let label_left = label_left.clone();
        let label_right = label_right.clone();
        let header_left = header_left.clone();
        let header_right = header_right.clone();
        let lbl_added = lbl_added.clone();
        let lbl_removed = lbl_removed.clone();
        let lbl_modified = lbl_modified.clone();
        let lbl_format = lbl_format.clone();
        let lbl_chars = lbl_chars.clone();
        btn_both.connect_clicked(move |_| {
            open_both_dialog(
                &window, &state, &view_l, &view_r,
                &label_left, &label_right, &header_left, &header_right,
                &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
        });
    }

    // ── Stack: welcome page + diff page ────────────────────────────────────
    let welcome = build_welcome_page(&window, &state, &view_l, &view_r,
                                     &label_left, &label_right,
                                     &header_left, &header_right,
                                     &lbl_added, &lbl_removed,
                                     &lbl_modified, &lbl_format, &lbl_chars,
                                     &recents);
    stack.add_titled(&welcome, Some("welcome"), "Welcome");
    stack.add_titled(&root, Some("diff"), "Diff");
    stack.set_visible_child_name("welcome");
    window.set_child(Some(&stack));

    window.present();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_welcome_page(
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
    recents: &Rc<RefCell<Recents>>,
) -> gtk4::Box {
    let page = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .css_classes(vec!["welcome-page"])
        .build();

    let title = gtk4::Label::new(Some("difff"));
    title.set_css_classes(&["welcome-title"]);

    let subtitle = gtk4::Label::new(Some("Side-by-side diff viewer"));
    subtitle.set_css_classes(&["welcome-subtitle"]);

    page.append(&title);
    page.append(&subtitle);

    // Spacer
    let spacer1 = gtk4::Box::builder().height_request(24).build();
    page.append(&spacer1);

    // "Open Files…" button
    let btn_open = gtk4::Button::with_label("Open Files…");
    btn_open.set_css_classes(&["welcome-button"]);
    {
        let window = window.clone();
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        let label_left = label_left.clone();
        let label_right = label_right.clone();
        let header_left = header_left.clone();
        let header_right = header_right.clone();
        let lbl_added = lbl_added.clone();
        let lbl_removed = lbl_removed.clone();
        let lbl_modified = lbl_modified.clone();
        let lbl_format = lbl_format.clone();
        let lbl_chars = lbl_chars.clone();
        btn_open.connect_clicked(move |_| {
            open_both_dialog(
                &window, &state, &view_l, &view_r,
                &label_left, &label_right, &header_left, &header_right,
                &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
        });
    }
    page.append(&btn_open);

    // Recent list (if any)
    let recents_data = recents.borrow();
    if !recents_data.is_empty() {
        let pairs: Vec<_> = recents_data.list().iter().cloned().collect();
        drop(recents_data);

        let spacer2 = gtk4::Box::builder().height_request(28).build();
        page.append(&spacer2);

        let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep.set_css_classes(&["welcome-separator"]);
        page.append(&sep);

        let spacer3 = gtk4::Box::builder().height_request(12).build();
        page.append(&spacer3);

        let recent_label = gtk4::Label::new(Some("Recent"));
        recent_label.set_css_classes(&["welcome-section-label"]);
        recent_label.set_halign(gtk4::Align::Start);
        page.append(&recent_label);

        let list_box = gtk4::ListBox::new();
        list_box.set_css_classes(&["recent-list"]);
        list_box.set_selection_mode(gtk4::SelectionMode::Single);

        for pair in &pairs {
            let row = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(6)
                .css_classes(vec!["recent-row"])
                .build();

            let l = gtk4::Label::new(Some(&pair.left_name));
            l.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
            l.set_max_width_chars(32);
            l.set_halign(gtk4::Align::Start);
            l.set_hexpand(true);
            row.append(&l);

            let arrow = gtk4::Label::new(Some("↔"));
            arrow.set_css_classes(&["recent-arrow"]);
            row.append(&arrow);

            let r = gtk4::Label::new(Some(&pair.right_name));
            r.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
            r.set_max_width_chars(32);
            r.set_halign(gtk4::Align::End);
            r.set_hexpand(true);
            row.append(&r);

            list_box.append(&row);
        }

        {
            let window = window.clone();
            let state = state.clone();
            let view_l = view_l.clone();
            let view_r = view_r.clone();
            let label_left = label_left.clone();
            let label_right = label_right.clone();
            let header_left = header_left.clone();
            let header_right = header_right.clone();
            let lbl_added = lbl_added.clone();
            let lbl_removed = lbl_removed.clone();
            let lbl_modified = lbl_modified.clone();
            let lbl_format = lbl_format.clone();
            let lbl_chars = lbl_chars.clone();
            list_box.connect_row_activated(move |_, row| {
                let idx = row.index() as usize;
                if let Some(pair) = pairs.get(idx) {
                    load_file(
                        &gio::File::for_path(&pair.left), Side::Left,
                        &window, &state, &view_l, &view_r,
                        &label_left, &label_right, &header_left, &header_right,
                        &lbl_added, &lbl_removed,
                        &lbl_modified, &lbl_format, &lbl_chars,
                    );
                    load_file(
                        &gio::File::for_path(&pair.right), Side::Right,
                        &window, &state, &view_l, &view_r,
                        &label_left, &label_right, &header_left, &header_right,
                        &lbl_added, &lbl_removed,
                        &lbl_modified, &lbl_format, &lbl_chars,
                    );
                }
            });
        }

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&list_box)
            .build();
        scroll.set_max_content_height(300);
        page.append(&scroll);
    } else {
        drop(recents_data);
    }

    page
}

fn make_panel() -> (gtk4::ScrolledWindow, gtk4::TextView) {
    let view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::None)
        .left_margin(0)
        .right_margin(0)
        .top_margin(2)
        .bottom_margin(2)
        .build();

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(&view)
        .build();

    (scroll, view)
}

fn stat_label(kind: &str) -> gtk4::Label {
    gtk4::Label::builder()
        .label("")
        .css_classes(vec![format!("stat-{kind}")])
        .build()
}

fn update_stats_labels(
    stats: &Stats,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    lbl_added.set_label(&format!("+{} added", stats.added));
    lbl_removed.set_label(&format!("-{} removed", stats.removed));
    lbl_modified.set_label(&format!("~{} modified", stats.modified));
    lbl_format.set_label(&format!("⌥{} formatting", stats.format));
    lbl_chars.set_label(&format!(
        "+{} / -{} chars",
        stats.chars_added, stats.chars_removed
    ));
}

fn show_error(window: &gtk4::ApplicationWindow, title: &str, detail: &str) {
    let dlg = gtk4::AlertDialog::builder()
        .message(title)
        .detail(detail)
        .modal(true)
        .build();
    dlg.show(Some(window));
}

// ── File loading ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn load_file(
    file: &gio::File,
    side: Side,
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    let filename = file
        .basename()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_owned());

    let window = window.clone();
    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let header_left = header_left.clone();
    let header_right = header_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    file.load_contents_async(
        None::<&gio::Cancellable>,
        move |result| match result {
            Err(e) => {
                show_error(&window, "Failed to load file", &e.to_string());
            }
            Ok((bytes, _etag)) => {
                // Reject non-UTF-8 files with a clear message rather than
                // silently replacing invalid bytes with '?'.
                let text = match String::from_utf8(bytes.to_vec()) {
                    Ok(s) => s,
                    Err(_) => {
                        show_error(
                            &window,
                            "File is not valid UTF-8",
                            "Only UTF-8 encoded text files are supported.",
                        );
                        return;
                    }
                };
                {
                    let mut s = state.borrow_mut();
                    match side {
                        Side::Left => {
                            s.text_a = Some(text);
                            s.filename_a = filename.clone();
                        }
                        Side::Right => {
                            s.text_b = Some(text);
                            s.filename_b = filename.clone();
                        }
                    }
                    label_left.set_label(&s.filename_a);
                    label_right.set_label(&s.filename_b);
                    header_left.set_label(&format!("− {}", s.filename_a));
                    header_right.set_label(&format!("+ {}", s.filename_b));

                    if let (Some(a), Some(b)) = (&s.text_a, &s.text_b) {
                        let (rows, stats) = process_diff(a, b);
                        s.rows = rows;
                        s.stats = stats;
                        if let Some(ref recents) = s.recents {
                            recents.borrow_mut().add(&s.filename_a, &s.filename_b);
                        }
                    }
                    if let Some(ref stack) = s.stack {
                        stack.set_visible_child_name("diff");
                    }
                }
                // Always update stats — even when rows is empty (identical files
                // or only one file loaded), stale stats must not remain visible.
                let s = state.borrow();
                update_stats_labels(
                    &s.stats, &lbl_added, &lbl_removed, &lbl_modified,
                    &lbl_format, &lbl_chars,
                );
                if !s.rows.is_empty() {
                    populate_views(&view_l, &view_r, &s.rows);
                }
            }
        },
    );
}

// ── File chooser (GTK 4.10+ FileDialog) ──────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn open_file_dialog(
    window: &gtk4::ApplicationWindow,
    side: Side,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    let dialog = gtk4::FileDialog::builder()
        .title(match side {
            Side::Left => "Select Left File",
            Side::Right => "Select Right File",
        })
        .modal(true)
        .build();

    let window_ref = window.clone();
    let window_cb = window.clone();
    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let header_left = header_left.clone();
    let header_right = header_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    dialog.open(
        Some(&window_ref),
        None::<&gio::Cancellable>,
        move |result| {
            if let Ok(file) = result {
                load_file(
                    &file, side, &window_cb, &state, &view_l, &view_r,
                    &label_left, &label_right, &header_left, &header_right,
                    &lbl_added, &lbl_removed,
                    &lbl_modified, &lbl_format, &lbl_chars,
                );
            }
        },
    );
}

// ── "Open Both…" dialog ───────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn open_both_dialog(
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    let dialog = gtk4::FileDialog::builder()
        .title("Select Two Files to Compare")
        .modal(true)
        .build();

    let window_ref = window.clone();
    let window_cb = window.clone();
    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let header_left = header_left.clone();
    let header_right = header_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    dialog.open_multiple(
        Some(&window_ref),
        None::<&gio::Cancellable>,
        move |result| {
            let Ok(model) = result else { return; };
            if let Some(f) = model.item(0).and_then(|o| o.downcast::<gio::File>().ok()) {
                load_file(
                    &f, Side::Left, &window_cb, &state, &view_l, &view_r,
                    &label_left, &label_right, &header_left, &header_right,
                    &lbl_added, &lbl_removed,
                    &lbl_modified, &lbl_format, &lbl_chars,
                );
            }
            if model.n_items() >= 2 {
                if let Some(f) = model.item(1).and_then(|o| o.downcast::<gio::File>().ok()) {
                    load_file(
                        &f, Side::Right, &window_cb, &state, &view_l, &view_r,
                        &label_left, &label_right, &header_left, &header_right,
                        &lbl_added, &lbl_removed,
                        &lbl_modified, &lbl_format, &lbl_chars,
                    );
                }
            }
        },
    );
}

// ── Drag-and-drop ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn setup_drop_target(
    widget: &impl IsA<gtk4::Widget>,
    side: Side,
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    let drop = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    drop.set_preload(true);

    let w = window.clone();
    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let header_left = header_left.clone();
    let header_right = header_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    drop.connect_drop(move |_, value, _, _| {
        let files = extract_files(&value);
        if files.len() >= 2 {
            load_file(&files[0], Side::Left, &w, &state, &view_l, &view_r,
                      &label_left, &label_right, &header_left, &header_right,
                      &lbl_added, &lbl_removed, &lbl_modified, &lbl_format, &lbl_chars);
            load_file(&files[1], Side::Right, &w, &state, &view_l, &view_r,
                      &label_left, &label_right, &header_left, &header_right,
                      &lbl_added, &lbl_removed, &lbl_modified, &lbl_format, &lbl_chars);
        } else if let Some(f) = files.first() {
            load_file(f, side, &w, &state, &view_l, &view_r,
                      &label_left, &label_right, &header_left, &header_right,
                      &lbl_added, &lbl_removed, &lbl_modified, &lbl_format, &lbl_chars);
        }
        !files.is_empty()
    });

    widget.add_controller(drop);
}

// ── Ctrl+V clipboard paste ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn setup_clipboard_paste(
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    let key_ctrl = gtk4::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);

    let window_cb = window.clone();
    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let header_left = header_left.clone();
    let header_right = header_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    key_ctrl.connect_key_pressed(move |_, key, _, modifier| {
        if key == gdk::Key::v && modifier.contains(gdk::ModifierType::CONTROL_MASK) {
            let clipboard = gdk::Display::default()
                .expect("display")
                .clipboard();

            let state = state.clone();
            let window = window_cb.clone();
            let view_l = view_l.clone();
            let view_r = view_r.clone();
            let label_left = label_left.clone();
            let label_right = label_right.clone();
            let header_left = header_left.clone();
            let header_right = header_right.clone();
            let lbl_added = lbl_added.clone();
            let lbl_removed = lbl_removed.clone();
            let lbl_modified = lbl_modified.clone();
            let lbl_format = lbl_format.clone();
            let lbl_chars = lbl_chars.clone();

            clipboard.read_text_async(
                None::<&gio::Cancellable>,
                move |result| {
                    if let Ok(Some(text)) = result {
                        let files = parse_clipboard_text(&text);
                        if !files.is_empty() {
                            load_files(&files, &window, &state, &view_l, &view_r,
                                       &label_left, &label_right, &header_left, &header_right,
                                       &lbl_added, &lbl_removed,
                                       &lbl_modified, &lbl_format, &lbl_chars);
                        }
                    }
                },
            );
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    window.add_controller(key_ctrl);
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Load files from a list: 2+ → left/right split, 1 → smart placement.
fn load_files(
    files: &[gio::File],
    window: &gtk4::ApplicationWindow,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    header_left: &gtk4::Label,
    header_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    if files.len() >= 2 {
        load_file(&files[0], Side::Left, window, state, view_l, view_r,
                  label_left, label_right, header_left, header_right,
                  lbl_added, lbl_removed, lbl_modified, lbl_format, lbl_chars);
        load_file(&files[1], Side::Right, window, state, view_l, view_r,
                  label_left, label_right, header_left, header_right,
                  lbl_added, lbl_removed, lbl_modified, lbl_format, lbl_chars);
    } else if let Some(f) = files.first() {
        let s = state.borrow();
        let side = if s.text_a.is_none() {
            Side::Left
        } else if s.text_b.is_none() {
            Side::Right
        } else {
            Side::Left
        };
        drop(s);
        load_file(f, side, window, state, view_l, view_r,
                  label_left, label_right, header_left, header_right,
                  lbl_added, lbl_removed, lbl_modified, lbl_format, lbl_chars);
    }
}

/// Parse clipboard text: handles file:// URIs, plain paths, and GNOME's
/// `x-special/gnome-copied-files` format.
fn parse_clipboard_text(text: &str) -> Vec<gio::File> {
    let mut files = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // GNOME file manager prefix: "copy\nfile://..." or "cut\nfile://..."
        let uri = trimmed.strip_prefix("copy\n")
            .or_else(|| trimmed.strip_prefix("cut\n"))
            .unwrap_or(trimmed);
        if uri.starts_with("file://") {
            files.push(gio::File::for_uri(uri));
        } else {
            files.push(gio::File::for_path(uri));
        }
    }
    files
}

/// Extract files from a drop value (gdk::FileList on X11, String on Wayland).
fn extract_files(value: &glib::Value) -> Vec<gio::File> {
    if let Ok(list) = value.get::<gdk::FileList>() {
        let files: Vec<_> = list.files().into_iter().collect();
        if !files.is_empty() {
            return files;
        }
    }
    if let Ok(uris) = value.get::<String>() {
        return parse_clipboard_text(&uris);
    }
    Vec::new()
}
