use similar::{ChangeTag, TextDiff};

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Equal,
    Added,
    Removed,
    Modified,
    /// Lines that differ only in whitespace inside brackets — not a real change.
    Format,
}

#[derive(Debug, Clone)]
pub struct DiffRow {
    pub kind: RowKind,
    pub left_num: Option<usize>,
    pub right_num: Option<usize>,
    /// Line content without trailing newline.  None → placeholder (alignment).
    pub left_text: Option<String>,
    pub right_text: Option<String>,
    /// Character-offset ranges within the content string that are highlighted.
    /// Only populated for `Modified` rows.
    pub left_highlights: Vec<(usize, usize)>,
    pub right_highlights: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub format: usize,
    pub chars_added: usize,
    pub chars_removed: usize,
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn process_diff(text_a: &str, text_b: &str) -> (Vec<DiffRow>, Stats) {
    let mut rows = build_rows(text_a, text_b);
    apply_semantic_normalization(&mut rows);
    compute_char_diffs(&mut rows);
    let stats = calc_stats(&rows);
    (rows, stats)
}

// ── Line-level diff ───────────────────────────────────────────────────────────

fn build_rows(text_a: &str, text_b: &str) -> Vec<DiffRow> {
    let diff = TextDiff::from_lines(text_a, text_b);

    let mut rows: Vec<DiffRow> = Vec::new();
    let mut left_num: usize = 1;
    let mut right_num: usize = 1;

    // Buffers for pairing consecutive deletions with insertions.
    let mut del_buf: Vec<String> = Vec::new();
    let mut ins_buf: Vec<String> = Vec::new();

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches('\n').to_owned();
        match change.tag() {
            ChangeTag::Equal => {
                flush(&mut del_buf, &mut ins_buf, &mut rows, &mut left_num, &mut right_num);
                rows.push(DiffRow {
                    kind: RowKind::Equal,
                    left_num: Some(left_num),
                    right_num: Some(right_num),
                    left_text: Some(text.clone()),
                    right_text: Some(text),
                    left_highlights: vec![],
                    right_highlights: vec![],
                });
                left_num += 1;
                right_num += 1;
            }
            ChangeTag::Delete => del_buf.push(text),
            ChangeTag::Insert => ins_buf.push(text),
        }
    }
    flush(&mut del_buf, &mut ins_buf, &mut rows, &mut left_num, &mut right_num);

    rows
}

/// Pair del/ins buffers into Modified rows, emit stragglers as Removed/Added.
fn flush(
    del_buf: &mut Vec<String>,
    ins_buf: &mut Vec<String>,
    rows: &mut Vec<DiffRow>,
    left_num: &mut usize,
    right_num: &mut usize,
) {
    let paired = del_buf.len().min(ins_buf.len());

    for i in 0..paired {
        rows.push(DiffRow {
            kind: RowKind::Modified,
            left_num: Some(*left_num),
            right_num: Some(*right_num),
            left_text: Some(del_buf[i].clone()),
            right_text: Some(ins_buf[i].clone()),
            left_highlights: vec![],
            right_highlights: vec![],
        });
        *left_num += 1;
        *right_num += 1;
    }

    for text in del_buf[paired..].iter().cloned() {
        rows.push(DiffRow {
            kind: RowKind::Removed,
            left_num: Some(*left_num),
            right_num: None,
            left_text: Some(text),
            right_text: None,
            left_highlights: vec![],
            right_highlights: vec![],
        });
        *left_num += 1;
    }

    for text in ins_buf[paired..].iter().cloned() {
        rows.push(DiffRow {
            kind: RowKind::Added,
            left_num: None,
            right_num: Some(*right_num),
            left_text: None,
            right_text: Some(text),
            left_highlights: vec![],
            right_highlights: vec![],
        });
        *right_num += 1;
    }

    del_buf.clear();
    ins_buf.clear();
}

// ── Semantic normalization ────────────────────────────────────────────────────

/// Collapse whitespace (including newlines) inside balanced brackets to a
/// single space, leave everything else untouched.  Mismatched brackets are
/// handled gracefully via saturating arithmetic.
#[allow(dead_code)]
pub fn semantic_normalize(text: &str) -> String {
    semantic_normalize_with_map(text).0
}

