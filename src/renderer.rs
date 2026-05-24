use gtk4::{gdk, prelude::*, TextBuffer, TextView};

use crate::diff::{DiffRow, RowKind};

// ── Color palette ──────────────────────────────────────────────────────────────

const BG_BASE: &str = "#0a0a0e";
const BG_SURFACE: &str = "#121218";
const BG_ADDED: &str = "#0d1a14";
const BG_REMOVED: &str = "#1a0f14";
const BG_MODIFIED: &str = "#1a140f";
const BG_FORMAT: &str = "#0a0a0e";
const FG_FORMAT: &str = "#555560";
const FG_LINENUM: &str = "#555565";
const FG_TEXT: &str = "#d4d4dc";
const FG_ADDED_GUTTER: &str = "#00cc88";
const FG_REMOVED_GUTTER: &str = "#ee4466";
#[allow(dead_code)]
const FG_MODIFIED_GUTTER: &str = "#ee9900";
const BG_CHAR_DEL: &str = "#cc3344";
const BG_CHAR_INS: &str = "#00aa55";
const FG_CHAR_HL: &str = "#ffffff";
const FG_LINE_DEL: &str = "#ff6677";
const FG_LINE_INS: &str = "#00dd88";

const BORDER: &str = "#252530";

// ── Line prefix ───────────────────────────────────────────────────────────────

pub const PREFIX_CHARS: usize = 10;

fn format_gutter(line_num: Option<usize>, marker: char) -> String {
    match line_num {
        Some(n) => format!(" {marker} {:>5} │ ", n),
        None =>    format!(" {marker} {:>5} │ ", ""),
    }
}

// ── Tag names ─────────────────────────────────────────────────────────────────

const TAG_ADDED: &str = "difff-added";
const TAG_REMOVED: &str = "difff-removed";
const TAG_MODIFIED: &str = "difff-modified";
const TAG_FORMAT: &str = "difff-format";
const TAG_LINENUM: &str = "difff-linenum";
const TAG_MARKER: &str = "difff-marker";
const TAG_CHAR_DEL: &str = "difff-char-del";
const TAG_CHAR_INS: &str = "difff-char-ins";
const TAG_LINE_DEL: &str = "difff-line-del";
const TAG_LINE_INS: &str = "difff-line-ins";

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

pub fn populate_views(
    view_left: &TextView,
    view_right: &TextView,
    rows: &[DiffRow],
) {
    let table = make_tag_table();
    let buf_l = TextBuffer::new(Some(&table));
    let buf_r = TextBuffer::new(Some(&table));

    fill_buffer(&buf_l, rows, Side::Left);
    fill_buffer(&buf_r, rows, Side::Right);

    view_left.set_buffer(Some(&buf_l));
    view_right.set_buffer(Some(&buf_r));
}

// ── Tag table ─────────────────────────────────────────────────────────────────

fn make_tag_table() -> gtk4::TextTagTable {
    let table = gtk4::TextTagTable::new();
    let add = |name: &str, f: &dyn Fn(&gtk4::TextTag)| {
        let tag = gtk4::TextTag::new(Some(name));
        f(&tag);
        table.add(&tag);
    };
    let rgba = |s: &str| -> gdk::RGBA { gdk::RGBA::parse(s).expect("valid colour") };

    add(TAG_ADDED, &|t| {
        t.set_paragraph_background_rgba(Some(&rgba(BG_ADDED)));
    });
    add(TAG_REMOVED, &|t| {
        t.set_paragraph_background_rgba(Some(&rgba(BG_REMOVED)));
    });
    add(TAG_MODIFIED, &|t| {
        t.set_paragraph_background_rgba(Some(&rgba(BG_MODIFIED)));
    });
    add(TAG_FORMAT, &|t| {
        t.set_paragraph_background_rgba(Some(&rgba(BG_FORMAT)));
        t.set_foreground_rgba(Some(&rgba(FG_FORMAT)));
    });
    add(TAG_LINENUM, &|t| {
        t.set_foreground_rgba(Some(&rgba(FG_LINENUM)));
        t.set_size_points(10.0);
    });
    add(TAG_MARKER, &|t| {
        t.set_weight(700);
    });
    add(TAG_CHAR_DEL, &|t| {
        t.set_background_rgba(Some(&rgba(BG_CHAR_DEL)));
        t.set_foreground_rgba(Some(&rgba(FG_CHAR_HL)));
    });
    add(TAG_CHAR_INS, &|t| {
        t.set_background_rgba(Some(&rgba(BG_CHAR_INS)));
        t.set_foreground_rgba(Some(&rgba(FG_CHAR_HL)));
    });
    add(TAG_LINE_DEL, &|t| {
        t.set_foreground_rgba(Some(&rgba(FG_LINE_DEL)));
    });
    add(TAG_LINE_INS, &|t| {
        t.set_foreground_rgba(Some(&rgba(FG_LINE_INS)));
    });

    table
}

