/// Integration tests for difff's diff engine.
///
/// These test the full `process_diff` pipeline end-to-end, including semantic
/// normalisation and character-level highlighting.
use difff::diff::{process_diff, RowKind};

// ── Regression: real-world Python refactoring ─────────────────────────────────

#[test]
fn python_multiline_args_is_format_only() {
    // No trailing commas — those would be content changes, not formatting.
    let before = r#"
def create_user(username, email, password, is_admin=False):
    return User(username=username, email=email, password=password, is_admin=is_admin)
"#;
    let after = r#"
def create_user(
    username,
    email,
    password,
    is_admin=False
):
    return User(
        username=username,
        email=email,
        password=password,
        is_admin=is_admin
    )
"#;
    let (rows, stats) = process_diff(before, after);
    assert!(
        stats.format > 0,
        "expected formatting rows, got stats: {stats:?}, rows: {rows:?}"
    );
    assert_eq!(stats.added, 0, "unexpected additions: {stats:?}");
    assert_eq!(stats.removed, 0, "unexpected removals: {stats:?}");
    assert!(
        rows.iter().all(|r| r.kind != RowKind::Modified),
        "unexpected Modified rows; rows: {rows:?}"
    );
}

#[test]
fn python_arg_change_is_not_format_only() {
    let before = "result = process(data, mode='fast', retries=3)\n";
    let after  = "result = process(data, mode='safe', retries=3)\n";
    let (rows, stats) = process_diff(before, after);
    assert_eq!(stats.format, 0, "should not be format-only: {stats:?}");
    assert!(
        rows.iter().any(|r| r.kind == RowKind::Modified),
        "expected a Modified row"
    );
}

// ── Multi-file scenario ───────────────────────────────────────────────────────

#[test]
fn large_identical_file_produces_no_diff() {
    // 1 000 identical lines.
    let text: String = (1..=1000).map(|i| format!("line {i}\n")).collect();
    let (rows, stats) = process_diff(&text, &text);
    assert!(rows.iter().all(|r| r.kind == RowKind::Equal));
    assert_eq!(stats.added + stats.removed + stats.modified + stats.format, 0);
}

