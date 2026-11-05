//! Keys: where the secret goes, what the key list file holds, and the rules for adding one.

use std::sync::Arc;

use flickfluent_core::cloud::keys::{KeyBook, KeyState, Storage};
use flickfluent_core::cloud::keystore::{KeyStore, MemoryStore};
use flickfluent_core::cloud::services;
use flickfluent_core::error::*;

const SECRET: &str = "AIzaSyUNIQUEMARKER1234";

fn book(dir: &std::path::Path, store: Arc<dyn KeyStore>) -> KeyBook {
    KeyBook::new(dir.to_path_buf(), store, services::load().unwrap())
}

fn add(
    b: &KeyBook,
    label: &str,
    secret: &str,
) -> Result<flickfluent_core::cloud::keys::KeyInfo, AppError> {
    b.add("google-gemini", label, secret.to_string(), None, false)
}

#[test]
fn the_secret_goes_to_the_store_and_only_safe_facts_go_to_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::new());
    let b = book(dir.path(), store.clone());
    let key = add(&b, "personal", SECRET).unwrap();
    assert_eq!(key.last4, "1234");
    assert_eq!(key.state, KeyState::Untested);
    assert_eq!(key.storage, Storage::Os);
    assert_eq!(store.get(&key.id).unwrap().unwrap().expose(), SECRET);

    let file = std::fs::read_to_string(dir.path().join("cloud-keys.json")).unwrap();
    assert!(
        !file.contains(SECRET),
        "secret text is in the key list file"
    );
    assert!(file.contains("1234") && file.contains("personal") && file.contains("fingerprint"));
}

#[test]
fn a_restart_lists_the_key_masked_and_the_store_still_has_the_secret() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::new());
    let id = add(&book(dir.path(), store.clone()), "a", SECRET)
        .unwrap()
        .id;
    let again = book(dir.path(), store.clone());
    let list = again.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].last4, "1234");
    assert_eq!(again.secret(&id).unwrap().unwrap().expose(), SECRET);
}

#[test]
fn the_same_key_twice_is_a_duplicate_even_with_another_label_or_spacing() {
    let dir = tempfile::tempdir().unwrap();
    let b = book(dir.path(), Arc::new(MemoryStore::new()));
    add(&b, "one", SECRET).unwrap();
    assert_eq!(add(&b, "two", SECRET).unwrap_err().code, KEY_DUPLICATE);
    assert_eq!(
        add(&b, "two", &format!("  {SECRET}\n")).unwrap_err().code,
        KEY_DUPLICATE
    );
    assert_eq!(
        add(&b, "one", "another-key-0000").unwrap_err().code,
        KEY_DUPLICATE
    );
}

#[test]
fn text_is_trimmed_and_the_rules_hold() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::new());
    let b = book(dir.path(), store.clone());
    let key = add(
        &b,
        "ok",
        &format!(" {}\r\n{} \n", &SECRET[..10], &SECRET[10..]),
    )
    .unwrap();
    assert_eq!(store.get(&key.id).unwrap().unwrap().expose(), SECRET);

    assert_eq!(add(&b, "empty", "  \n ").unwrap_err().code, KEY_INVALID);
    assert_eq!(
        add(&b, "long", &"k".repeat(513)).unwrap_err().code,
        KEY_INVALID
    );
    assert!(add(&b, "max", &"m".repeat(512)).is_ok());
    assert_eq!(add(&b, "", "some-key-0001").unwrap_err().code, KEY_INVALID);
    assert_eq!(
        add(&b, &"l".repeat(41), "some-key-0002").unwrap_err().code,
        KEY_INVALID
    );
    assert_eq!(
        b.add("nope", "x", "some-key-0003".into(), None, false)
            .unwrap_err()
            .code,
        KEY_INVALID
    );
}

#[test]
fn a_service_that_needs_a_region_refuses_a_key_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let b = book(dir.path(), Arc::new(MemoryStore::new()));
    let none = b.add("azure-ai", "az", "azure-key-0001".into(), None, false);
    assert_eq!(none.unwrap_err().code, KEY_INVALID);
    let ok = b
        .add(
            "azure-ai",
            "az",
            "azure-key-0001".into(),
            Some("westeurope".into()),
            false,
        )
        .unwrap();
    assert_eq!(ok.region.as_deref(), Some("westeurope"));
}

#[test]
fn remove_deletes_the_store_entry_then_the_row() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(MemoryStore::new());
    let b = book(dir.path(), store.clone());
    let key = add(&b, "a", SECRET).unwrap();
    b.remove(&key.id).unwrap();
    assert!(store.get(&key.id).unwrap().is_none());
    assert!(b.list().unwrap().is_empty());
    assert!(b.secret(&key.id).unwrap().is_none());
}

#[test]
fn reorder_changes_only_the_keys_of_one_service() {
    let dir = tempfile::tempdir().unwrap();
    let b = book(dir.path(), Arc::new(MemoryStore::new()));
    let g1 = add(&b, "g1", "gemini-key-0001").unwrap();
    let g2 = add(&b, "g2", "gemini-key-0002").unwrap();
    let az = b
        .add(
            "azure-ai",
            "az",
            "azure-key-0001".into(),
            Some("westeurope".into()),
            false,
        )
        .unwrap();
    assert_eq!((g1.order, g2.order, az.order), (0, 1, 0));

    b.reorder("google-gemini", &[g2.id.clone(), g1.id.clone()])
        .unwrap();
    let list = b.list().unwrap();
    let order = |id: &str| list.iter().find(|k| k.id == id).unwrap().order;
    assert_eq!((order(&g2.id), order(&g1.id), order(&az.id)), (0, 1, 0));
    // A key of another service in the list is refused.
    assert_eq!(
        b.reorder("google-gemini", &[g1.id.clone(), az.id.clone()])
            .unwrap_err()
            .code,
        KEY_INVALID
    );
}

#[test]
fn a_failing_store_saves_nothing_but_session_only_keeps_the_key_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let b = book(dir.path(), Arc::new(MemoryStore::failing()));
    assert_eq!(
        add(&b, "a", SECRET).unwrap_err().code,
        KEY_STORE_UNAVAILABLE
    );
    assert!(!dir.path().join("cloud-keys.json").exists() || b.list().unwrap().is_empty());

    let key = b
        .add("google-gemini", "temp", SECRET.into(), None, true)
        .unwrap();
    assert_eq!(key.storage, Storage::Session);
    assert_eq!(b.secret(&key.id).unwrap().unwrap().expose(), SECRET);
    assert_eq!(b.list().unwrap().len(), 1);

    // After "exit" (a new book on the same folder) the session key is gone and no file has it.
    let after = book(dir.path(), Arc::new(MemoryStore::failing()));
    assert!(after.list().unwrap().is_empty());
    if let Ok(file) = std::fs::read_to_string(dir.path().join("cloud-keys.json")) {
        assert!(!file.contains(SECRET) && !file.contains("temp"));
    }
}
