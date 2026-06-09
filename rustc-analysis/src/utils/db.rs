use duckdb::{Connection, params};

use crate::utils::analysis_results::AnalysisResults;


#[derive(Clone, Debug, Default)]
pub struct Repo {
    pub id: u32,
    pub repo_url: String,
    pub commit_hash: Option<String>,
    pub cargo_args: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Crate {
    pub stable_crate_id: u64,
    pub src_repo: u32,
    pub name: String,
    pub version: String,
    pub internal: bool,
    pub path_url: String,
    pub merged_crate_id: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct DefIdKind {
    pub id: u32,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct DefIdRow {
    pub stable_crate_id: u64,
    pub local_hash: u64,
    pub def_path_str: String,
    pub kind: u32,
    pub nonsafe: bool, // unsafe is reserved keyword
}

#[derive(Clone, Debug, Default)]
pub struct Dependency {
    pub from_stable_crate_id: u64,
    pub from_local_hash: u64,
    pub to_stable_crate_id: u64,
    pub to_local_hash: u64,
}


pub struct DB {
    pub conn: Connection,
}

impl DB {
    // DB creation/initiation
    pub fn open(path: String) -> Self {
        let connection = Connection::open(path)
            .expect("failed to establish duckdb connection");

        DB{conn: connection}   
    }

    pub fn insert_table_scheme(&self) {
        self.conn.execute(Self::DB_SCHEME_SQL, ())
            .expect("failed to create tables");

        let mut stmt = self.conn.prepare(r#"
            INSERT INTO DefIdKinds
                VALUES (?, ?);
        "#).unwrap();
        
        stmt.execute(params![0, "placeholder"]).unwrap();
    }


    // repo interaction
    pub fn insert_repo_urls(&self, url_list: Vec<String>) -> duckdb::Result<()>{
        self.conn.execute("BEGIN TRANSACTION;", ()).unwrap();

        {
            let mut stmt = self.conn.prepare(
                r#"
                INSERT INTO Repos (repo_url)
                VALUES (?)
                "#,
            ).unwrap();
            
            for url in url_list {
                stmt.execute(params![url]).unwrap();
            }
        }

        self.conn.execute("COMMIT;", ()).unwrap();
        Ok(())
    }

    pub fn fetch_repos(&self) -> Vec<Repo> {
        let mut stmt = self.conn.prepare(r#"
            SELECT id, repo_url, commit_hash, cargo_args
            FROM Repos
            ORDER BY id
            "#,
        ).unwrap();

        stmt
            .query_map([], |row| {
                Ok(Repo {
                    id: row.get(0).unwrap(),
                    repo_url: row.get(1).unwrap(),
                    commit_hash: row.get(2).unwrap(),
                    cargo_args: row.get(3).unwrap(),
                })
            }).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap()
    }

    pub fn update_commit_hash(&self, id: u32, hash: String) {
        let mut stmt = self.conn.prepare(r#"
            UPDATE Repos
            SET commit_hash = ?
            WHERE id = ?
            "#,
        ).unwrap();

        stmt.execute(params![hash, id]).unwrap();
    }


    // analysis results
    pub fn save_results(&mut self, results: AnalysisResults) {
        // TODO: SHOULD bring transaction back

        {
            eprintln!{"{:#?}", results};

            // persist Crates
            self.conn.appender("Crates")
                .unwrap()
                .append_rows(results.crates.into_iter().map(Crate::params))
                .unwrap();

            // persist DefIDKinds (use id=0)
            // skip for now
            // TODO: SHOULD check if this should remain skipped

            // persist DefIds
            self.conn.appender("DefIds")
                .unwrap()
                .append_rows(results.def_ids.into_iter().map(DefIdRow::params))
                .unwrap();

            // persist Dependencies
            self.conn.appender("Dependencies")
                .unwrap()
                .append_rows(results.dependencies.into_iter().map(Dependency::params))
                .unwrap();
        }

    }


    pub const DB_SCHEME_SQL: &'static str = "
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
            cargo_args VARCHAR
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
            src_repo UINT32 NOT NULL,
            name VARCHAR NOT NULL,
            version VARCHAR NOT NULL,
            internal BOOLEAN NOT NULL,
            path_url VARCHAR NOT NULL,
            merged_crate_id UINT32,

            FOREIGN KEY (src_repo) REFERENCES Repos(id),
            FOREIGN KEY (merged_crate_id) REFERENCES MergedCrates(id),
            UNIQUE (src_repo, name, version, path_url)
        );

        CREATE TABLE DefIds (
            stable_crate_id UINT64 NOT NULL,
            local_hash UINT64 NOT NULL,
            def_path_str VARCHAR NOT NULL,
            kind UINT32 NOT NULL,
            unsafe BOOLEAN NOT NULL,
            
            PRIMARY KEY (stable_crate_id, local_hash),
            FOREIGN KEY (stable_crate_id) REFERENCES Crates(stable_crate_id),
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
            CHECK (line_nr_start >= 0),
            CHECK (line_nr_end >= line_nr_start)
        );
    "; 
}

impl Crate {
    fn params(self) -> (u64, u32, String, String, bool, String, Option<u32>) {
        (
            self.stable_crate_id,
            self.src_repo,
            self.name,
            self.version,
            self.internal,
            self.path_url,
            self.merged_crate_id,
        )
    }
}

impl DefIdRow {
    fn params(self) -> (u64, u64, String, u32, bool) {
        (
            self.stable_crate_id,
            self.local_hash,
            self.def_path_str,
            self.kind,
            self.nonsafe,
        )
    }
}

impl Dependency {
    fn params(self) -> (u64, u64, u64, u64) {
        (
            self.from_stable_crate_id,
            self.from_local_hash,
            self.to_stable_crate_id,
            self.to_local_hash,
        )
    }
}