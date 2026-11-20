//! Canonical folder identities and user choices belong to E. R owns the disposable folder
//! directory and the project seek keys on the conversation summaries.
pub(crate) const EVIDENCE: &[&str] = &[
    "CREATE TABLE ah_project_identity(path TEXT PRIMARY KEY,id TEXT NOT NULL UNIQUE)",
    "CREATE TABLE ah_project_choices(singleton INTEGER PRIMARY KEY,version INTEGER NOT NULL,choices TEXT NOT NULL,previous TEXT)",
    "INSERT INTO ah_project_choices VALUES(1,0,'{}',NULL)",
];
pub(crate) const MODEL: &[&str] = &[
    "CREATE TABLE ah_project_paths(path TEXT PRIMARY KEY)",
    "INSERT OR IGNORE INTO ah_project_paths SELECT folder FROM ah_chat_summary WHERE folder IS NOT NULL AND folder<>''",
    "CREATE TABLE ah_project_folder(path TEXT PRIMARY KEY,project TEXT NOT NULL,label TEXT NOT NULL,kind TEXT NOT NULL)",
    "CREATE INDEX ah_project_locations ON ah_project_folder(project,path)",
    "ALTER TABLE ah_chat_summary ADD COLUMN project_id TEXT",
    "CREATE INDEX ah_project_folder_update ON ah_chat_summary(folder)",
    "CREATE INDEX ah_project_recent ON ah_chat_summary(live,root,project_id,recent_key)",
    "INSERT INTO ah_meta(key,value) VALUES('project_epoch','0')",
    "CREATE TABLE ah_project_dirty(path TEXT PRIMARY KEY)",
    "INSERT INTO ah_project_dirty SELECT path FROM ah_project_paths",
    "CREATE TABLE ah_project_rollup(path TEXT NOT NULL,harness TEXT NOT NULL,roots INTEGER NOT NULL,children INTEGER NOT NULL,ms INTEGER,tokens INTEGER,known INTEGER NOT NULL,first_at INTEGER,last_at INTEGER,latest TEXT NOT NULL,title TEXT,PRIMARY KEY(path,harness))",
];
