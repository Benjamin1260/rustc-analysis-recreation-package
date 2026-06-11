# Workfile

> **Brief Description:** \
> This file will be used to list things that still have to done/implemented in the rustc-analysis crate.



## Architectures

### L0-Pipeline Architecture:

1. (optional) creating .duckdb file
2. (optional) generating repository list
3. fetch data
4. run analysis
5. duckdb postprocessing

#### 1. creating .duckdb file

create the .duckdb scheme, establish a connection.

#### 2. generating repository list

This stage should fill the `.duckdb`-file of github repositories we aim to analyze.. \
The form used for this should be the same as defined in `db_sceme_setup.sql`. \

#### 3. fetch data

This stage should use the input repo + hash to fetch the repository. \ 
Maybe we can say if no hash is present, it fetches the latest commit and also writes the hash to the `.duckdb` file. 

#### 4. run analysis
see [run analysis](#run-analysis)

#### 5. duckdb postprocessing

This stage should perform any duckdb postprocessing that is universal across all analysis. An example of this is trying to already resolve/instantiate `MergedCrates`.

### Run Analysis:

Use the existing `.duckdb` file to run the rustc analysis (repeatedly) and updating the file with results in the process.

1. cargo invokes us
2. we invoke rustc with our callback function
3. create `cargo_invocation.csv` file, every invocation, append `[repo_id, cargo_args, err]`
4. user can add this `cargo_invocation.csv` file on `analyze` and it will be persisted to the `.duckdb` file
5. if analysis is successful, what output should be returned? how do we store this in the `.duckdb` file?

#### Output Tables:

Tables in order of duckdb insertion:
1. `Repos` (already present)
2. `Crates`
3. `DefIdKinds` (what was this again?)
4. `DefIds`
5. `Dependencies`

All of these can be stored in `Vec<RowType>` and inserted later (I am worried about memory usage).


### Binary Architecture:

#### User-Facing Binary:

This binary will be the main handle/API for the user to use. It will be responsible for invoking the different stages of the pipeline

#### Cargo-Facing Binary:


### CrateMetadataIndex:

This is a resolver to match rustc crates to their cargo representation. This way we can get metadata like the version and path which are not available on the rustc side. We distinguish 3 types:

#### Crate Origin:
##### Self:
This is the crate being compiled/checked itself. It is not present on the `rustc` side in `TyCtxt.crates()`. This is the main loop over which `AnalysisResults.crates` is initialized. So we need to handle this crate separately.

##### Cargo Crates:
These are the most straightforward since they are returned by both the `rustc` side aswel as `cargo_metadata`. We simply iterate over these and match the `rustc` data `(name, path)` to the `cargo_metadata` data to try and match these. Multiple crates may share the same name, hence we use the `path` for resolving conflicts.

##### Sysroot Crates:
These are returned by `rustc` but not by `cargo_metadata` meaning we need to handle these separately. They come from the **rust toolchain** used. These crates include `core`, `alloc`, `std`, `proc_macro`, `test`... 

To tackle this, we parse the **toolchain** used in the `CrateMetadataIndex`, this will be used to resolve the version. Furthermore, we define an enum: `CrateOrigin` which is `Self`/`Cargo`/`RustLib`.

#### Construction:
`Self` and `Cargo` crates should be initialized in the user-side binary, serialized and sent to the rustc-side. `RustLib` crates are only visible to `rustc` and thus will be added here.

For `RustLib` crates, this means `version`, `internal` and `path_url` will not be available from `CrateMetadataIndex`. This is not a problem since we can define them as follows:
- `Crate.version` = `rustc_toolchain_version`
- `Crate.internal` = `false`
- `Crate.path_url` = `path_to_rustc_toolchain`


## Tasks

### L0-pipeline: (CURRENT TASK)

#### Considered Done:

- [x] CLI command which takes a number to start analysis from scratch and spits out fully finished `.duckdb` file
- [x] CLI command which takes a `.duckdb` file with repository urls and fetches their latest comits, if hash is present, use those instead, then analyses those and runs the analysis, resulting in a filled `.duckdb` file

#### Description:

implement [L0-pipeline architecture](#l0-pipeline-arch)

- [x] create commands `InitRepoList` and `Analyze`
- [x] implement `InitRepoList` 
- [x] implement `Analyze`
- [x] hash present &rarr; fetch hash, else fetch latest and write hash
- [x] create table row types
- [x] create `AnalysisCallback` struct with tables using these row types (KEEP IN MIND: CALLBACK MIGHT BE INVOKED MULTIPLE TIMES, WHICH/WHEN DO WE WRITE BACK?) (maybe not since only callback for workspace crate?)
- [x] make callback write to these tables
- [x] after rustc invocation, write these results to `.duckdb` file
- [x] fix `CrateMetadataIndex` to properly resolve/handle `self` and `rust-lib` crates
- [ ] fix TODOs in sourcecode &larr;

STOP HERE, GET SOME RESULTS FIRST (e.g. on 3 small crates manually)

- [ ] implement write to `cargo_invocation.csv` on analysis invocation
- [ ] create API/command to simply write `cargo_invocation.csv` (without err msg)
- [ ] create API/command to simply read `cargo_invocation.csv`
- [ ] create arg on `analysis`-command that takes `cargo_invocation.csv`
- [ ] implement pipeline `stage5`


#### Questions:

- should the top level command be shell or rust? &rarr; Rust
- what calls will be made? what binaries will be used? &rarr; see [binaries](#binary-architecture)
- what will internal data flow look like between binaries? &rarr; `.duckdb`-file
- how do we deal with repo-specific `cargo_args` &rarr; require user-interaction with `.csv` file
- should we save `err_msg` in db? &rarr; no!



### Recreation Package

- [ ] push `.duckdb` file with repositories + hash we used
- [ ] write `README`s for different parts of the crate

### Misc/Uncategorized

- [ ] remove duckDB table init from Cargo binary (should be created in User binary once)