/// Like `semantic_normalize` but also returns a vector mapping each output
/// character position back to its source byte offset in the input string.
fn semantic_normalize_with_map(text: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(text.len());
    let mut map = Vec::with_capacity(text.len());
    let mut depth: u32 = 0;
    let mut chars = text.char_indices().peekable();

    while let Some((src, ch)) = chars.next() {
        match ch {
            '(' | '[' | '{' => {
                depth += 1;
                out.push(ch);
                map.push(src);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                if out.ends_with(' ') {
                    out.pop();
                    map.pop();
                }
                out.push(ch);
                map.push(src);
            }
            c if c.is_whitespace() => {
                while chars.peek().is_some_and(|(_, p)| p.is_whitespace()) {
                    chars.next();
                }
                let after_opener = out
                    .chars()
                    .last()
                    .is_some_and(|p| matches!(p, '(' | '[' | '{'));
                let before_closer = chars
                    .peek()
                    .is_some_and(|(_, p)| matches!(p, ')' | ']' | '}'));
                if !after_opener && !before_closer {
                    out.push(' ');
                    map.push(src);
                }
            }
            '"' if depth > 0 => {
                out.push('\'');
                map.push(src);
            }
            c => {
                out.push(c);
                map.push(src);
            }
        }
    }
    (out, map)
}

/// Build a concatenated string from one side (left or right) of a hunk, plus a
/// byte-index→row-index lookup table for mapping normalized positions back to
/// rows.  `char_to_row[i]` is `Some(row_idx)` if the byte at position `i`
/// belongs to that row, or `None` for the `\n` separators between rows.
fn build_side(hunk: &[DiffRow], left_side: bool) -> (String, Vec<Option<usize>>) {
    let mut text = String::new();
    let mut char_to_row: Vec<Option<usize>> = Vec::new();

    for (row_idx, row) in hunk.iter().enumerate() {
        let opt = if left_side {
            row.left_text.as_deref()
        } else {
            row.right_text.as_deref()
        };
        if let Some(content) = opt {
            if !text.is_empty() {
                text.push('\n');
                char_to_row.push(None);
            }
            let start = text.len();
            text.push_str(content);
            for _ in start..text.len() {
                char_to_row.push(Some(row_idx));
            }
        }
    }

    (text, char_to_row)
}

fn apply_semantic_normalization(rows: &mut [DiffRow]) {
    let n = rows.len();
    let mut i = 0;

    while i < n {
        if rows[i].kind == RowKind::Equal {
            i += 1;
            continue;
        }

        let hunk_start = i;
        while i < n && rows[i].kind != RowKind::Equal {
            i += 1;
        }
        let hunk_end = i;
        let hunk = &rows[hunk_start..hunk_end];

        let (left_concat, left_char_to_row) = build_side(hunk, true);
        let (right_concat, right_char_to_row) = build_side(hunk, false);

        if left_concat.is_empty() || right_concat.is_empty() {
            continue;
        }

        let (left_norm, left_map) = semantic_normalize_with_map(&left_concat);
        let (right_norm, right_map) = semantic_normalize_with_map(&right_concat);

        if left_norm == right_norm {
            for row in &mut rows[hunk_start..hunk_end] {
                if row.kind != RowKind::Equal {
                    row.kind = RowKind::Format;
                }
            }
        } else {
            let diff = TextDiff::from_chars(&left_norm, &right_norm);
            let hunk_len = hunk_end - hunk_start;
            let mut row_clean = vec![true; hunk_len];
            let mut lpos: usize = 0;
            let mut rpos: usize = 0;

            for change in diff.iter_all_changes() {
                let n_chars = change.value().chars().count();
                match change.tag() {
                    ChangeTag::Delete => {
                        for j in lpos..lpos + n_chars {
                            if let Some(&src) = left_map.get(j) {
                                if let Some(Some(ri)) = left_char_to_row.get(src) {
                                    row_clean[*ri] = false;
                                }
                            }
                        }
                        lpos += n_chars;
                    }
                    ChangeTag::Insert => {
                        for j in rpos..rpos + n_chars {
                            if let Some(&src) = right_map.get(j) {
                                if let Some(Some(ri)) = right_char_to_row.get(src) {
                                    row_clean[*ri] = false;
                                }
                            }
                        }
                        rpos += n_chars;
                    }
                    ChangeTag::Equal => {
                        lpos += n_chars;
                        rpos += n_chars;
                    }
                }
            }

            for (j, clean) in row_clean.iter().enumerate() {
                if *clean && rows[hunk_start + j].kind != RowKind::Equal {
                    rows[hunk_start + j].kind = RowKind::Format;
                }
            }
        }
    }
}

// ── Character-level diff ──────────────────────────────────────────────────────

fn compute_char_diffs(rows: &mut [DiffRow]) {
    for row in rows.iter_mut() {
        if row.kind != RowKind::Modified {
            continue;
        }
        let left = row.left_text.as_deref().unwrap_or("");
        let right = row.right_text.as_deref().unwrap_or("");
        let (lh, rh) = char_diff_ranges(left, right);
        row.left_highlights = lh;
        row.right_highlights = rh;
    }
}

