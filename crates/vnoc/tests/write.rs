//! `identity::attach` with `EditSink::Disk`: the file on disk is the document, and nobody else is on the other end (`docs/21-agent-cli/04-write-commands.md` §2).
//! This is what the command line's write commands stand on. Needs the driver built first: `cd drivers/python && cargo build` (or `VNOC_DRIVER=<path>`).
//!
//! Every test works on a copy of `tests/sample/` outside this repository (its git root would be this repository otherwise).

#[path = "common/testdir.rs"]
mod testdir;

use std::path::{Path, PathBuf};

use vnoc::cli::project::{Opened, open};
use vnoc::identity::{AttachError, AttachOptions, EditSink, Env, attach};
use vnoc::protocol::{AttachParams, AttachResult, Document};
use vnoc::userdata::UserData;
use vnoc_domain::{IdInCode, NodeId};

const BOM: &[u8] = b"\xef\xbb\xbf";

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// As `lifecycle.rs` finds it: `VNOC_DRIVER`, else the debug build in `drivers/python`.
fn driver_path() -> PathBuf {
    std::env::var_os("VNOC_DRIVER")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            repo_root()
                .join("drivers/python/target/debug")
                .join(format!(
                    "vnoc-driver-python{}",
                    std::env::consts::EXE_SUFFIX
                ))
        })
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A fresh copy of the sample, symlinks resolved (a document's URI is compared with the project's path).
fn sample_copy(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("vnoc-write-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    testdir::remove_at_end(&root);
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample"),
        &root,
    );
    dunce::canonicalize(root).unwrap()
}

async fn attach_on_disk(
    opened: &Opened,
    key: &str,
    document: Option<Document>,
) -> Result<AttachResult, AttachError> {
    let env = Env {
        project: &opened.project,
        index: &opened.index,
        host: &opened.host,
        edits: EditSink::Disk,
    };
    attach(
        env,
        AttachParams {
            key: key.into(),
            title: "Why".into(),
            markers: None,
            document,
        },
        AttachOptions::default(),
    )
    .await
    .map(|attached| attached.result)
}

fn id_in_code(root: &Path, result: &AttachResult) -> Option<IdInCode> {
    UserData::new(root)
        .node_toml(&NodeId::parse(&result.id).unwrap())
        .unwrap()
        .id_in_code()
}

/// `after` with the line that carries `id` made empty (a byte-order mark on that line stays): what the file was before the ID went in.
fn without_the_id_line(after: &[u8], id: &str) -> Vec<u8> {
    let mut before = Vec::new();
    let mut found = 0;
    for line in after.split_inclusive(|&b| b == b'\n') {
        if line.windows(id.len()).any(|w| w == id.as_bytes()) {
            found += 1;
            if line.starts_with(BOM) {
                before.extend_from_slice(BOM);
            }
        } else {
            before.extend_from_slice(line);
        }
    }
    assert_eq!(found, 1, "exactly one line carries the ID");
    before
}

