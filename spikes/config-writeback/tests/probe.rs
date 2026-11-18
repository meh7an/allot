//! Where `toml_edit` keeps comments, which decides what an edit takes along with a key, and
//! how line endings and a byte order mark survive.

use allot_shared::config::Config;
use config_writeback::edit;
use toml_edit::DocumentMut;

fn prefix(decor: &toml_edit::Decor) -> &str {
    decor.prefix().and_then(|raw| raw.as_str()).unwrap_or("")
}

#[test]
fn test_comment_above_key_belongs_to_that_key() {
    let doc: DocumentMut = "[a]\n# about x\nx = 1\n".parse().unwrap();
    let key = doc["a"].as_table().unwrap().key("x").unwrap();
    assert_eq!(prefix(key.leaf_decor()), "# about x\n");
}

#[test]
fn test_comment_between_tables_belongs_to_next_header() {
    let doc: DocumentMut = "[a]\nx = 1\n# about a, really\n\n[b]\ny = 2\n"
        .parse()
        .unwrap();
    assert_eq!(
        prefix(doc["b"].as_table().unwrap().decor()),
        "# about a, really\n\n"
    );
}

#[test]
fn test_comment_after_last_key_belongs_to_document() {
    let doc: DocumentMut = "[a]\nx = 1\n# the end\n".parse().unwrap();
    assert_eq!(doc.trailing().as_str(), Some("# the end\n"));
}

const CRLF: &str = "# top\r\n[admission]\r\nheadroom_pct = 20.0  # c\r\n";

/// Both parsers read Windows line endings, but `toml_edit` prints `\n` everywhere.
#[test]
fn test_crlf_read_by_both_printed_as_lf() {
    let doc: DocumentMut = CRLF.parse().unwrap();
    assert_eq!(doc.to_string(), CRLF.replace("\r\n", "\n"));
    assert_eq!(
        Config::from_toml(CRLF).unwrap().admission.headroom_pct,
        20.0
    );
}

#[test]
fn test_layout_keeps_crlf_for_old_and_new_lines() {
    let mut doc: DocumentMut = CRLF.parse().unwrap();
    let layout = edit::Layout::of(CRLF);
    assert_eq!(layout.render(&doc), CRLF);
    edit::set(&mut doc, &["estimates", "min_runs"], 5);
    assert_eq!(
        layout.render(&doc),
        format!("{CRLF}\r\n[estimates]\r\nmin_runs = 5\r\n")
    );
}

/// Notepad can save UTF-8 with a byte order mark: both parsers accept it, `toml_edit` drops
/// it on output, and the layout puts it back.
#[test]
fn test_byte_order_mark_read_by_both_kept_by_layout() {
    let text = "\u{feff}[admission]\nheadroom_pct = 20.0\n";
    let doc: DocumentMut = text.parse().unwrap();
    assert_eq!(doc.to_string(), &text[3..]);
    assert_eq!(edit::Layout::of(text).render(&doc), text);
    assert_eq!(
        Config::from_toml(text).unwrap().admission.headroom_pct,
        20.0
    );
}

/// A file with mixed endings cannot be rebuilt byte for byte, so it is written with `\n`.
#[test]
fn test_mixed_line_endings_written_as_lf() {
    let text = "[a]\r\nx = 1\ny = 2\r\n";
    let doc: DocumentMut = text.parse().unwrap();
    assert_eq!(edit::Layout::of(text).render(&doc), "[a]\nx = 1\ny = 2\n");
}