// ── Buffer filling ────────────────────────────────────────────────────────────

fn fill_buffer(buf: &TextBuffer, rows: &[DiffRow], side: Side) {
    buf.begin_irreversible_action();
    buf.set_text("");

    for (line_idx, row) in rows.iter().enumerate() {
        let is_last = line_idx + 1 == rows.len();

        let (num, content) = match side {
            Side::Left => (row.left_num, row.left_text.as_deref()),
            Side::Right => (row.right_num, row.right_text.as_deref()),
        };

        let display_text = content;

        let marker = gutter_marker(row, side);
        let line = format!(
            "{}{}",
            format_gutter(num, marker),
            display_text.unwrap_or("")
        );
        if is_last {
            buf.insert_at_cursor(&line);
        } else {
            buf.insert_at_cursor(&format!("{line}\n"));
        }

        let row_tag = row_tag_name(row, side);
        if let Some(tag_name) = row_tag {
            let start = buf.iter_at_line(line_idx as i32).unwrap_or(buf.end_iter());
            let end = if is_last {
                buf.end_iter()
            } else {
                buf.iter_at_line((line_idx + 1) as i32).unwrap_or(buf.end_iter())
            };
            buf.apply_tag_by_name(tag_name, &start, &end);
        }

        if let Some(gutter_start) = buf.iter_at_line(line_idx as i32) {
            let mut marker_start = gutter_start;
            marker_start.forward_chars(1);
            let mut marker_end = marker_start;
            marker_end.forward_chars(1);
            buf.apply_tag_by_name(TAG_MARKER, &marker_start, &marker_end);

            match row.kind {
                RowKind::Added if side == Side::Right => {
                    buf.apply_tag_by_name(TAG_MARKER, &marker_start, &marker_end);
                }
                RowKind::Removed if side == Side::Left => {
                    buf.apply_tag_by_name(TAG_MARKER, &marker_start, &marker_end);
                }
                _ => {}
            }
        }

        if let Some(gutter_start) = buf.iter_at_line(line_idx as i32) {
            let mut gutter_end = gutter_start;
            gutter_end.forward_chars(PREFIX_CHARS as i32);
            let mut gutter_begin = gutter_start;
            gutter_begin.forward_chars(2);
            buf.apply_tag_by_name(TAG_LINENUM, &gutter_begin, &gutter_end);
        }

        if (row.kind == RowKind::Added && side == Side::Right)
            || (row.kind == RowKind::Removed && side == Side::Left)
        {
            if let Some(line_start) = buf.iter_at_line(line_idx as i32) {
                let mut content_start = line_start;
                content_start.forward_chars(PREFIX_CHARS as i32);
                let line_tag = if row.kind == RowKind::Added {
                    TAG_LINE_INS
                } else {
                    TAG_LINE_DEL
                };
                let end = if is_last {
                    buf.end_iter()
                } else {
                    let mut e = line_start;
                    e.forward_to_line_end();
                    e
                };
                buf.apply_tag_by_name(line_tag, &content_start, &end);
            }
        }

        if row.kind == RowKind::Modified && display_text.is_some() {
            let highlights = match side {
                Side::Left => &row.left_highlights,
                Side::Right => &row.right_highlights,
            };
            let char_tag = match side {
                Side::Left => TAG_CHAR_DEL,
                Side::Right => TAG_CHAR_INS,
            };
            for &(start_ch, end_ch) in highlights {
                if let (Some(mut iter_s), Some(mut iter_e)) = (
                    buf.iter_at_line(line_idx as i32),
                    buf.iter_at_line(line_idx as i32),
                ) {
                    iter_s.forward_chars((PREFIX_CHARS + start_ch) as i32);
                    iter_e.forward_chars((PREFIX_CHARS + end_ch) as i32);
                    buf.apply_tag_by_name(char_tag, &iter_s, &iter_e);
                }
            }
        }
    }

    buf.end_irreversible_action();
}

fn gutter_marker(row: &DiffRow, side: Side) -> char {
    match (&row.kind, side) {
        (RowKind::Added, Side::Right) => '+',
        (RowKind::Removed, Side::Left) => '-',
        (RowKind::Modified, _) => '~',
        _ => ' ',
    }
}

