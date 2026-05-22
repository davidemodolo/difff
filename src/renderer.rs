use gtk4::{gdk, prelude::*, TextBuffer, TextView};

use crate::diff::{DiffRow, RowKind};

// ── Color palette (GitHub dark) ───────────────────────────────────────────────

const BG_BASE: &str = "#0d1117";
const BG_ADDED: &str = "#0d4429";
const BG_REMOVED: &str = "#67060c";
const BG_MODIFIED: &str = "#341a00";
const BG_FORMAT: &str = "#1c2128";
const FG_FORMAT: &str = "#484f58";
const FG_LINENUM: &str = "#6e7681";
// Char-level highlight colours (vivid, drawn on top of row background).
const BG_CHAR_DEL: &str = "#b50822";
const BG_CHAR_INS: &str = "#196727";
const FG_CHAR_HL: &str = "#ffffff";

// ── Line prefix ───────────────────────────────────────────────────────────────

/// Width of the gutter column in characters: `"  NNN │ "` = 5 + 3 = 8.
pub const PREFIX_CHARS: usize = 8;

fn format_gutter(line_num: Option<usize>) -> String {
    match line_num {
        Some(n) => format!("{:>5} │ ", n),
        None => "      │ ".to_owned(),
    }
}

// ── Tag names ─────────────────────────────────────────────────────────────────

const TAG_ADDED: &str = "difff-added";
const TAG_REMOVED: &str = "difff-removed";
const TAG_MODIFIED: &str = "difff-modified";
const TAG_FORMAT: &str = "difff-format";
const TAG_LINENUM: &str = "difff-linenum";
const TAG_CHAR_DEL: &str = "difff-char-del";
const TAG_CHAR_INS: &str = "difff-char-ins";

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum Side {
    Left,
    Right,
}

/// Rebuild both views from scratch.  Call any time the diff changes or the
/// `ignore_format` flag is toggled.
pub fn populate_views(
    view_left: &TextView,
    view_right: &TextView,
    rows: &[DiffRow],
    ignore_format: bool,
) {
    // Build new buffers on a shared tag table so tags are defined once.
    let table = make_tag_table();
    let buf_l = TextBuffer::new(Some(&table));
    let buf_r = TextBuffer::new(Some(&table));

    fill_buffer(&buf_l, rows, Side::Left, ignore_format);
    fill_buffer(&buf_r, rows, Side::Right, ignore_format);

    view_left.set_buffer(Some(&buf_l));
    view_right.set_buffer(Some(&buf_r));
}

// ── Tag table ─────────────────────────────────────────────────────────────────

fn make_tag_table() -> gtk4::TextTagTable {
    let table = gtk4::TextTagTable::new();

    // Helper: create a tag, configure it, add to table.
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
    });
    add(TAG_CHAR_DEL, &|t| {
        t.set_background_rgba(Some(&rgba(BG_CHAR_DEL)));
        t.set_foreground_rgba(Some(&rgba(FG_CHAR_HL)));
    });
    add(TAG_CHAR_INS, &|t| {
        t.set_background_rgba(Some(&rgba(BG_CHAR_INS)));
        t.set_foreground_rgba(Some(&rgba(FG_CHAR_HL)));
    });

    table
}

// ── Buffer filling ────────────────────────────────────────────────────────────

