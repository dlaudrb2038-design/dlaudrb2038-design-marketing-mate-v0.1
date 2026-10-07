//! Rust owns the database. No SQL or database path is accepted from the webview.
//! Migration safety copies are local implementation artifacts, not the encrypted
//! user backup/export feature scheduled for a later development phase.

use rusqlite::{
    backup::{Backup, StepResult},
    params, Connection, OpenFlags,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: include_str!("../migrations/0001_personal_profile.sql"),
}];

const WORK_HOURS_JSON: &str =
    r#"{"weekdays":[1,2,3,4,5],"start":"09:00","end":"18:00","timezone":"Asia/Seoul"}"#;

struct Migration {
    version: i64,
    sql: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PersonalProfile {
    pub profile_id: String,
    pub user_name: String,
    pub tone: String,
    pub report_style: String,
    pub preferred_kpis_json: String,
    pub work_hours_json: String,
    pub settings_json: String,
}

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Corrupt,
    SchemaTooNew,
    MigrationChecksumMismatch,
    InvalidMigrationHistory,
    UnversionedDatabase,
    ProfileInvariant,
    InvalidDatabasePath,
    CheckpointBusy,
    BackupBusy,
}

impl StorageError {
    /// Stable, non-sensitive error codes suitable for the diagnostic settings UI.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Corrupt => "DB_CORRUPT",
            Self::Sqlite(rusqlite::Error::SqliteFailure(error, _))
                if matches!(
                    error.code,
                    rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
                ) =>
            {
                "DB_CORRUPT"
            }
            Self::SchemaTooNew => "DB_SCHEMA_TOO_NEW",
            Self::MigrationChecksumMismatch => "DB_MIGRATION_CHECKSUM_MISMATCH",
            Self::ProfileInvariant => "DB_PROFILE_INVALID",
            Self::CheckpointBusy | Self::BackupBusy => "DB_BUSY",
            Self::Io(_) | Self::InvalidDatabasePath => "DB_IO_FAILED",
            _ => "DB_MIGRATION_FAILED",
        }
    }
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep underlying SQLite/OS details and paths out of IPC and logs.
        write!(formatter, "{}", self.code())
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct Storage {
    connection: Connection,
    _migration_backup: Option<PathBuf>,
}

impl Storage {
    /// `path` must come from Tauri's app_data_dir, never from IPC arguments.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::open_with_migrations(path, MIGRATIONS)
    }

    fn open_with_migrations(path: &Path, migrations: &[Migration]) -> Result<Self, StorageError> {
        validate_migration_definitions(migrations)?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or(StorageError::InvalidDatabasePath)?;
        if path.file_name().is_none() || path.is_dir() {
            return Err(StorageError::InvalidDatabasePath);
        }

        // Reject unsupported/corrupt existing files before any write-mode open,
        // journal-mode change, migration, or attempt to create a fresh profile.
        let migration_backup = if path.try_exists()? {
            let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            source.busy_timeout(Duration::from_secs(5))?;
            check_integrity(&source)?;
            let applied = validate_history(&source, migrations)?;
            if applied < migrations.len() {
                Some(create_migration_backup(
                    &source,
                    parent,
                    applied,
                    migrations.len(),
                )?)
            } else {
                None
            }
        } else {
            fs::create_dir_all(parent)?;
            None
        };

        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let journal_mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(StorageError::InvalidDatabasePath);
        }
        apply_migrations(&mut connection, migrations)?;
        let storage = Self {
            connection,
            _migration_backup: migration_backup,
        };
        storage.profile()?;
        Ok(storage)
    }

    pub fn profile(&self) -> Result<PersonalProfile, StorageError> {
        let count: i64 =
            self.connection
                .query_row("SELECT count(*) FROM personal_profile", [], |row| {
                    row.get(0)
                })?;
        if count != 1 {
            return Err(StorageError::ProfileInvariant);
        }
        Ok(self.connection.query_row(
            "SELECT profile_id, user_name, tone, report_style, preferred_kpis_json, work_hours_json, settings_json
             FROM personal_profile WHERE singleton = 1",
            [],
            |row| Ok(PersonalProfile {
                profile_id: row.get(0)?,
                user_name: row.get(1)?,
                tone: row.get(2)?,
                report_style: row.get(3)?,
                preferred_kpis_json: row.get(4)?,
                work_hours_json: row.get(5)?,
                settings_json: row.get(6)?,
            }),
        )?)
    }

    #[cfg(test)]
    pub fn schema_version(&self) -> i64 {
        MIGRATIONS.last().map_or(0, |migration| migration.version)
    }

    /// Intended for Rust diagnostics/tests; never return local paths over IPC.
    #[cfg(test)]
    pub fn migration_backup_path(&self) -> Option<&Path> {
        self._migration_backup.as_deref()
    }

    pub fn checkpoint(&self) -> Result<(), StorageError> {
        let busy: i64 =
            self.connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if busy != 0 {
            return Err(StorageError::CheckpointBusy);
        }
        Ok(())
    }

    pub fn close(self) -> Result<(), StorageError> {
        self.checkpoint()?;
        self.connection
            .close()
            .map_err(|(_, error)| StorageError::Sqlite(error))
    }
}