fn no_temp_files(dir: &Path) {
    let left: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[tokio::test]
async fn a_function_gets_its_id_line_in_the_file() {
    let root = sample_copy("function");
    let file = root.join("billing/tax.py");
    let before = std::fs::read(&file).unwrap();
    let opened = open(&root, &[driver_path()]).await.unwrap();

    let result = attach_on_disk(&opened, "billing/tax.py::apply_tax", None)
        .await
        .unwrap();

    assert!(result.id_in_code, "{:?}", result.notice);
    assert_eq!(result.notice, None);
    let after = std::fs::read(&file).unwrap();
    assert!(
        String::from_utf8_lossy(&after).contains(&format!("ID: {}", result.id)),
        "the ID: line is in the file"
    );
    assert_eq!(without_the_id_line(&after, &result.id), before);
    assert_eq!(id_in_code(&root, &result), Some(IdInCode::Yes));
    assert!(Path::new(&result.doc).is_file());
    no_temp_files(file.parent().unwrap());
}

#[tokio::test]
async fn a_file_gets_its_file_id_comment() {
    let root = sample_copy("file");
    let file = root.join("orders/status.py");
    let before = std::fs::read(&file).unwrap();
    let opened = open(&root, &[driver_path()]).await.unwrap();

    let result = attach_on_disk(&opened, "orders/status.py", None)
        .await
        .unwrap();

    assert!(result.id_in_code, "{:?}", result.notice);
    let after = String::from_utf8(std::fs::read(&file).unwrap()).unwrap();
    assert!(after.contains(&format!("# FileID: {}", result.id)));
    assert_eq!(without_the_id_line(after.as_bytes(), &result.id), before);
    assert_eq!(id_in_code(&root, &result), Some(IdInCode::Yes));
    no_temp_files(file.parent().unwrap());
}

#[tokio::test]
async fn a_file_that_is_not_utf8_is_attached_by_name_with_the_notice() {
    let root = sample_copy("latin1");
    let file = root.join("billing/tax.py");
    let opened = open(&root, &[driver_path()]).await.unwrap(); // the index was built while the file was still text
    let latin1 = b"\"\"\"Sales tax, caf\xe9.\"\"\"\n\n\ndef rate_for(country):\n    return 0.0\n";
    std::fs::write(&file, latin1).unwrap();

    let result = attach_on_disk(&opened, "billing/tax.py::apply_tax", None)
        .await
        .unwrap();

    assert!(!result.id_in_code);
    let notice = result.notice.expect("a notice says why");
    assert!(
        notice.starts_with("The ID could not be written (")
            && notice.ends_with("); this doc is attached by name."),
        "{notice}"
    );
    assert!(notice.contains("not UTF-8"), "{notice}");
    assert_eq!(
        std::fs::read(&file).unwrap(),
        latin1,
        "the file is untouched"
    );
    assert_eq!(
        id_in_code(
            &root,
            &AttachResult {
                notice: None,
                ..result
            }
        ),
        Some(IdInCode::No)
    );
}

#[tokio::test]
async fn a_file_edited_between_the_plan_and_the_apply_is_attached_by_name_and_left_as_it_is() {
    let root = sample_copy("changed");
    let file = root.join("billing/tax.py");
    let opened = open(&root, &[driver_path()]).await.unwrap();
    // the plan is made for this text...
    let planned_for = Document::from_disk(&file).unwrap();
    // ...and somebody saves the file before the edit is applied
    let edited = format!("# somebody was here\n{}", planned_for.text);
    std::fs::write(&file, &edited).unwrap();

    let result = attach_on_disk(&opened, "billing/tax.py::apply_tax", Some(planned_for))
        .await
        .unwrap();

    assert!(!result.id_in_code);
    let notice = result.notice.clone().expect("a notice says why");
    assert!(
        notice.contains("the file changed while the ID was being written"),
        "{notice}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        edited,
        "byte for byte"
    );
    assert_eq!(id_in_code(&root, &result), Some(IdInCode::No));
    no_temp_files(file.parent().unwrap());
}

#[tokio::test]
async fn a_byte_order_mark_is_kept_for_a_function_and_for_a_file() {
    for (key, label) in [("bom.py::with_mark", "bom-def"), ("bom.py", "bom-file")] {
        let root = sample_copy(label);
        let file = root.join("bom.py");
        let before = [BOM, b"def with_mark(a):\n    return a\n".as_slice()].concat();
        std::fs::write(&file, &before).unwrap();
        let opened = open(&root, &[driver_path()]).await.unwrap();

        let result = attach_on_disk(&opened, key, None).await.unwrap();

        assert!(result.id_in_code, "{key}: {:?}", result.notice);
        let after = std::fs::read(&file).unwrap();
        assert!(
            after.starts_with(BOM),
            "{key}: the mark is still the first bytes: {after:?}"
        );
        assert_eq!(without_the_id_line(&after, &result.id), before, "{key}");
        no_temp_files(&root);
    }
}

#[tokio::test]
async fn a_byte_order_mark_is_kept_when_the_function_is_further_down() {
    let root = sample_copy("bom-later");
    let file = root.join("bom.py");
    let before = [
        BOM,
        b"\"\"\"A module with a mark.\"\"\"\n\nimport os\n\n\ndef later(a):\n    return os.sep + a\n".as_slice(),
    ]
    .concat();
    std::fs::write(&file, &before).unwrap();
    let opened = open(&root, &[driver_path()]).await.unwrap();

    let result = attach_on_disk(&opened, "bom.py::later", None)
        .await
        .unwrap();

    assert!(result.id_in_code, "{:?}", result.notice);
    let after = std::fs::read(&file).unwrap();
    assert!(after.starts_with(BOM));
    assert_eq!(without_the_id_line(&after, &result.id), before);
    no_temp_files(&root);
}

#[tokio::test]
async fn crlf_line_ends_are_kept_for_a_function_and_for_a_file() {
    for (key, label) in [("crlf.py::with_crlf", "crlf-def"), ("crlf.py", "crlf-file")] {
        let root = sample_copy(label);
        let file = root.join("crlf.py");
        let before = b"def with_crlf(a):\r\n    b = a\r\n    return b\r\n";
        std::fs::write(&file, before).unwrap();
        let opened = open(&root, &[driver_path()]).await.unwrap();

        let result = attach_on_disk(&opened, key, None).await.unwrap();

        assert!(result.id_in_code, "{key}: {:?}", result.notice);
        let after = std::fs::read(&file).unwrap();
        assert_eq!(without_the_id_line(&after, &result.id), before, "{key}");
        no_temp_files(&root);
    }
}