fn row_tag_name(row: &DiffRow, side: Side) -> Option<&'static str> {
    match (&row.kind, side) {
        (RowKind::Added, Side::Right) => Some(TAG_ADDED),
        (RowKind::Added, Side::Left) => None,
        (RowKind::Removed, Side::Left) => Some(TAG_REMOVED),
        (RowKind::Removed, Side::Right) => None,
        (RowKind::Modified, _) => Some(TAG_MODIFIED),
        (RowKind::Format, _) => Some(TAG_FORMAT),
        _ => None,
    }
}

// ── CSS ───────────────────────────────────────────────────────────────────────

pub fn global_css() -> String {
    let rgba = |s: &str| -> String {
        let c = gdk::RGBA::parse(s).expect("valid colour");
        format!(
            "rgb({},{},{})",
            (c.red() * 255.0) as u8,
            (c.green() * 255.0) as u8,
            (c.blue() * 255.0) as u8
        )
    };

    format!(
        r#"
        window, .main-window {{
            background-color: {BG_BASE};
        }}

        textview {{
            background-color: {BG_BASE};
            color: {FG_TEXT};
            font-family: "JetBrains Mono", "Fira Code", "IBM Plex Mono", monospace;
            font-size: 11pt;
            line-height: 1.35;
        }}
        textview text {{
            background-color: transparent;
        }}

        .toolbar {{
            background-color: {r_surface};
            border-bottom: 1px solid {r_border};
            padding: 6px 10px;
            min-height: 32px;
        }}

        .stats-bar {{
            background-color: {BG_BASE};
            border-bottom: 1px solid {r_border};
            padding: 2px 10px;
            min-height: 22px;
        }}

        .panel-header-left {{
            background-color: {r_surface};
            color: {r_text};
            padding: 2px 10px;
            font-weight: 600;
            font-size: 9.5pt;
            font-family: "JetBrains Mono", "Fira Code", monospace;
            border-bottom: 2px solid {FG_REMOVED_GUTTER};
        }}

        .panel-header-right {{
            background-color: {r_surface};
            color: {r_text};
            padding: 2px 10px;
            font-weight: 600;
            font-size: 9.5pt;
            font-family: "JetBrains Mono", "Fira Code", monospace;
            border-bottom: 2px solid {FG_ADDED_GUTTER};
        }}
        "#,
        r_surface = rgba(BG_SURFACE),
        r_border = rgba(BORDER),
        r_text = rgba(FG_TEXT),
    ) + r#"

        button {
            background: #1a1a22;
            color: #d0d0d8;
            border: 1px solid #2a2a35;
            border-radius: 0;
            padding: 3px 10px;
            font-size: 10pt;
            font-family: "JetBrains Mono", "Fira Code", monospace;
            min-height: 24px;
            letter-spacing: 0.5px;
        }
        button:hover {
            background: #252530;
            border-color: #555565;
        }
        button:active {
            background: #15151d;
            border-color: #00cc88;
        }

        checkbutton {
            color: #aaaaaa;
            font-size: 9.5pt;
            font-family: "JetBrains Mono", "Fira Code", monospace;
        }
        checkbutton check {
            background: #1a1a22;
            border: 1px solid #2a2a35;
            border-radius: 0;
            min-width: 14px;
            min-height: 14px;
        }
        checkbutton:checked check {
            background: #00cc88;
            border-color: #00cc88;
        }

        label {
            color: #909098;
        }
        .filename-label {
            color: #d4d4dc;
            font-weight: 600;
            font-size: 9.5pt;
            font-family: "JetBrains Mono", "Fira Code", monospace;
        }

        .stat-added    { color: #00cc88; font-weight: 700; font-size: 9pt; font-family: "JetBrains Mono", monospace; }
        .stat-removed  { color: #ee4466; font-weight: 700; font-size: 9pt; font-family: "JetBrains Mono", monospace; }
        .stat-modified { color: #ee9900; font-weight: 700; font-size: 9pt; font-family: "JetBrains Mono", monospace; }
        .stat-format   { color: #6a6a70; font-weight: 500; font-size: 9pt; font-family: "JetBrains Mono", monospace; }
        .stat-chars    { color: #888890; font-size: 9pt; font-family: "JetBrains Mono", monospace; }

        scrollbar {
            background-color: #0a0a0e;
            border: none;
        }
        scrollbar slider {
            background-color: #2a2a35;
            border-radius: 0;
            min-width: 6px;
            min-height: 6px;
        }
        scrollbar slider:hover {
            background-color: #3a3a48;
        }

        separator {
            background-color: #252530;
            min-width: 1px;
        }
    "#
}
