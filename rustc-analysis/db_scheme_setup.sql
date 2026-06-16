-- This file is not used by the rustc-analysis crate
-- The actual SQL command used is hardcoded in the binary
-- This just servers as an easy way to see how the DB-file is defined

-- TODO: SHOULD paste actually used scheme here when finished
-- OLD


CREATE TABLE FieldList (
    field VARCHAR PRIMARY KEY
);

CREATE TABLE ProblemTree (
    problem VARCHAR PRIMARY KEY,
    parent VARCHAR,

    FOREIGN KEY (parent) REFERENCES ProblemTree(problem)
);

CREATE TABLE DefIdKinds (
    id UINT32 PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE
);

CREATE SEQUENCE repo_id_seq;

CREATE TABLE Repos (
    id UINT32 PRIMARY KEY DEFAULT nextval('repo_id_seq'),
    repo_url VARCHAR NOT NULL UNIQUE,
    commit_hash VARCHAR,
    cargo_args VARCHAR,
    analyzed BOOLEAN DEFAULT FALSE
);

CREATE TABLE MergedCrates (
    id UINT32 PRIMARY KEY,
    name VARCHAR NOT NULL,
    src_url VARCHAR,
    field VARCHAR,

    FOREIGN KEY (field) REFERENCES FieldList(field)
);

CREATE TABLE Crates (
    stable_crate_id UINT64 PRIMARY KEY,
    name VARCHAR NOT NULL,
    version VARCHAR NOT NULL,
    internal BOOLEAN NOT NULL,
    path_url VARCHAR NOT NULL,
    merged_crate_id UINT32,

    FOREIGN KEY (merged_crate_id) REFERENCES MergedCrates(id)
);

CREATE TABLE DefIds (
    stable_crate_id UINT64 NOT NULL,
    local_hash UINT64 NOT NULL,
    src_repo UINT32 NOT NULL,
    def_path_str VARCHAR NOT NULL,
    kind UINT32 NOT NULL,
    unsafe BOOLEAN NOT NULL,
    
    PRIMARY KEY (stable_crate_id, local_hash),
    FOREIGN KEY (stable_crate_id) REFERENCES Crates(stable_crate_id),
    FOREIGN KEY (src_repo) REFERENCES Repos(id),
    FOREIGN KEY (kind) REFERENCES DefIdKinds(id)
);

CREATE TABLE Dependencies (
    from_stable_crate_id UINT64 NOT NULL,
    from_local_hash UINT64 NOT NULL,
    to_stable_crate_id UINT64 NOT NULL,
    to_local_hash UINT64 NOT NULL,

    PRIMARY KEY (from_stable_crate_id, from_local_hash, to_stable_crate_id, to_local_hash),
    FOREIGN KEY (from_stable_crate_id, from_local_hash) REFERENCES DefIds(stable_crate_id, local_hash),
    FOREIGN KEY (to_stable_crate_id, to_local_hash) REFERENCES DefIds(stable_crate_id, local_hash)
);

CREATE TABLE ManAnalysisResults (
    stable_crate_id UINT64 NOT NULL,
    local_hash UINT64 NOT NULL,
    problem VARCHAR NOT NULL,
    file_path VARCHAR NOT NULL,
    line_nr_start UINT32 NOT NULL,
    line_nr_end UINT32 NOT NULL,

    PRIMARY KEY (stable_crate_id, local_hash, problem),
    FOREIGN KEY (stable_crate_id, local_hash) REFERENCES DefIds(stable_crate_id, local_hash),
    FOREIGN KEY (problem) REFERENCES ProblemTree(problem),
    CHECK (line_nr_end >= line_nr_start)
);