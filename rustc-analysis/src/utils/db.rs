use std::{thread, time::{Duration, Instant}};

use duckdb::{Connection, params};

use crate::utils::analysis_results::AnalysisResults;


#[derive(Clone, Debug, Default)]
pub struct Repo {
    pub id: u32,
    pub repo_url: String,
    pub commit_hash: Option<String>,
    pub cargo_args: Option<String>,
    pub analyzed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Crate {
    pub stable_crate_id: u64,
    pub name: String,
    pub version: String,
    pub internal: bool,
    pub path_url: String,
    pub merged_crate_id: Option<u32>,
    pub repo_url: Option<String>,
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
    pub src_repo: u32,
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
    pub fn open(path: &String) -> duckdb::Result<Self> {
        Ok(DB{conn: Connection::open(&path)?})
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

    // try open db, if already opened, wait and retry
    pub fn open_with_retry(path: &String, timeout: Duration) -> Self {
        let start = Instant::now();

        loop {
            let res = Self::open(&path);
            
            match res {
                Ok(db) => { return db; },
                Err(_) => {
                    // eprintln!("failed to open db for {}ms: {}", start.elapsed().as_millis(), err);

                    if start.elapsed() > timeout {
                        panic!("failed to open db, timeout surpassed");
                    } else {
                        thread::sleep(Duration::from_millis(250));
                    }
                },
            }

        }
    }


    // repos
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
            SELECT *
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
                    analyzed: row.get(4).unwrap(),
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

    pub fn set_analyzed_true(&self, id: u32) {
        let mut stmt = self.conn.prepare(r#"
            UPDATE repos
            SET analyzed = TRUE
            WHERE id = ?;
        "#).unwrap();

        stmt.execute(params![id]).unwrap();
    }


    // analysis results
    pub fn save_results(&mut self, _repo_id: u32, results: AnalysisResults) {
        let tx = self.conn.transaction().unwrap();

        {
            // persist Crates
            let mut stmt = tx.prepare(r#"
                INSERT INTO Crates
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(stable_crate_id) DO NOTHING
            "#).unwrap();

            for crate_row in results.crates {
                stmt.execute(crate_row.params()).unwrap();
            }

            // persist DefIDKinds (use id=0)
            // skip for now
            // TODO: SHOULD check if this should remain skipped

            // persist DefIds
            let mut stmt = tx.prepare(r#"
                INSERT OR IGNORE INTO DefIds
                VALUES (?, ?, ?, ?, ?, ?, NULL) 
                "#).unwrap();
            
            for def_id in results.def_ids {
                stmt.execute(def_id.params()).unwrap();
            }

            // tx.appender("DefIds")
            //     .unwrap()
            //     .append_rows(results.def_ids.into_iter().map(DefIdRow::params))
            //     .unwrap();

            // persist Dependencies
            tx.appender("Dependencies")
                .unwrap()
                .append_rows(results.dependencies.into_iter().map(Dependency::params))
                .unwrap();
        }

        tx.commit().unwrap();
    }


    pub const DB_SCHEME_SQL: &'static str = "
        CREATE TABLE FieldList (
            field VARCHAR PRIMARY KEY
        );

        CREATE OR REPLACE TABLE ProblemTree (
            problem VARCHAR PRIMARY KEY,
            parent VARCHAR, -- references ProblemTree.problem (soft constraint)
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
            repo_url, -- points to crate source code repo, not the analyzed repo

            FOREIGN KEY (merged_crate_id) REFERENCES MergedCrates(id)
        );

        CREATE TABLE DefIds (
            stable_crate_id UINT64 NOT NULL,
            local_hash UINT64 NOT NULL,
            src_repo UINT32 NOT NULL,
            def_path_str VARCHAR NOT NULL,
            kind UINT32 NOT NULL,
            unsafe BOOLEAN NOT NULL,
            problem VARCHAR,
            
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
    "; 
}

impl Crate {
    fn params(self) -> (u64, String, String, bool, String, Option<u32>, Option<String>) {
        (
            self.stable_crate_id,
            self.name,
            self.version,
            self.internal,
            self.path_url,
            self.merged_crate_id,
            self.repo_url,
        )
    }
}

impl DefIdRow {
    fn params(self) -> (u64, u64, u32, String, u32, bool) {
        (
            self.stable_crate_id,
            self.local_hash,
            self.src_repo,
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