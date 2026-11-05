//! The key store trait, tested on the in-memory store: set, get, delete, and a store that fails.

use flickfluent_core::cloud::keystore::{KeyStore, MemoryStore};
use flickfluent_core::cloud::secret::Secret;
use flickfluent_core::error::KEY_STORE_UNAVAILABLE;

fn secret(v: &str) -> Secret {
    Secret::new(v.to_string())
}

#[test]
fn set_get_and_delete() {
    let store = MemoryStore::new();
    store.set("a", &secret("one")).unwrap();
    assert_eq!(store.get("a").unwrap().unwrap().expose(), "one");
    store.set("a", &secret("two")).unwrap();
    assert_eq!(store.get("a").unwrap().unwrap().expose(), "two");
    store.delete("a").unwrap();
    assert!(store.get("a").unwrap().is_none());
}

#[test]
fn deleting_a_missing_entry_is_ok_and_an_unset_entry_reads_as_missing() {
    let store = MemoryStore::new();
    assert!(store.get("never").unwrap().is_none());
    store.delete("never").unwrap();
}

#[test]
fn a_store_that_always_fails_says_key_store_unavailable() {
    let store = MemoryStore::failing();
    assert_eq!(
        store.set("a", &secret("x")).unwrap_err().code,
        KEY_STORE_UNAVAILABLE
    );
    assert_eq!(store.get("a").unwrap_err().code, KEY_STORE_UNAVAILABLE);
    assert_eq!(store.delete("a").unwrap_err().code, KEY_STORE_UNAVAILABLE);
}