fn fill_buffer(buf: &TextBuffer, rows: &[DiffRow], side: Side, ignore_format: bool) {
    // Suppress undo-history tracking while doing bulk inserts.
    buf.begin_irreversible_action();

    buf.set_text("");

    for (line_idx, row) in rows.iter().enumerate() {
        let is_last = line_idx + 1 == rows.len();

        let (num, content) = match side {
            Side::Left => (row.left_num, row.left_text.as_deref()),
            Side::Right => (row.right_num, row.right_text.as_deref()),
        };

        // Format rows become blank when ignored (preserves line alignment).
        let display_text = if row.kind == RowKind::Format && ignore_format {
            None
        } else {
            content
        };

        let line = format!("{}{}", format_gutter(num), display_text.unwrap_or(""));
        if is_last {
            buf.insert_at_cursor(&line);
        } else {
            buf.insert_at_cursor(&format!("{line}\n"));
        }

        // ── Apply tags for this line ───────────────────────────────────────

        let row_tag = row_tag_name(row, side, ignore_format);
        if let Some(tag_name) = row_tag {
            let start = buf.iter_at_line(line_idx as i32).unwrap_or_else(|| buf.end_iter());
            let end = if is_last {
                buf.end_iter()
            } else {
                buf.iter_at_line((line_idx + 1) as i32).unwrap_or_else(|| buf.end_iter())
            };
            buf.apply_tag_by_name(tag_name, &start, &end);
        }

        // Gutter / line number styling.
        if let Some(gutter_start) = buf.iter_at_line(line_idx as i32) {
            let mut gutter_end = gutter_start;
            gutter_end.forward_chars(PREFIX_CHARS as i32);
            buf.apply_tag_by_name(TAG_LINENUM, &gutter_start, &gutter_end);
        }

        // Char-level highlights (Modified rows only).
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
                let buf_start = PREFIX_CHARS + start_ch;
                let buf_end = PREFIX_CHARS + end_ch;
                if let (Some(mut iter_s), Some(mut iter_e)) = (
                    buf.iter_at_line(line_idx as i32),
                    buf.iter_at_line(line_idx as i32),
                ) {
                    iter_s.forward_chars(buf_start as i32);
                    iter_e.forward_chars(buf_end as i32);
                    buf.apply_tag_by_name(char_tag, &iter_s, &iter_e);
                }
            }
        }
    }

    buf.end_irreversible_action();
}

fn row_tag_name(row: &DiffRow, side: Side, ignore_format: bool) -> Option<&'static str> {
    match (&row.kind, side) {
        (RowKind::Added, Side::Right) => Some(TAG_ADDED),
        (RowKind::Added, Side::Left) => None,
        (RowKind::Removed, Side::Left) => Some(TAG_REMOVED),
        (RowKind::Removed, Side::Right) => None,
        (RowKind::Modified, _) => Some(TAG_MODIFIED),
        (RowKind::Format, _) if !ignore_format => Some(TAG_FORMAT),
        (RowKind::Format, _) => None,
        _ => None,
    }
}

// ── CSS for the text view itself ──────────────────────────────────────────────

pub fn global_css() -> String {
    format!(
        r#"
        window, .main-window {{
            background-color: {BG_BASE};
        }}
        textview {{
            background-color: {BG_BASE};
            color: #e6edf3;
            font-family: "Cascadia Code", "Fira Code", "Jetbrains Mono", monospace;
            font-size: 13px;
        }}
        textview text {{
            background-color: {BG_BASE};
        }}
        .toolbar {{
            background-color: #161b22;
            border-bottom: 1px solid #30363d;
            padding: 6px 10px;
        }}
        .stats-bar {{
            background-color: #161b22;
            border-bottom: 1px solid #21262d;
            padding: 3px 12px;
        }}
        button {{
            background: #21262d;
            color: #e6edf3;
            border: 1px solid #30363d;
            border-radius: 6px;
            padding: 4px 10px;
        }}
        button:hover {{
            background: #30363d;
            border-color: #8b949e;
        }}
        button:active {{
            background: #161b22;
        }}
        checkbutton {{
            color: #e6edf3;
        }}
        checkbutton check {{
            background: #21262d;
            border: 1px solid #30363d;
            border-radius: 3px;
        }}
        checkbutton:checked check {{
            background: #1f6feb;
            border-color: #1f6feb;
        }}
        label {{
            color: #8b949e;
        }}
        .filename-label {{
            color: #e6edf3;
            font-weight: bold;
        }}
        .stat-added   {{ color: #3fb950; }}
        .stat-removed {{ color: #f85149; }}
        .stat-modified {{ color: #d29922; }}
        .stat-format  {{ color: #8b949e; }}
        scrollbar {{
            background-color: #161b22;
        }}
        scrollbar slider {{
            background-color: #30363d;
            border-radius: 4px;
            min-width: 6px;
            min-height: 6px;
        }}
        scrollbar slider:hover {{
            background-color: #8b949e;
        }}
        separator {{
            background-color: #30363d;
        }}
        "#
    )
}
