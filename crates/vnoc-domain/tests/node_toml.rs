//! node.toml round trips: what the MVP writes, and what must survive a read-modify-write.

use vnoc_domain::{IdInCode, LocationKey, NodeKind, NodeToml, RepoPath, Status};

fn loc(path: &str, q: &str, occ: u32) -> LocationKey {
    LocationKey::new(RepoPath::new(path).unwrap(), q, occ).unwrap()
}

#[test]
fn a_new_file_has_exactly_the_mvp_fields() {
    let text = NodeToml::new(&loc("src/orders.py", "OrderService.parse_order", 1)).to_string();
    assert_eq!(text, "path = \"src/orders.py\"\nqualname = \"OrderService.parse_order\"\nid_in_code = \"yes\"\nstatus = \"active\"\n");
}

#[test]
fn occurrence_is_written_only_when_it_is_not_one() {
    assert!(!NodeToml::new(&loc("a.py", "f", 1)).to_string().contains("occurrence"));
    let two = NodeToml::new(&loc("a.py", "f", 2)).to_string();
    assert!(two.contains("occurrence = 2"));
    assert_eq!(NodeToml::parse(&two).unwrap().location().unwrap().occurrence(), 2);
    assert_eq!(NodeToml::parse("path = \"a.py\"\nqualname = \"f\"\n").unwrap().location().unwrap().occurrence(), 1);
}

#[test]
fn a_new_file_reads_back_to_the_same_location() {
    let l = loc("src/a.py", "top.<locals>.inner", 3);
    let back = NodeToml::parse(&NodeToml::new(&l).to_string()).unwrap();
    assert_eq!(back.location().unwrap(), l);
    assert_eq!(back.id_in_code(), Some(IdInCode::Yes));
    assert_eq!(back.status(), Status::Active);
}

/// A file written by a NEWER V-NOC: an unknown field, an unknown value, comments, a different key order.
const FROM_THE_FUTURE: &str = r#"# written by V-NOC 9
status = "active"
future_field = { a = 1, b = [1, 2, 3] }   # a field this version has never heard of
path = "src/orders.py"
qualname = "OrderService.parse_order"
id_in_code = "sometimes"                   # a value this version does not know

[future_table]
x = "y"
"#;

#[test]
fn an_untouched_file_comes_back_byte_for_byte() {
    assert_eq!(NodeToml::parse(FROM_THE_FUTURE).unwrap().to_string(), FROM_THE_FUTURE);
}

#[test]
fn unknown_fields_values_comments_and_order_survive_edits() {
    let mut n = NodeToml::parse(FROM_THE_FUTURE).unwrap();
    assert_eq!(n.id_in_code(), Some(IdInCode::Other("sometimes".into())), "an unknown value is read, not rejected");

    n.set_location(&loc("src/sales.py", "OrderService.parse_order", 1));
    n.archive("2026-11-06".parse().unwrap());
    let out = n.to_string();

    for kept in ["# written by V-NOC 9", "future_field = { a = 1, b = [1, 2, 3] }", "# a field this version has never heard of", "id_in_code = \"sometimes\"", "[future_table]", "x = \"y\""] {
        assert!(out.contains(kept), "lost `{kept}` in:\n{out}");
    }
    assert!(out.contains("path = \"src/sales.py\""));
    assert!(out.contains("status = \"archived\""));
    assert!(out.contains("archived_at = 2026-11-06"));
    assert!(out.find("status").unwrap() < out.find("path").unwrap(), "the key order the file had is kept");

    let back = NodeToml::parse(&out).unwrap();
    assert_eq!(back.status(), Status::Archived);
    assert_eq!(back.archived_at().as_deref(), Some("2026-11-06"));
    assert_eq!(back.id_in_code(), Some(IdInCode::Other("sometimes".into())));
}

#[test]
fn unarchive_removes_the_date_and_keeps_the_rest() {
    let mut n = NodeToml::new(&loc("a.py", "f", 1));
    n.archive("2026-11-06".parse().unwrap());
    n.unarchive();
    assert_eq!(n.status(), Status::Active);
    assert_eq!(n.archived_at(), None);
    assert!(!n.to_string().contains("archived_at"));
}

#[test]
fn moving_a_node_back_to_occurrence_one_drops_the_field() {
    let mut n = NodeToml::new(&loc("a.py", "f", 2));
    n.set_location(&loc("a.py", "f", 1));
    assert!(!n.to_string().contains("occurrence"));
}

#[test]
fn a_file_without_an_anchor_is_an_error_not_a_guess() {
    assert!(NodeToml::parse("qualname = \"f\"\n").is_err());
    assert!(NodeToml::parse("path = \"a.py\"\n").is_err());
    assert!(NodeToml::parse("path = \"/abs/a.py\"\nqualname = \"f\"\n").is_err());
    assert!(NodeToml::parse("path = \"a.py\"\nqualname = \"f\"\noccurrence = 0\n").is_err());
    assert!(NodeToml::parse("this is = not [ toml").is_err());
}

#[test]
fn kind_is_written_when_set_and_a_file_without_it_reads_as_unknown_and_stays_as_it_is() {
    let mut n = NodeToml::new(&loc("a.py", "f", 1));
    assert_eq!(n.kind(), None, "a new file has no kind until the code sets one");
    n.set_kind(&NodeKind::Function);
    assert!(n.to_string().contains("kind = \"function\"\n"));
    assert_eq!(NodeToml::parse(&n.to_string()).unwrap().kind(), Some(NodeKind::Function));

    let old = "path = \"a.py\"\nqualname = \"f\"\nid_in_code = \"yes\"\n# my note\n";
    let parsed = NodeToml::parse(old).unwrap();
    assert_eq!(parsed.kind(), None);
    assert_eq!(parsed.to_string(), old, "reading never writes a kind");
}

#[test]
fn an_unknown_kind_is_kept_and_a_relink_can_replace_it() {
    let text = "path = \"a.py\"\nqualname = \"f\"\nkind = \"method\"\nfuture = 1\n";
    let mut n = NodeToml::parse(text).unwrap();
    assert_eq!(n.kind(), Some(NodeKind::Other("method".into())));
    assert_eq!(n.to_string(), text);
    n.set_kind(&NodeKind::Class);
    assert_eq!(n.to_string(), "path = \"a.py\"\nqualname = \"f\"\nkind = \"class\"\nfuture = 1\n", "other keys keep their place");
}
