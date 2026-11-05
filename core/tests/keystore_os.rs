//! Store, read and delete one real entry in the operating system's secure storage.
//! Ignored by default: it touches the real Keychain or Credential Manager (macOS may ask to
//! allow access). Run by hand on each OS:
//! `cargo test -p flickfluent-core --test keystore_os -- --ignored`
//! and record the result in `docs/verification/003.md`.

use flickfluent_core::cloud::keystore::{KeyStore, OsKeyStore};
use flickfluent_core::cloud::secret::Secret;

#[test]
#[ignore = "uses the real OS secure storage"]
fn store_read_and_delete_one_real_entry() {
    let store = OsKeyStore::install().expect("the OS secure storage opens");
    let id = format!("test-{}", uuid::Uuid::new_v4());
    assert!(store.get(&id).unwrap().is_none());
    store
        .set(&id, &Secret::new("flickfluent-test-secret".into()))
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().unwrap().expose(),
        "flickfluent-test-secret"
    );
    store.delete(&id).unwrap();
    assert!(store.get(&id).unwrap().is_none());
    store.delete(&id).unwrap();
}