/// Returns `(left_ranges, right_ranges)` where each range is `(start, end)` in
/// Unicode character offsets (not byte offsets) from the beginning of the string.
type CharRanges = Vec<(usize, usize)>;

fn char_diff_ranges(a: &str, b: &str) -> (CharRanges, CharRanges) {
    let diff = TextDiff::from_chars(a, b);
    let mut left_hl: Vec<(usize, usize)> = Vec::new();
    let mut right_hl: Vec<(usize, usize)> = Vec::new();
    let mut lpos: usize = 0;
    let mut rpos: usize = 0;

    for change in diff.iter_all_changes() {
        let n = change.value().chars().count();
        match change.tag() {
            ChangeTag::Delete => {
                merge_push(&mut left_hl, lpos, lpos + n);
                lpos += n;
            }
            ChangeTag::Insert => {
                merge_push(&mut right_hl, rpos, rpos + n);
                rpos += n;
            }
            ChangeTag::Equal => {
                lpos += n;
                rpos += n;
            }
        }
    }

    (left_hl, right_hl)
}

/// Push a range, merging with the last one if contiguous.  Zero-width ranges are
/// silently discarded — they can arise on Unicode boundaries and must never reach
/// the renderer.
fn merge_push(v: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    if start >= end {
        return;
    }
    if let Some(last) = v.last_mut() {
        if last.1 == start {
            last.1 = end;
            return;
        }
    }
    v.push((start, end));
}

// ── Stats ─────────────────────────────────────────────────────────────────────

