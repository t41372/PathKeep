//! Additional migrations of the registered read-model database. No source files or evidence tables are involved.
pub(crate) const TABLES: &[&str] = &[
    "CREATE TABLE ah_reader_version(token TEXT PRIMARY KEY, attempt INTEGER NOT NULL,
                 source TEXT NOT NULL,revision INTEGER NOT NULL,digest TEXT NOT NULL,byte_len
                 INTEGER NOT NULL, codec INTEGER NOT NULL,renderer INTEGER NOT NULL,records
                 INTEGER NOT NULL,selective INTEGER NOT NULL,state TEXT NOT NULL, record_limit
                 INTEGER NOT NULL,decoded_limit INTEGER NOT NULL,max_frame INTEGER NOT NULL
                 DEFAULT 0,format_version TEXT)",
    "CREATE INDEX ah_reader_attempt ON ah_reader_version(attempt,state)",
    "CREATE INDEX ah_reader_state ON ah_reader_version(state,attempt)",
    "CREATE TABLE ah_reader_page(owner TEXT NOT NULL,family INTEGER NOT
                 NULL,first_rank INTEGER NOT NULL, part INTEGER NOT NULL,nbytes INTEGER NOT
                 NULL,hash BLOB NOT NULL,binding BLOB NOT NULL, PRIMARY
                 KEY(owner,family,first_rank,part))",
    "CREATE INDEX ah_reader_page_hash ON ah_reader_page(hash)",
    "CREATE TABLE ah_reader_blob(hash BLOB PRIMARY KEY, data BLOB NOT NULL)",
    "CREATE TABLE ah_reader_sort(owner TEXT NOT NULL,level INTEGER NOT NULL,run
                 INTEGER NOT NULL,page INTEGER NOT NULL,data BLOB NOT NULL, PRIMARY
                 KEY(owner,level,run,page))",
    "CREATE TABLE ah_reader_lookup(owner TEXT NOT NULL,key TEXT NOT NULL,ordinal
                 INTEGER NOT NULL,page INTEGER NOT NULL, PRIMARY KEY(owner,key,ordinal))",
    "CREATE TABLE ah_reader_rank(owner TEXT NOT NULL,first_rank INTEGER NOT
                 NULL,visible_before INTEGER NOT NULL,mask BLOB NOT NULL,PRIMARY
                 KEY(owner,first_rank))",
    "CREATE UNIQUE INDEX ah_reader_visible_seek ON ah_reader_rank(owner,visible_before)",
    "CREATE TABLE ah_reader_leaf(owner TEXT PRIMARY KEY,total INTEGER NOT NULL,latest INTEGER,selected_visible INTEGER)",
    "CREATE TABLE ah_reader_leaf_line(owner TEXT NOT NULL,reverse_rank INTEGER NOT NULL,visible_start INTEGER NOT NULL,count INTEGER NOT NULL,PRIMARY KEY(owner,reverse_rank))",
    "CREATE TABLE ah_reader_head(conversation TEXT PRIMARY KEY,publication TEXT NOT
                 NULL UNIQUE,current_source TEXT NOT NULL,input_digest TEXT NOT NULL)",
    "CREATE TABLE ah_reader_member(publication TEXT NOT NULL,source TEXT NOT
                 NULL,version TEXT NOT NULL, descriptor TEXT NOT NULL,records INTEGER NOT
                 NULL,complete INTEGER NOT NULL,attachments INTEGER NOT NULL, reason TEXT,visible
                 INTEGER NOT NULL, PRIMARY KEY(publication,source))",
    "CREATE INDEX ah_reader_member_source ON ah_reader_member(source,publication)",
    "CREATE INDEX ah_reader_member_version ON ah_reader_member(version)",
    "CREATE TABLE ah_reader_line(publication TEXT NOT NULL,branch TEXT NOT
                 NULL,first_rank INTEGER NOT NULL, version TEXT NOT NULL,source TEXT NOT
                 NULL,records INTEGER NOT NULL,first_visible INTEGER NOT NULL,visible INTEGER NOT
                 NULL, PRIMARY KEY(publication,branch,first_rank))",
    "CREATE INDEX ah_reader_line_visible ON
                 ah_reader_line(publication,branch,first_visible)",
    "CREATE INDEX ah_reader_line_version ON ah_reader_line(version)",
    "CREATE TABLE ah_reader_dependency(target TEXT NOT NULL,publication TEXT NOT
                 NULL,PRIMARY KEY(target,publication))",
    "CREATE INDEX ah_reader_dependency_owner ON ah_reader_dependency(publication)",
    "CREATE TABLE ah_reader_edge(publication TEXT NOT NULL,child TEXT NOT
                 NULL,PRIMARY KEY(publication,child))",
    "CREATE TABLE ah_reader_dirty(conversation TEXT PRIMARY KEY,ticket TEXT NOT NULL)",
    "CREATE TABLE ah_reader_build(publication TEXT PRIMARY KEY,conversation TEXT NOT NULL UNIQUE,
        ticket TEXT NOT NULL,current_source TEXT,input_digest TEXT)",
    "CREATE TABLE ah_reader_retired(publication TEXT PRIMARY KEY)",
];
