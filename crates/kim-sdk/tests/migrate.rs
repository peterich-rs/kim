#![allow(clippy::unwrap_used)]

#[test]
fn production_migrate_does_not_import_messages_into_outbox() {
    let src = include_str!("../src/store/migrate.rs");
    assert!(
        !src.contains("INSERT OR IGNORE INTO outbox"),
        "production migrate must not copy Dart pending rows into outbox"
    );
    assert!(
        !src.contains("WHERE m.status IN ('sending', 'failed')"),
        "messages→outbox importer must stay deleted"
    );
}