fn checksum(sql: &str) -> String {
    // A Windows CRLF checkout and an LF checkout must describe one migration.
    let normalized = sql.replace("\r\n", "\n");
    Sha256::digest(normalized.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_migration_definitions(migrations: &[Migration]) -> Result<(), StorageError> {
    if migrations.is_empty()
        || migrations
            .iter()
            .enumerate()
            .any(|(index, migration)| migration.version != index as i64 + 1)
    {
        return Err(StorageError::InvalidMigrationHistory);
    }
    Ok(())
}

fn check_integrity(connection: &Connection) -> Result<(), StorageError> {
    let mut statement = connection.prepare("PRAGMA quick_check")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    for result in rows {
        if result? != "ok" {
            return Err(StorageError::Corrupt);
        }
    }
    Ok(())
}

fn validate_history(
    connection: &Connection,
    migrations: &[Migration],
) -> Result<usize, StorageError> {
    let has_ledger: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations')",
        [], |row| row.get(0),
    )?;
    if !has_ledger {
        let other_tables: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        return if other_tables == 0 {
            Ok(0)
        } else {
            Err(StorageError::UnversionedDatabase)
        };
    }

    let mut statement =
        connection.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let history = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if history
        .iter()
        .any(|(version, _)| *version > migrations.last().map_or(0, |migration| migration.version))
    {
        return Err(StorageError::SchemaTooNew);
    }
    for (index, (version, recorded_checksum)) in history.iter().enumerate() {
        if *version != index as i64 + 1 {
            return Err(StorageError::InvalidMigrationHistory);
        }
        let migration = migrations
            .get(index)
            .ok_or(StorageError::InvalidMigrationHistory)?;
        if recorded_checksum != &checksum(migration.sql) {
            return Err(StorageError::MigrationChecksumMismatch);
        }
    }
    // An empty ledger alongside a user schema cannot be treated as a fresh DB.
    if history.is_empty() {
        return Err(StorageError::InvalidMigrationHistory);
    }
    Ok(history.len())
}

fn create_migration_backup(
    connection: &Connection,
    parent: &Path,
    current: usize,
    target: usize,
) -> Result<PathBuf, StorageError> {
    let directory = parent.join("migration-backups");
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "pre-migration-v{current}-to-v{target}-{}.sqlite3",
        Uuid::new_v4()
    ));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    drop(file);
    let outcome = (|| -> Result<(), StorageError> {
        let mut destination = Connection::open(&path)?;
        {
            let backup = Backup::new(connection, &mut destination)?;
            let mut blocked_since = None;
            loop {
                match backup.step(128)? {
                    StepResult::Done => break,
                    StepResult::More => blocked_since = None,
                    // Bound consecutive lock waits; run_to_completion otherwise
                    // retries forever and could hang application startup.
                    _ => {
                        let started = blocked_since.get_or_insert_with(Instant::now);
                        if started.elapsed() >= Duration::from_secs(5) {
                            return Err(StorageError::BackupBusy);
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
        check_integrity(&destination)?;
        destination
            .close()
            .map_err(|(_, error)| StorageError::Sqlite(error))?;
        Ok(())
    })();
    if let Err(error) = outcome {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

fn apply_migrations(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<(), StorageError> {
    let applied = validate_history(connection, migrations)?;
    if applied == migrations.len() {
        return Ok(());
    }
    // Each startup's pending migrations and initial profile are one atomic unit.
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    for migration in &migrations[applied..] {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, applied_at, checksum) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?2)",
            params![migration.version, checksum(migration.sql)],
        )?;
    }
    if applied == 0 {
        transaction.execute(
            "INSERT INTO personal_profile
             (profile_id, user_name, tone, report_style, preferred_kpis_json, work_hours_json, settings_json)
             VALUES (?1, ?2, ?3, '', '[]', ?4, '{}')",
            params![Uuid::new_v4().to_string(), "명규", "간결하고 친근한 존댓말", WORK_HOURS_JSON],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDatabase {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TestDatabase {
        fn new() -> Self {
            let directory = std::env::temp_dir()
                .join(format!("marketing-mate-storage-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&directory).unwrap();
            let path = directory.join("marketing-mate.sqlite3");
            Self { directory, path }
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn error_code(result: Result<Storage, StorageError>) -> &'static str {
        match result {
            Ok(_) => panic!("opening the database should have failed"),
            Err(error) => error.code(),
        }
    }

    #[test]
    fn first_launch_and_relaunch_preserve_one_profile_and_defaults() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        let first = storage.profile().unwrap();
        assert_eq!(
            Uuid::parse_str(&first.profile_id)
                .unwrap()
                .get_version_num(),
            4
        );
        assert_eq!(first.user_name, "명규");
        assert_eq!(first.tone, "간결하고 친근한 존댓말");
        assert_eq!(first.report_style, "");
        assert_eq!(first.preferred_kpis_json, "[]");
        assert_eq!(first.settings_json, "{}");
        assert_eq!(first.work_hours_json, WORK_HOURS_JSON);
        assert_eq!(storage.schema_version(), 1);
        assert!(storage.migration_backup_path().is_none());
        storage.close().unwrap();

        let reopened = Storage::open(&database.path).unwrap();
        assert_eq!(reopened.profile().unwrap().profile_id, first.profile_id);
        assert!(reopened.migration_backup_path().is_none());
        reopened.close().unwrap();
    }

    #[test]
    fn profile_singleton_and_json_contracts_are_enforced_by_sqlite() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        assert!(storage.connection.execute(
            "INSERT INTO personal_profile SELECT ?1, singleton, user_name, tone, report_style, preferred_kpis_json, work_hours_json, settings_json FROM personal_profile",
            [Uuid::new_v4().to_string()],
        ).is_err());
        assert!(storage
            .connection
            .execute("UPDATE personal_profile SET singleton = 2", [])
            .is_err());
        assert!(storage
            .connection
            .execute("UPDATE personal_profile SET preferred_kpis_json = '{}'", [])
            .is_err());
        assert!(storage
            .connection
            .execute(
                "UPDATE personal_profile SET settings_json = 'invalid-json'",
                []
            )
            .is_err());
        assert_eq!(storage.profile().unwrap().settings_json, "{}");
        storage.close().unwrap();
    }

    #[test]
    fn required_connection_pragmas_are_enabled() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        let foreign_keys: i64 = storage
            .connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        let journal_mode: String = storage
            .connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let timeout: i64 = storage
            .connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(timeout, 5000);
        storage.close().unwrap();
    }

    #[test]
    fn checksum_is_stable_across_windows_and_unix_line_endings() {
        assert_eq!(
            checksum(MIGRATIONS[0].sql),
            checksum(&MIGRATIONS[0].sql.replace('\n', "\r\n"))
        );
    }

    #[test]
    fn missing_profile_is_reported_without_silent_recreation() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        storage
            .connection
            .execute("DELETE FROM personal_profile", [])
            .unwrap();
        storage.close().unwrap();
        assert_eq!(
            error_code(Storage::open(&database.path)),
            "DB_PROFILE_INVALID"
        );
        let connection =
            Connection::open_with_flags(&database.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let count: i64 = connection
            .query_row("SELECT count(*) FROM personal_profile", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn future_schema_and_modified_migration_refuse_without_mutating_file() {
        for (sql, expected) in [
            ("INSERT INTO schema_migrations VALUES (2, 'future', '0000000000000000000000000000000000000000000000000000000000000000')", "DB_SCHEMA_TOO_NEW"),
            ("UPDATE schema_migrations SET checksum = '0000000000000000000000000000000000000000000000000000000000000000'", "DB_MIGRATION_CHECKSUM_MISMATCH"),
        ] {
            let database = TestDatabase::new();
            let storage = Storage::open(&database.path).unwrap();
            storage.connection.execute_batch(sql).unwrap();
            storage.close().unwrap();
            let before = fs::read(&database.path).unwrap();
            assert_eq!(error_code(Storage::open(&database.path)), expected);
            assert_eq!(fs::read(&database.path).unwrap(), before);
            assert!(!database.directory.join("migration-backups").exists());
        }
    }

    #[test]
    fn corrupt_file_is_preserved_and_reported() {
        let database = TestDatabase::new();
        let original = b"This is not a SQLite database; do not replace it.";
        fs::write(&database.path, original).unwrap();
        assert_eq!(error_code(Storage::open(&database.path)), "DB_CORRUPT");
        assert_eq!(fs::read(&database.path).unwrap(), original);
        assert!(!database.directory.join("migration-backups").exists());
    }

    #[test]
    fn pre_migration_copy_captures_wal_data_and_failed_upgrade_rolls_back() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        // Leave data in WAL with the existing connection open. A plain file copy
        // could miss it; SQLite's backup API must include the committed snapshot.
        storage
            .connection
            .execute("UPDATE personal_profile SET user_name = 'local test'", [])
            .unwrap();
        let original_profile_id = storage.profile().unwrap().profile_id;
        let failed_migrations = [
            Migration { version: 1, sql: MIGRATIONS[0].sql },
            Migration { version: 2, sql: "CREATE TABLE test_rollback(value TEXT); INSERT INTO table_that_does_not_exist VALUES (1);" },
        ];
        assert_eq!(
            error_code(Storage::open_with_migrations(
                &database.path,
                &failed_migrations
            )),
            "DB_MIGRATION_FAILED"
        );

        let backups = fs::read_dir(database.directory.join("migration-backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        let backup =
            Connection::open_with_flags(&backups[0], OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let backup_profile: (String, String) = backup
            .query_row(
                "SELECT profile_id, user_name FROM personal_profile",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            backup_profile,
            (original_profile_id.clone(), "local test".into())
        );
        assert_eq!(validate_history(&backup, MIGRATIONS).unwrap(), 1);
        drop(backup);

        let upgraded_table_exists: bool = storage
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'test_rollback')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!upgraded_table_exists);
        assert_eq!(storage.profile().unwrap().profile_id, original_profile_id);
        assert_eq!(storage.profile().unwrap().user_name, "local test");
        assert_eq!(
            validate_history(&storage.connection, MIGRATIONS).unwrap(),
            1
        );
        storage.close().unwrap();
        Storage::open(&database.path).unwrap().close().unwrap();
    }

    #[test]
    fn successful_upgrade_preserves_profile_and_records_checksum() {
        let database = TestDatabase::new();
        let storage = Storage::open(&database.path).unwrap();
        let profile_id = storage.profile().unwrap().profile_id;
        storage.close().unwrap();
        let migrations = [
            Migration {
                version: 1,
                sql: MIGRATIONS[0].sql,
            },
            Migration {
                version: 2,
                sql: "CREATE TABLE test_upgrade(value TEXT);",
            },
        ];
        let upgraded = Storage::open_with_migrations(&database.path, &migrations).unwrap();
        assert_eq!(upgraded.profile().unwrap().profile_id, profile_id);
        assert!(upgraded.migration_backup_path().unwrap().is_file());
        assert_eq!(
            validate_history(&upgraded.connection, &migrations).unwrap(),
            2
        );
        upgraded.close().unwrap();
        assert_eq!(
            error_code(Storage::open(&database.path)),
            "DB_SCHEMA_TOO_NEW"
        );
    }

    #[test]
    fn unversioned_user_database_is_not_adopted_or_overwritten() {
        let database = TestDatabase::new();
        let connection = Connection::open(&database.path).unwrap();
        connection.execute_batch("CREATE TABLE existing_data(value TEXT); INSERT INTO existing_data VALUES ('preserve');").unwrap();
        connection.close().unwrap();
        let original = fs::read(&database.path).unwrap();
        assert_eq!(
            error_code(Storage::open(&database.path)),
            "DB_MIGRATION_FAILED"
        );
        assert_eq!(fs::read(&database.path).unwrap(), original);
    }
}