#[test]
fn single_line_insertion_in_large_file() {
    let base: Vec<String> = (1..=500).map(|i| format!("line {i}\n")).collect();
    let mut modified = base.clone();
    modified.insert(250, "NEW LINE\n".to_owned());
    let a: String = base.concat();
    let b: String = modified.concat();

    let (_, stats) = process_diff(&a, &b);
    assert_eq!(stats.added, 1);
    assert_eq!(stats.removed, 0);
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn both_empty() {
    let (rows, stats) = process_diff("", "");
    assert_eq!(stats.added + stats.removed + stats.modified, 0);
    // An empty string still has one "line" after split('\n').
    assert!(rows.iter().all(|r| r.kind == RowKind::Equal));
}

#[test]
fn left_empty_right_has_content() {
    let (_, stats) = process_diff("", "hello\nworld\n");
    assert_eq!(stats.added, 2);
    assert_eq!(stats.removed, 0);
}

#[test]
fn right_empty_left_has_content() {
    let (_, stats) = process_diff("hello\nworld\n", "");
    assert_eq!(stats.removed, 2);
    assert_eq!(stats.added, 0);
}

#[test]
fn line_numbers_start_at_one_and_are_monotone() {
    let a = "alpha\nbeta\ngamma\n";
    let b = "alpha\ndelta\ngamma\n";
    let (rows, _) = process_diff(a, b);

    let (mut prev_l, mut prev_r) = (0usize, 0usize);
    for row in &rows {
        if let Some(n) = row.left_num  { assert!(n > prev_l); prev_l = n; }
        if let Some(n) = row.right_num { assert!(n > prev_r); prev_r = n; }
    }
}

#[test]
fn char_highlights_cover_exact_changed_region() {
    let a = "version = \"1.2.3\"\n";
    let b = "version = \"1.2.4\"\n";
    let (rows, _) = process_diff(a, b);

    let modified = rows.iter().find(|r| r.kind == RowKind::Modified).unwrap();
    let l_len: usize = modified.left_highlights.iter().map(|(s, e)| e - s).sum();
    let r_len: usize = modified.right_highlights.iter().map(|(s, e)| e - s).sum();
    assert_eq!(l_len, 1, "only '3' should be highlighted on the left");
    assert_eq!(r_len, 1, "only '4' should be highlighted on the right");
}

#[test]
fn unicode_filenames_and_content() {
    let a = "путь = \"/tmp/тест\"\n";
    let b = "путь = \"/tmp/тест2\"\n";
    let (rows, _) = process_diff(a, b);
    assert!(rows.iter().any(|r| r.kind == RowKind::Modified));
}

#[test]
fn mixed_additions_removals_modifications() {
    let a = "keep\nchange_me\ndelete_me\n";
    let b = "keep\nchanged\nadd_me\n";
    let (_, stats) = process_diff(a, b);
    // "change_me" → "changed" is a modification.
    // "delete_me" → "add_me" is also paired as modified (Myers pairs them).
    assert!(stats.modified >= 1);
}

#[test]
fn python_same_code_different_formatting_is_format_only() {
    let a = r#"def calculate_total(items, tax_rate=0.05, discount=0.1):
    total = sum(item['price'] for item in items)
    return (total - discount) * (1 + tax_rate)

shopping_cart = [{'name': 'apple', 'price': 1.0}, {'name': 'banana', 'price': 0.5}]
print("Total:", calculate_total(shopping_cart))
"#;
    let b = r#"def calculate_total(
    items,
    tax_rate=0.05,
    discount=0.1
):
    total = sum(
        item["price"]
        for item in items
    )
    return (total - discount) * (1 + tax_rate)

shopping_cart = [
    {
        "name": "apple",
        "price": 1.0
    },
    {
        "name": "banana",
        "price": 0.5
    }
]

print(
    "Total:",
    calculate_total(shopping_cart)
)
"#;
    let (rows, stats) = process_diff(a, b);
    assert!(
        stats.format > 0,
        "expected format-only rows, got stats: {stats:?}, rows: {rows:?}"
    );
    assert_eq!(stats.added, 0, "unexpected additions: {stats:?}");
    assert_eq!(stats.removed, 0, "unexpected removals: {stats:?}");
    assert_eq!(stats.modified, 0, "unexpected modifications: {stats:?}");
}

#[test]
fn formatting_with_one_content_change_shows_only_that_change() {
    let a = r#"def calculate_total(items, tax_rate=0.05, discount=0.1):
    total = sum(item['price'] for item in items)
    return (total - discount) * (1 + tax_rate)

shopping_cart = [{'name': 'apple', 'price': 1.0}, {'name': 'banana', 'price': 0.5}]
print("Total:", calculate_total(shopping_cart))
"#;
    let c = r#"def calculate_total(
    items,
    tax_rate=0.05,
    discount=0.1
):
    total = sum(
        item["price"]
        for item in items
    )
    return (total - discount) * (1 + tax_rate)

shopping_cart = [
    {
        "name": "aple",
        "price": 1.0
    },
    {
        "name": "banana",
        "price": 0.5
    }
]

print(
    "Total:",
    calculate_total(shopping_cart)
)
"#;
    let (_, stats) = process_diff(a, c);
    // The lines containing the real content change stay as Removed/Added;
    // everything else should be Format. No spurious Modified rows.
    assert!(stats.added >= 1, "expected at least 1 added line: {stats:?}");
    assert!(stats.removed >= 1, "expected at least 1 removed line: {stats:?}");
    assert_eq!(stats.modified, 0, "unexpected Modified rows: {stats:?}");
    assert!(
        stats.format > 0,
        "expected format-only rows: {stats:?}"
    );
}
