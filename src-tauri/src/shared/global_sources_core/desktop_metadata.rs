use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopMetadataPaths {
    pub codex_home_identity: String,
    pub global_state_path: PathBuf,
    pub catalog_db_path: PathBuf,
    pub persisted_state_db_path: PathBuf,
}

impl DesktopMetadataPaths {
    pub(crate) fn for_codex_home(
        codex_home_identity: impl Into<String>,
        codex_home: &Path,
    ) -> Self {
        Self {
            codex_home_identity: codex_home_identity.into(),
            global_state_path: codex_home.join(".codex-global-state.json"),
            catalog_db_path: codex_home.join("sqlite").join("codex-dev.db"),
            persisted_state_db_path: codex_home.join("sqlite").join("state_5.sqlite"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopCatalogEntry {
    pub host_id: String,
    pub thread_id: String,
    pub display_title: Option<String>,
    pub cwd: Option<String>,
    pub source_kind: Option<String>,
    pub source_detail: Option<String>,
    pub observation_sequence: Option<i64>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopPersistedThread {
    pub thread_id: String,
    pub rollout_path: Option<String>,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub agent_path: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProjectMetadata {
    pub project_id: String,
    pub name: Option<String>,
    pub root_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProjectMigrationState {
    pub projects_migrated: Option<bool>,
    pub thread_assignments_migrated: Option<bool>,
    pub version: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopMetadataDiagnostic {
    pub source: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopMetadataSnapshot {
    pub codex_home_identity: String,
    pub global_state_available: bool,
    pub catalog_available: bool,
    pub persisted_state_available: bool,
    pub legacy_project_assignments_available: bool,
    pub persisted_project_id_available: bool,
    pub catalog_entries: Vec<DesktopCatalogEntry>,
    pub persisted_threads: HashMap<String, DesktopPersistedThread>,
    pub project_assignments: HashMap<String, String>,
    pub projects: HashMap<String, DesktopProjectMetadata>,
    pub project_id_mappings_by_host: HashMap<String, HashMap<String, String>>,
    pub project_migrations_by_host: HashMap<String, DesktopProjectMigrationState>,
    pub thread_writable_roots: HashMap<String, Vec<String>>,
    pub diagnostics: Vec<DesktopMetadataDiagnostic>,
}

impl DesktopMetadataSnapshot {
    pub(crate) fn contains_catalog_thread(&self, thread_id: &str) -> bool {
        self.catalog_entries
            .iter()
            .any(|entry| entry.thread_id == thread_id)
    }

    pub(crate) fn project_roots_for_thread(&self, thread_id: &str) -> Vec<String> {
        self.project_assignments
            .get(thread_id)
            .and_then(|project_id| self.projects.get(project_id))
            .map(|project| project.root_paths.clone())
            .unwrap_or_default()
    }

    pub(crate) fn from_global_state_value(
        codex_home_identity: impl Into<String>,
        value: &Value,
    ) -> Self {
        let mut snapshot = Self {
            codex_home_identity: codex_home_identity.into(),
            global_state_available: true,
            ..Self::default()
        };
        parse_assignments(value, &mut snapshot);
        parse_projects(value, &mut snapshot);
        parse_project_id_mappings(value, &mut snapshot);
        parse_project_migrations(value, &mut snapshot);
        parse_writable_roots(value, &mut snapshot);
        snapshot
    }

    fn merge_global_state(&mut self, mut other: Self) {
        self.project_assignments = std::mem::take(&mut other.project_assignments);
        self.projects = std::mem::take(&mut other.projects);
        self.legacy_project_assignments_available = other.legacy_project_assignments_available;
        self.project_id_mappings_by_host = std::mem::take(&mut other.project_id_mappings_by_host);
        self.project_migrations_by_host = std::mem::take(&mut other.project_migrations_by_host);
        self.thread_writable_roots = std::mem::take(&mut other.thread_writable_roots);
        self.diagnostics.append(&mut other.diagnostics);
    }
}

pub(crate) struct DesktopMetadataReader;

impl DesktopMetadataReader {
    pub(crate) fn read(paths: &DesktopMetadataPaths) -> DesktopMetadataSnapshot {
        let mut snapshot = DesktopMetadataSnapshot {
            codex_home_identity: paths.codex_home_identity.clone(),
            ..DesktopMetadataSnapshot::default()
        };
        match fs::read(&paths.global_state_path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) => {
                    snapshot.global_state_available = true;
                    snapshot.merge_global_state(DesktopMetadataSnapshot::from_global_state_value(
                        paths.codex_home_identity.clone(),
                        &value,
                    ));
                }
                Err(error) => snapshot.diagnostics.push(diagnostic(
                    &paths.global_state_path,
                    "malformed-json",
                    error.to_string(),
                )),
            },
            Err(error) => snapshot.diagnostics.push(diagnostic(
                &paths.global_state_path,
                if error.kind() == std::io::ErrorKind::NotFound {
                    "missing-file"
                } else {
                    "read-failed"
                },
                error.to_string(),
            )),
        }
        read_catalog(&paths.catalog_db_path, &mut snapshot);
        read_persisted_threads(&paths.persisted_state_db_path, &mut snapshot);
        snapshot
    }
}

fn parse_assignments(value: &Value, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(assignments) = value.get("thread-project-assignments") else {
        return;
    };
    let Some(assignments) = assignments.as_object() else {
        snapshot.diagnostics.push(private_schema_drift(
            ".codex-global-state.json",
            "thread-project-assignments is not an object",
        ));
        return;
    };
    let mut schema_valid = true;
    for (thread_id, assignment) in assignments {
        if let Some(project_id) = assignment.get("projectId").and_then(Value::as_str) {
            snapshot
                .project_assignments
                .insert(thread_id.clone(), project_id.to_string());
        } else {
            schema_valid = false;
            snapshot.diagnostics.push(private_schema_drift(
                ".codex-global-state.json",
                "thread project assignment projectId is missing or not a string",
            ));
        }
    }
    snapshot.legacy_project_assignments_available = schema_valid;
}

fn parse_project_id_mappings(value: &Value, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(mappings) = value.get("app-server-project-id-by-legacy-project-id-by-host") else {
        return;
    };
    let Some(mappings) = mappings.as_object() else {
        snapshot.diagnostics.push(private_schema_drift(
            ".codex-global-state.json",
            "app-server-project-id-by-legacy-project-id-by-host is not an object",
        ));
        return;
    };
    for (host_identity, host_mappings) in mappings {
        let Some(host_mappings) = host_mappings.as_object() else {
            snapshot.diagnostics.push(private_schema_drift(
                ".codex-global-state.json",
                "project ID mapping host entry is not an object",
            ));
            continue;
        };
        let mut parsed = HashMap::new();
        for (legacy_project_id, app_server_project_id) in host_mappings {
            let Some(app_server_project_id) = app_server_project_id.as_str() else {
                snapshot.diagnostics.push(private_schema_drift(
                    ".codex-global-state.json",
                    "project ID mapping value is not a string",
                ));
                continue;
            };
            parsed.insert(legacy_project_id.clone(), app_server_project_id.to_string());
        }
        snapshot
            .project_id_mappings_by_host
            .insert(host_identity.clone(), parsed);
    }
}

fn parse_project_migrations(value: &Value, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(migrations) = value.get("app-server-projects-migration-by-host") else {
        return;
    };
    let Some(migrations) = migrations.as_object() else {
        snapshot.diagnostics.push(private_schema_drift(
            ".codex-global-state.json",
            "app-server-projects-migration-by-host is not an object",
        ));
        return;
    };
    for (host_identity, migration) in migrations {
        let Some(migration) = migration.as_object() else {
            snapshot.diagnostics.push(private_schema_drift(
                ".codex-global-state.json",
                "project migration host entry is not an object",
            ));
            continue;
        };
        let projects_migrated = optional_migration_bool(migration, "projectsMigrated", snapshot);
        let thread_assignments_migrated =
            optional_migration_bool(migration, "threadAssignmentsMigrated", snapshot);
        let version = optional_migration_version(migration, snapshot);
        snapshot.project_migrations_by_host.insert(
            host_identity.clone(),
            DesktopProjectMigrationState {
                projects_migrated,
                thread_assignments_migrated,
                version,
            },
        );
    }
}

fn optional_migration_bool(
    migration: &serde_json::Map<String, Value>,
    field: &str,
    snapshot: &mut DesktopMetadataSnapshot,
) -> Option<bool> {
    match migration.get(field) {
        Some(value) => match value.as_bool() {
            Some(value) => Some(value),
            None => {
                snapshot.diagnostics.push(private_schema_drift(
                    ".codex-global-state.json",
                    &format!("project migration {field} is not a boolean"),
                ));
                None
            }
        },
        None => None,
    }
}

fn optional_migration_version(
    migration: &serde_json::Map<String, Value>,
    snapshot: &mut DesktopMetadataSnapshot,
) -> Option<i64> {
    match migration.get("version") {
        Some(value) => match value.as_i64() {
            Some(value) => Some(value),
            None => {
                snapshot.diagnostics.push(private_schema_drift(
                    ".codex-global-state.json",
                    "project migration version is not an integer",
                ));
                None
            }
        },
        None => None,
    }
}

fn parse_projects(value: &Value, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(projects) = value.get("local-projects") else {
        return;
    };
    let Some(projects) = projects.as_object() else {
        snapshot.diagnostics.push(private_schema_drift(
            ".codex-global-state.json",
            "local-projects is not an object",
        ));
        return;
    };
    for (key, project) in projects {
        let project_id = project
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(key)
            .to_string();
        let root_paths = project
            .get("rootPaths")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        snapshot.projects.insert(
            project_id.clone(),
            DesktopProjectMetadata {
                project_id,
                name: project
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                root_paths,
            },
        );
    }
}

fn parse_writable_roots(value: &Value, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(roots) = value.get("thread-writable-roots") else {
        return;
    };
    let Some(roots) = roots.as_object() else {
        snapshot.diagnostics.push(private_schema_drift(
            ".codex-global-state.json",
            "thread-writable-roots is not an object",
        ));
        return;
    };
    for (thread_id, values) in roots {
        let Some(values) = values.as_array() else {
            continue;
        };
        snapshot.thread_writable_roots.insert(
            thread_id.clone(),
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
        );
    }
}

fn read_catalog(path: &Path, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(connection) = open_read_only(path, snapshot) else {
        return;
    };
    let Some(columns) = table_columns(&connection, "local_thread_catalog", path, snapshot) else {
        return;
    };
    if !columns.contains("thread_id") {
        snapshot.diagnostics.push(private_schema_drift(
            &path.to_string_lossy(),
            "local_thread_catalog.thread_id is missing",
        ));
        return;
    }
    let sql = format!(
        "SELECT {}, thread_id, {}, {}, {}, {}, {}, {}, {} FROM local_thread_catalog",
        column_or_null(&columns, "host_id"),
        column_or_null(&columns, "display_title"),
        column_or_null(&columns, "cwd"),
        column_or_null(&columns, "source_kind"),
        column_or_null(&columns, "source_detail"),
        column_or_null(&columns, "observation_sequence"),
        column_or_null(&columns, "project_id"),
        column_or_null(&columns, "conversation_origin"),
    );
    let result = (|| -> rusqlite::Result<Vec<DesktopCatalogEntry>> {
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok(DesktopCatalogEntry {
                host_id: row
                    .get::<_, Option<String>>(0)?
                    .unwrap_or_else(|| "unknown".to_string()),
                thread_id: row.get(1)?,
                display_title: row.get(2)?,
                cwd: row.get(3)?,
                source_kind: row.get(4)?,
                source_detail: first_nonempty(
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(8)?,
                ),
                observation_sequence: row.get(6)?,
                project_id: row.get(7)?,
            })
        })?;
        rows.collect()
    })();
    match result {
        Ok(entries) => {
            snapshot.catalog_available = true;
            snapshot.catalog_entries = entries;
        }
        Err(error) => {
            snapshot
                .diagnostics
                .push(diagnostic(path, "catalog-query-failed", error.to_string()))
        }
    }
}

fn read_persisted_threads(path: &Path, snapshot: &mut DesktopMetadataSnapshot) {
    let Some(connection) = open_read_only(path, snapshot) else {
        return;
    };
    let Some(columns) = table_columns(&connection, "threads", path, snapshot) else {
        return;
    };
    if !columns.contains("id") {
        snapshot.diagnostics.push(private_schema_drift(
            &path.to_string_lossy(),
            "threads.id is missing",
        ));
        return;
    }
    snapshot.persisted_project_id_available = columns.contains("project_id");
    if !snapshot.persisted_project_id_available {
        snapshot.diagnostics.push(private_schema_drift(
            &path.to_string_lossy(),
            "threads.project_id is missing",
        ));
    }
    let sql = format!(
        "SELECT id, {}, {}, {}, {}, {}, {} FROM threads",
        column_or_null(&columns, "rollout_path"),
        column_or_null(&columns, "cwd"),
        column_or_null(&columns, "source"),
        column_or_null(&columns, "model"),
        column_or_null(&columns, "agent_path"),
        column_or_null(&columns, "project_id"),
    );
    let result = (|| -> rusqlite::Result<HashMap<String, DesktopPersistedThread>> {
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            let thread_id: String = row.get(0)?;
            Ok(DesktopPersistedThread {
                thread_id,
                rollout_path: row.get(1)?,
                cwd: row.get(2)?,
                source: row.get(3)?,
                model: row.get(4)?,
                agent_path: row.get(5)?,
                project_id: row.get(6)?,
            })
        })?;
        let mut values = HashMap::new();
        for row in rows {
            let row = row?;
            values.insert(row.thread_id.clone(), row);
        }
        Ok(values)
    })();
    match result {
        Ok(threads) => {
            snapshot.persisted_state_available = true;
            snapshot.persisted_threads = threads;
        }
        Err(error) => snapshot.diagnostics.push(diagnostic(
            path,
            "persisted-query-failed",
            error.to_string(),
        )),
    }
}

fn open_read_only(path: &Path, snapshot: &mut DesktopMetadataSnapshot) -> Option<Connection> {
    if !path.exists() {
        snapshot.diagnostics.push(diagnostic(
            path,
            "missing-file",
            "optional Desktop metadata database is absent",
        ));
        return None;
    }
    match Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(connection) => Some(connection),
        Err(error) => {
            snapshot.diagnostics.push(diagnostic(
                path,
                "sqlite-open-read-only-failed",
                error.to_string(),
            ));
            None
        }
    }
}

fn table_columns(
    connection: &Connection,
    table: &str,
    path: &Path,
    snapshot: &mut DesktopMetadataSnapshot,
) -> Option<HashSet<String>> {
    let sql = format!("PRAGMA table_info({table})");
    let result = (|| -> rusqlite::Result<HashSet<String>> {
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
        rows.collect()
    })();
    match result {
        Ok(columns) if columns.is_empty() => {
            snapshot.diagnostics.push(private_schema_drift(
                &path.to_string_lossy(),
                &format!("table {table} is missing"),
            ));
            None
        }
        Ok(columns) => Some(columns),
        Err(error) => {
            snapshot.diagnostics.push(diagnostic(
                path,
                "sqlite-schema-read-failed",
                error.to_string(),
            ));
            None
        }
    }
}

fn column_or_null(columns: &HashSet<String>, column: &str) -> String {
    if columns.contains(column) {
        format!("\"{column}\"")
    } else {
        "NULL".to_string()
    }
}

fn first_nonempty(first: Option<String>, second: Option<String>) -> Option<String> {
    first.filter(|value| !value.is_empty()).or(second)
}

fn diagnostic(path: &Path, code: &str, message: impl Into<String>) -> DesktopMetadataDiagnostic {
    DesktopMetadataDiagnostic {
        source: path.to_string_lossy().to_string(),
        code: code.to_string(),
        message: message.into(),
    }
}

fn private_schema_drift(source: &str, message: &str) -> DesktopMetadataDiagnostic {
    DesktopMetadataDiagnostic {
        source: source.to_string(),
        code: "private-schema-drift".to_string(),
        message: message.to_string(),
    }
}