pub fn calc_stats(rows: &[DiffRow]) -> Stats {
    let mut s = Stats::default();
    for row in rows {
        match row.kind {
            RowKind::Added => {
                s.added += 1;
                s.chars_added += row.right_text.as_deref().unwrap_or("").chars().count();
            }
            RowKind::Removed => {
                s.removed += 1;
                s.chars_removed += row.left_text.as_deref().unwrap_or("").chars().count();
            }
            RowKind::Modified => {
                s.modified += 1;
                s.chars_added += row.right_highlights.iter().map(|(a, b)| b - a).sum::<usize>();
                s.chars_removed += row.left_highlights.iter().map(|(a, b)| b - a).sum::<usize>();
            }
            RowKind::Format => s.format += 1,
            RowKind::Equal => {}
        }
    }
    s
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── semantic_normalize ────────────────────────────────────────────────────

    #[test]
    fn normalize_no_brackets() {
        assert_eq!(semantic_normalize("hello world"), "hello world");
    }

    #[test]
    fn normalize_single_arg_on_multiple_lines() {
        let a = "foo(a, b, c)";
        let b = "foo(\n    a,\n    b,\n    c\n)";
        assert_eq!(semantic_normalize(a), semantic_normalize(b));
    }

    #[test]
    fn normalize_nested_brackets() {
        let a = "f(g(a, b), c)";
        let b = "f(g(\n  a,\n  b\n),\nc)";
        assert_eq!(semantic_normalize(a), semantic_normalize(b));
    }

    #[test]
    fn normalize_preserves_content_change() {
        let a = "foo(a, b)";
        let b = "foo(a, x)";
        assert_ne!(semantic_normalize(a), semantic_normalize(b));
    }

    #[test]
    fn normalize_mismatched_brackets_no_panic() {
        // Should not panic.
        let _ = semantic_normalize("))()(");
    }

    #[test]
    fn normalize_square_curly_brackets() {
        let a = "x[1, 2, 3]";
        let b = "x[\n  1,\n  2,\n  3\n]";
        assert_eq!(semantic_normalize(a), semantic_normalize(b));
    }

    // ── build_rows / process_diff ─────────────────────────────────────────────

    #[test]
    fn identical_files_produce_only_equal_rows() {
        let text = "line one\nline two\nline three\n";
        let (rows, stats) = process_diff(text, text);
        assert!(rows.iter().all(|r| r.kind == RowKind::Equal));
        assert_eq!(stats.added, 0);
        assert_eq!(stats.removed, 0);
        assert_eq!(stats.modified, 0);
    }

    #[test]
    fn empty_files_produce_no_rows() {
        let (rows, stats) = process_diff("", "");
        // both split to [""], so one Equal row with empty string
        assert!(rows.iter().all(|r| r.kind == RowKind::Equal));
        assert_eq!(stats.added, 0);
    }

    #[test]
    fn pure_addition() {
        let a = "line1\nline2\n";
        let b = "line1\nnew\nline2\n";
        let (rows, stats) = process_diff(a, b);
        assert_eq!(stats.added, 1);
        assert_eq!(stats.removed, 0);
        assert!(rows.iter().any(|r| r.kind == RowKind::Added));
    }

    #[test]
    fn pure_removal() {
        let a = "line1\nremoved\nline2\n";
        let b = "line1\nline2\n";
        let (rows, stats) = process_diff(a, b);
        assert_eq!(stats.removed, 1);
        assert_eq!(stats.added, 0);
        assert!(rows.iter().any(|r| r.kind == RowKind::Removed));
    }

    #[test]
    fn modification_detected() {
        let a = "foo(a, b)\n";
        let b = "foo(a, x)\n";
        let (rows, stats) = process_diff(a, b);
        assert_eq!(stats.modified, 1);
        assert_eq!(stats.added, 0);
        assert_eq!(stats.removed, 0);
        let modified = rows.iter().find(|r| r.kind == RowKind::Modified).unwrap();
        // The 'b'→'x' change should produce char highlights.
        assert!(!modified.left_highlights.is_empty());
        assert!(!modified.right_highlights.is_empty());
    }

    #[test]
    fn formatting_only_detected_python_style() {
        // Function definition on one line vs. multi-line args (no trailing comma —
        // a trailing comma would be a real content change, not just formatting).
        let a = "def foo(arg1, arg2, arg3):\n    pass\n";
        let b = "def foo(\n    arg1,\n    arg2,\n    arg3\n):\n    pass\n";
        let (rows, stats) = process_diff(a, b);
        assert!(
            rows.iter().any(|r| r.kind == RowKind::Format),
            "no format rows found; stats: {stats:?}, rows: {rows:?}"
        );
        assert_eq!(stats.added, 0, "unexpected additions: {stats:?}");
        assert_eq!(stats.removed, 0, "unexpected removals: {stats:?}");
    }

    #[test]
    fn formatting_only_not_triggered_for_real_change() {
        let a = "foo(a, b)\n";
        let b = "foo(\n  a,\n  x\n)\n";
        let (rows, _) = process_diff(a, b);
        // The argument changed (b → x), so at least one row must stay non-format.
        assert!(
            rows.iter().any(|r| r.kind != RowKind::Format && r.kind != RowKind::Equal),
            "expected a real content change, all rows: {rows:?}"
        );
        // The formatting-only lines ("a," and ")") should be classified as Format.
        assert!(
            rows.iter().filter(|r| r.kind == RowKind::Format).count() > 0,
            "expected some format-only rows, all rows: {rows:?}"
        );
    }

    #[test]
    fn left_right_line_numbers_are_consistent() {
        let a = "a\nb\nc\n";
        let b = "a\nx\nc\n";
        let (rows, _) = process_diff(a, b);
        let mut last_left = 0usize;
        let mut last_right = 0usize;
        for row in &rows {
            if let Some(n) = row.left_num {
                assert!(n > last_left, "left line numbers not monotone");
                last_left = n;
            }
            if let Some(n) = row.right_num {
                assert!(n > last_right, "right line numbers not monotone");
                last_right = n;
            }
        }
    }

    // ── char_diff_ranges ──────────────────────────────────────────────────────

    #[test]
    fn char_diff_identical_lines_no_highlights() {
        let (l, r) = char_diff_ranges("hello", "hello");
        assert!(l.is_empty());
        assert!(r.is_empty());
    }

    #[test]
    fn char_diff_single_substitution() {
        let (l, r) = char_diff_ranges("foo(a, b)", "foo(a, x)");
        // 'b' and 'x' are the changed chars.
        assert!(!l.is_empty());
        assert!(!r.is_empty());
        // Total highlighted chars should be 1 on each side.
        let l_len: usize = l.iter().map(|(a, b)| b - a).sum();
        let r_len: usize = r.iter().map(|(a, b)| b - a).sum();
        assert_eq!(l_len, 1);
        assert_eq!(r_len, 1);
    }

    #[test]
    fn char_diff_unicode_aware() {
        // Emoji are multi-byte but single char; offsets must count codepoints.
        let (l, r) = char_diff_ranges("a😀b", "a🎉b");
        let l_len: usize = l.iter().map(|(a, b)| b - a).sum();
        assert_eq!(l_len, 1);
        let r_len: usize = r.iter().map(|(a, b)| b - a).sum();
        assert_eq!(r_len, 1);
    }

    // ── calc_stats ────────────────────────────────────────────────────────────

    #[test]
    fn stats_added_chars() {
        let a = "";
        let b = "hello\n";
        let (_, stats) = process_diff(a, b);
        assert_eq!(stats.added, 1);
        assert_eq!(stats.chars_added, 5); // "hello"
    }

    #[test]
    fn stats_removed_chars() {
        let a = "bye\n";
        let b = "";
        let (_, stats) = process_diff(a, b);
        assert_eq!(stats.removed, 1);
        assert_eq!(stats.chars_removed, 3); // "bye"
    }
}
