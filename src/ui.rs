use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gdk, gio, prelude::*};

use crate::diff::{process_diff, DiffRow, Stats};
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
    ignore_format: bool,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub fn build_ui(app: &gtk4::Application) {
    let state: Rc<RefCell<AppState>> = Rc::new(RefCell::new(AppState::default()));

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
    window.set_child(Some(&root));

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

    let spacer_left = gtk4::Box::builder()
        .hexpand(true)
        .build();
    let spacer_right = gtk4::Box::builder()
        .hexpand(true)
        .build();

    let format_check = gtk4::CheckButton::builder()
        .label("Ignore formatting")
        .active(false)
        .build();

    toolbar.append(&btn_left);
    toolbar.append(&label_left);
    toolbar.append(&spacer_left);
    toolbar.append(&format_check);
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

    let (scroll_l, view_l) = make_panel();
    let (scroll_r, view_r) = make_panel();
    paned.set_start_child(Some(&scroll_l));
    paned.set_end_child(Some(&scroll_r));

    // Set initial pane position after the window has been allocated.
    let paned_c = paned.clone();
    window.connect_map(move |w| {
        let w_width = w.width();
        if w_width > 0 {
            paned_c.set_position(w_width / 2);
        }
    });

    // ── Scroll synchronisation ───────────────────────────────────────────────
    let lock: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    let adj_l = scroll_l.vadjustment();
    let adj_r = scroll_r.vadjustment();

    {
        let adj_r = adj_r.clone();
        let lock = lock.clone();
        adj_l.connect_value_changed(move |adj| {
            if *lock.borrow() {
                return;
            }
            *lock.borrow_mut() = true;
            adj_r.set_value(adj.value());
            *lock.borrow_mut() = false;
        });
    }
    {
        let adj_l = adj_l.clone();
        let lock = lock.clone();
        adj_r.connect_value_changed(move |adj| {
            if *lock.borrow() {
                return;
            }
            *lock.borrow_mut() = true;
            adj_l.set_value(adj.value());
            *lock.borrow_mut() = false;
        });
    }

    // ── Drag-and-drop ────────────────────────────────────────────────────────
    setup_drop_target(&scroll_l, Side::Left, &state, &view_l, &view_r,
                      &label_left, &label_right, &lbl_added, &lbl_removed,
                      &lbl_modified, &lbl_format, &lbl_chars);
    setup_drop_target(&scroll_r, Side::Right, &state, &view_l, &view_r,
                      &label_left, &label_right, &lbl_added, &lbl_removed,
                      &lbl_modified, &lbl_format, &lbl_chars);

    // ── File chooser buttons ─────────────────────────────────────────────────
    {
        let window = window.clone();
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        let label_left = label_left.clone();
        let label_right = label_right.clone();
        let lbl_added = lbl_added.clone();
        let lbl_removed = lbl_removed.clone();
        let lbl_modified = lbl_modified.clone();
        let lbl_format = lbl_format.clone();
        let lbl_chars = lbl_chars.clone();
        btn_left.connect_clicked(move |_| {
            open_file_dialog(
                &window, Side::Left, &state, &view_l, &view_r,
                &label_left, &label_right, &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
        });
    }
    {
        let window = window.clone();
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        let label_left = label_left.clone();
        let label_right = label_right.clone();
        let lbl_added = lbl_added.clone();
        let lbl_removed = lbl_removed.clone();
        let lbl_modified = lbl_modified.clone();
        let lbl_format = lbl_format.clone();
        let lbl_chars = lbl_chars.clone();
        btn_right.connect_clicked(move |_| {
            open_file_dialog(
                &window, Side::Right, &state, &view_l, &view_r,
                &label_left, &label_right, &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
        });
    }

    // ── Formatting toggle ────────────────────────────────────────────────────
    {
        let state = state.clone();
        let view_l = view_l.clone();
        let view_r = view_r.clone();
        format_check.connect_toggled(move |btn| {
            let mut s = state.borrow_mut();
            s.ignore_format = btn.is_active();
            if !s.rows.is_empty() {
                drop(s);
                let s = state.borrow();
                populate_views(&view_l, &view_r, &s.rows, s.ignore_format);
            }
        });
    }

    window.present();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_panel() -> (gtk4::ScrolledWindow, gtk4::TextView) {
    let view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::None)
        .left_margin(4)
        .right_margin(4)
        .top_margin(4)
        .bottom_margin(4)
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

// ── File loading ──────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn load_file(
    file: &gio::File,
    side: Side,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
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

    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    file.load_contents_async(
        None::<&gio::Cancellable>,
        move |result| match result {
            Err(e) => {
                eprintln!("difff: failed to load file: {e}");
            }
            Ok((bytes, _etag)) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
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

                    if let (Some(a), Some(b)) = (&s.text_a, &s.text_b) {
                        let (rows, stats) = process_diff(a, b);
                        s.rows = rows;
                        s.stats = stats;
                    }
                }
                let s = state.borrow();
                if !s.rows.is_empty() {
                    populate_views(&view_l, &view_r, &s.rows, s.ignore_format);
                    update_stats_labels(
                        &s.stats, &lbl_added, &lbl_removed, &lbl_modified,
                        &lbl_format, &lbl_chars,
                    );
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

    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    dialog.open(
        Some(window),
        None::<&gio::Cancellable>,
        move |result| {
            if let Ok(file) = result {
                load_file(
                    &file, side, &state, &view_l, &view_r,
                    &label_left, &label_right, &lbl_added, &lbl_removed,
                    &lbl_modified, &lbl_format, &lbl_chars,
                );
            }
        },
    );
}

// ── Drag-and-drop ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn setup_drop_target(
    widget: &gtk4::ScrolledWindow,
    side: Side,
    state: &Rc<RefCell<AppState>>,
    view_l: &gtk4::TextView,
    view_r: &gtk4::TextView,
    label_left: &gtk4::Label,
    label_right: &gtk4::Label,
    lbl_added: &gtk4::Label,
    lbl_removed: &gtk4::Label,
    lbl_modified: &gtk4::Label,
    lbl_format: &gtk4::Label,
    lbl_chars: &gtk4::Label,
) {
    // Accept single File or a FileList (from Nautilus / Nemo).
    let drop = gtk4::DropTarget::builder()
        .actions(gdk::DragAction::COPY)
        .build();
    drop.set_types(&[gio::File::static_type(), gdk::FileList::static_type()]);

    let state = state.clone();
    let view_l = view_l.clone();
    let view_r = view_r.clone();
    let label_left = label_left.clone();
    let label_right = label_right.clone();
    let lbl_added = lbl_added.clone();
    let lbl_removed = lbl_removed.clone();
    let lbl_modified = lbl_modified.clone();
    let lbl_format = lbl_format.clone();
    let lbl_chars = lbl_chars.clone();

    drop.connect_drop(move |_, value, _, _| {
        let file: Option<gio::File> = if let Ok(f) = value.get::<gio::File>() {
            Some(f)
        } else if let Ok(list) = value.get::<gdk::FileList>() {
            list.files().into_iter().next()
        } else {
            None
        };

        if let Some(f) = file {
            load_file(
                &f, side, &state, &view_l, &view_r,
                &label_left, &label_right, &lbl_added, &lbl_removed,
                &lbl_modified, &lbl_format, &lbl_chars,
            );
            true
        } else {
            false
        }
    });

    widget.add_controller(drop);
}
