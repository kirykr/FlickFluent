mod support;

use flickfluent_core::models::{
    bundled_catalogue, delete_model, installed, part_bytes, record_installed, sha256_file,
    verify_file, Control, Outcome,
};
use support::{body, entry, serve, Behavior};

#[test]
fn the_bundled_catalogue_is_valid() {
    let list = bundled_catalogue();
    assert!(!list.is_empty());
    let mut ids = std::collections::HashSet::new();
    for e in &list {
        assert!(ids.insert(e.id.clone()), "duplicate id {}", e.id);
        assert!(e.size_bytes > 0);
        // Every downloadable piece, whether the entry is one file or several, has a hash and https.
        for piece in e.pieces() {
            assert_eq!(piece.sha256.len(), 64, "{} needs a SHA-256", e.id);
            assert!(piece.url.starts_with("https://"), "{} must use https", e.id);
        }
        assert_eq!(
            e.size_bytes,
            e.pieces().iter().map(|p| p.size_bytes).sum::<u64>(),
            "{} size is the sum",
            e.id
        );
        assert!(!e.licence.is_empty() && !e.purpose.is_empty() && !e.name.is_empty());
    }
}

#[test]
fn sha256_of_a_file_matches_the_known_value() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("x");
    std::fs::write(&f, b"abc").unwrap();
    let expect = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    assert_eq!(sha256_file(&f).unwrap(), expect);
    assert!(verify_file(&f, expect).is_ok());
    assert!(
        verify_file(&f, &expect.to_uppercase()).is_ok(),
        "case does not matter"
    );
    assert_eq!(
        verify_file(&f, &"0".repeat(64)).unwrap_err().code,
        "HASH_MISMATCH"
    );
}

#[tokio::test]
async fn installed_models_are_listed_with_their_size_and_can_be_deleted() {
    let data = body(30_000);
    let server = serve(Behavior::new(data.clone()));
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);
    assert!(installed(dir.path()).is_empty());

    let Outcome::Completed(path) =
        flickfluent_core::models::download(&e, dir.path(), &Control::new(), &mut |_| {})
            .await
            .unwrap()
    else {
        panic!("expected completed")
    };
    record_installed(dir.path(), &e, &path).unwrap();

    let list = installed(dir.path());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, e.id);
    assert_eq!(list[0].size_bytes, 30_000);

    delete_model(dir.path(), &e).unwrap();
    assert!(installed(dir.path()).is_empty());
    assert!(!path.exists());
}

#[tokio::test]
async fn part_bytes_tells_a_paused_download_how_far_it_got() {
    let data = body(100_000);
    let mut b = Behavior::new(data.clone());
    b.cut_first_after = Some(20_000);
    let server = serve(b);
    let dir = tempfile::tempdir().unwrap();
    let e = entry(&server.url, &data);
    assert_eq!(part_bytes(dir.path(), &e), 0);

    let _ = flickfluent_core::models::download(&e, dir.path(), &Control::new(), &mut |_| {}).await;
    assert!(part_bytes(dir.path(), &e) > 0);
}

#[test]
fn a_damaged_index_file_means_nothing_is_installed_not_a_crash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("index.json"), b"{{{").unwrap();
    assert!(installed(dir.path()).is_empty());
}

/// Needs internet, so it is skipped by default. Run it with:
/// `cargo test -p flickfluent-core --test models_storage -- --ignored`
#[tokio::test]
#[ignore = "needs internet"]
async fn the_real_self_test_download_works_and_matches_its_hash() {
    let entry = bundled_catalogue()
        .into_iter()
        .find(|e| e.id == "download-self-test")
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let Outcome::Completed(path) =
        flickfluent_core::models::download(&entry, dir.path(), &Control::new(), &mut |_| {})
            .await
            .unwrap()
    else {
        panic!("expected completed")
    };
    assert_eq!(std::fs::metadata(path).unwrap().len(), entry.size_bytes);
}
