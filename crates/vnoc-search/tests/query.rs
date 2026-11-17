//! The parse (01 §2, §4, §5): a table of inputs and the `Query` each gives, and the shape of a reference.

use vnoc_search::KindFilter::*;
use vnoc_search::{Query, parse};

fn v(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn words(items: &[&str]) -> Query {
    Query {
        words: v(items),
        ..Query::default()
    }
}

fn kinds(items: &[vnoc_search::KindFilter]) -> Query {
    Query {
        kinds: Some(items.to_vec()),
        ..Query::default()
    }
}

fn inside(items: &[&str]) -> Query {
    Query {
        inside: v(items),
        ..Query::default()
    }
}

const NOT_A_KIND: &str = "is not a kind (folder, file, class, function, method)";

/// A query that keeps nothing, with the lines it noted.
fn nothing(notes: &[String], rest: Query) -> Query {
    Query {
        keeps_nothing: true,
        notes: notes.to_vec(),
        ..rest
    }
}

fn fnn() -> Vec<String> {
    vec![format!("`kind:fnn` {NOT_A_KIND}")]
}

const TOGETHER: &str = "`has:doc` and `no:doc` together keep nothing";

#[test]
fn the_parse_table() {
    let doc = |docs| Query {
        docs: Some(docs),
        ..Query::default()
    };
    let rows: Vec<(&str, Query)> = vec![
        // nothing
        ("", Query::default()),
        ("   ", Query::default()),
        // words
        ("chor", words(&["chor"])),
        (
            "Chor  OrderService.par",
            words(&["chor", "orderservice.par"]),
        ),
        ("billing/", words(&["billing/"])),
        // kind:
        ("kind:fn", kinds(&[Function])),
        ("kind:function", kinds(&[Function])),
        ("kind:method", kinds(&[Method])),
        ("kind:cls", kinds(&[Class])),
        ("kind:folder,file", kinds(&[Folder, File])),
        ("kind:class,fn", kinds(&[Class, Function])),
        ("kind:fn kind:class", kinds(&[Function, Class])),
        ("kind:fn kind:fn", kinds(&[Function])),
        ("KIND:FN", kinds(&[Function])),
        ("Kind:Cls,METHOD", kinds(&[Class, Method])),
        ("kind:class,", kinds(&[Class])),
        (
            "-kind:file",
            Query {
                not_kinds: vec![File],
                ..Query::default()
            },
        ),
        (
            "kind:fn -kind:method",
            Query {
                kinds: Some(vec![Function]),
                not_kinds: vec![Method],
                ..Query::default()
            },
        ),
        // has: and no:
        ("has:doc", doc(true)),
        ("no:doc", doc(false)),
        ("-has:doc", doc(false)),
        ("-no:doc", doc(true)),
        ("HAS:DOC", doc(true)),
        ("has:doc has:doc", doc(true)),
        (
            "has:doc no:doc",
            nothing(&[TOGETHER.to_string()], doc(true)),
        ),
        (
            "no:doc has:doc",
            nothing(&[TOGETHER.to_string()], doc(false)),
        ),
        (
            "has:doc -has:doc",
            nothing(&[TOGETHER.to_string()], doc(true)),
        ),
        (
            "has:code",
            nothing(
                &["`has:code` is not a filter value: has: and no: take doc".to_string()],
                Query::default(),
            ),
        ),
        (
            "-no:Code",
            nothing(
                &["`no:code` is not a filter value: has: and no: take doc".to_string()],
                Query::default(),
            ),
        ),
        // in:
        ("in:billing", inside(&["billing"])),
        ("in:billing/", inside(&["billing"])),
        ("in:.\\billing\\", inside(&["billing"])),
        ("in:./Billing/Invoice.PY", inside(&["billing/invoice.py"])),
        ("IN:Billing", inside(&["billing"])),
        ("in:\"my folder/\"", inside(&["my folder"])),
        ("in:a/ in:a/b/", inside(&["a", "a/b"])),
        (
            "-in:tests/",
            Query {
                not_inside: v(&["tests"]),
                ..Query::default()
            },
        ),
        // a value that means nothing
        ("kind:fnn", nothing(&fnn(), Query::default())),
        ("kind:fnn kind:fnn", nothing(&fnn(), Query::default())),
        ("-kind:fnn kind:fnn", nothing(&fnn(), Query::default())),
        ("KIND:FNN", nothing(&fnn(), Query::default())),
        ("kind:class,fnn", nothing(&fnn(), kinds(&[Class]))),
        (
            "kind:,",
            nothing(&[format!("`kind:,` {NOT_A_KIND}")], Query::default()),
        ),
        // a name that is not a filter, or a filter with no value: a word
        ("kind:", words(&["kind:"])),
        ("-in:", words(&["-in:"])),
        ("has:", words(&["has:"])),
        ("NO:", words(&["no:"])),
        ("http://x", words(&["http://x"])),
        ("self:", words(&["self:"])),
        ("tag:plan", words(&["tag:plan"])),
        ("author:me", words(&["author:me"])),
        ("-1", words(&["-1"])),
        ("-", words(&["-"])),
        ("kind:\"\"", words(&["kind:"])),
        // phrases
        (
            "\"retry  Policy\"",
            Query {
                phrases: v(&["retry policy"]),
                ..Query::default()
            },
        ),
        (
            "\"  retry\tpolicy \"",
            Query {
                phrases: v(&["retry policy"]),
                ..Query::default()
            },
        ),
        ("\"unclosed phrase", words(&["unclosed", "phrase"])),
        (
            "a \"b c\" \"d",
            Query {
                words: v(&["a", "d"]),
                phrases: v(&["b c"]),
                ..Query::default()
            },
        ),
        ("\"\"", Query::default()),
        ("\"   \"", Query::default()),
        ("\"\" a", words(&["a"])),
        // all of it
        (
            "kind:fn no:doc in:billing/ -in:billing/old retry \"a b\"",
            Query {
                words: v(&["retry"]),
                phrases: v(&["a b"]),
                kinds: Some(vec![Function]),
                docs: Some(false),
                inside: v(&["billing"]),
                not_inside: v(&["billing/old"]),
                ..Query::default()
            },
        ),
    ];
    for (input, expect) in rows {
        assert_eq!(parse(input), expect, "{input:?}");
    }
}

#[test]
fn references_have_a_shape() {
    let id = "0q8v5w4t2f7kx9m3b6d1rz4h7k";
    assert_eq!(id.len(), 26);
    let references = [
        "src/orders.py:142",
        "a.py:12:5",
        "C:\\repo\\a.py:12",
        "src/orders.py::OrderService.parse_order",
        id,
        "0Q8V5W4T2F7KX9M3B6D1RZ4H7K", // the same, in capitals
        "0IL8V5W4T2F7KX9M3B6D1RZ4HO", // `I` `L` `O` are read as digits, so they are allowed
    ];
    for token in references {
        let q = parse(token);
        assert_eq!(q.reference.as_deref(), Some(token), "{token}");
        assert_eq!(q.words, v(&[&token.to_lowercase()]), "{token}: a word too");
        assert!(!q.keeps_nothing && q.notes.is_empty(), "{token}");
    }

    let not_references = [
        "0Q8V5W4T2F7KX9M3B6D1RZ4H7U", // a `U`
        "0q8v5w4t2f7kx9m3b6d1rz4h7u",
        "0q8v5w4t2f7kx9m3b6d1rz4h7",   // 25
        "0q8v5w4t2f7kx9m3b6d1rz4h7kk", // 27
        "0q8v5w4t2f7kx9m3b6d1rz4h7-",  // not letters and digits
        "src/orders.py",
        "orders.py:",
        "orders.py:12a",
        "OrderService.parse_order",
        "billing/",
    ];
    for token in not_references {
        let q = parse(token);
        assert_eq!(q.reference, None, "{token}");
        assert_eq!(q.words, v(&[&token.to_lowercase()]), "{token}: a word");
    }
}

#[test]
fn a_reference_is_the_whole_query() {
    // two tokens are never a reference, though one has the shape: both are words
    for text in [
        "src/orders.py:142 retry",
        "retry src/orders.py:142",
        "src/orders.py:142 a.py:12",
        "src/orders.py:142 \"retry\"",
        "src/orders.py:142 kind:fn",
    ] {
        assert_eq!(parse(text).reference, None, "{text}");
    }
    assert_eq!(
        parse("src/orders.py:142 retry").words,
        v(&["src/orders.py:142", "retry"])
    );
    // a filter and a phrase are not words, so not references, though their text has the shape
    for text in [
        "in:src/a.py:12",
        "\"a.py:12\"",
        "kind:fn",
        "-in:src/a.py:12",
    ] {
        assert_eq!(parse(text).reference, None, "{text}");
    }
    // an empty phrase is not a token: this is the reference alone
    assert_eq!(parse("a.py:12 \"\"").reference.as_deref(), Some("a.py:12"));
}
