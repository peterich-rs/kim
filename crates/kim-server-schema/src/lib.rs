//! Historical DDL. Checksums match the files previously embedded by Chat.
//! Online Chat and Royal processes must not link this crate.

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub fn versions() -> Vec<i64> {
    MIGRATOR.iter().map(|migration| migration.version).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_0001_through_0016() {
        assert_eq!(versions(), (1..=16).collect::<Vec<_>>());
    }
}
