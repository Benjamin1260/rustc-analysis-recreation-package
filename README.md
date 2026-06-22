# RustC-Analysis Recreation Package

## Contents

1. **[Running The Analyzer](#1-running-the-analyzer)**
   1. **[Dependencies And Requirements](#11-dependencies-and-requirements)** 
   2. **[Building The Binaries](#12-building-the-binaries)**
   3. **[Obtaining A List Of Repositories](#13-obtaining-a-list-of-repositories)**
      1. **[Generating A New .duckdb File](#131-generating-a-new-duckdb-file)**
      2. **[Reusing An Existing .duckdb File](#132-reusing-an-existing-duckdb-file)**
   4. **[Fetching The Repositories](#14-fetching-the-repositories)**
   5. **[Ensuring Succesful Compilation](#15-ensuring-succesful-compilation)**
   6. **[Runnning The Analysis](#16-runnning-the-analysis)**
   7. **[Interpreting The DuckDB Output](#17-interpreting-the-duckdb-output)**

## 1. Running The Analyzer

In this section, we explain how to use the rustc-analysis tool to analyze GitHub Rust repositories for their concurrent usages.

### 1.1 Dependencies And Requirements

The following software must be installed before running the analysis:

- Rust nightly toolchain (via Cargo/rustup)
- DuckDB

### 1.2 Building The Binaries

To build the binaries, run `cargo +nightly build --release` in the directory with the manifest file. This will crate a `target/release/` directory with two binaries inside: `rustc_analysis` and `rustc_analysis_wrapper`. The first one is aimed at the end user, and is responible for providing the CLI functionality. The second is aimed at performing the analysis, and should only ever be invoked by `Cargo`.

### 1.3 Obtaining A List Of Repositories

After you have succesfully built the necessary binaries, you need to get obtain the initial `.duckdb` file, includies the repositories and hashes you want to analyze. You may obtain this file in the following two ways:

#### 1.3.1 Generating A New `.duckdb` File

This can be done by using the `init-repo-list` command on the compiled binary. As arguments, you should specify `repo_count`, the number of repositories you want to fetch, and `duckdb_path`, the location of the output `.duckdb` file.

#### 1.3.2 Reusing An Existing `.duckdb` File

Alternatively, you may also reuse an existing `.duckdb` file that already has these repositories and commit hashes defined. This way, you can reproduce someone else's findings. A copy of the `.duckdb` used in [our study](https://repository.tudelft.nl/record/uuid:1ae0a0bc-9c1b-4cae-a7a3-f7e734ec1531) can be found [under the duckdb dir](duckdb/analysis.duckdb).

### 1.4 Fetching The Repositories

Once you have a `.duckdb` file with the repositories you want to analyze, you need to actually download these to fetch their source code. You can do this by simply running the `analyze` command with the path to your `.duckdb` file and the tool will automatically fetch and download the repositories and hashes, as specified in the `.duckdb` file.

### 1.5 Ensuring Succesful Compilation

Before running the analysis, it is important you ensure all of the newly fetched repositories compile succesfully. This is a requirement for the rust compiler as it cannot generate a HIR/MIR representation for invalid projects.

To do this, we provide the [cargo check script](shell_scripts/cargo_check.sh). This runs the check command, as the analysis tool would. If this passes, the analysis tool will run succesfully. If it fails, it means you either need to download the necessary dependencies, or provide the required cargo arguments.

An example of this might be if the crate allows the user to specify a specific database implementation, or if the manifest file is not located at the root of the repository.

Once you have the necessary cargo arguments, you should add these manually to the `.duckdb` file, you can do this through the CLI or through a UI provided by DuckDB, which can be launched using `duckdb -ui [file]`. The arguments should be inserted in the `Repos` table, under the `cargo_args` column.

### 1.6 Runnning The Analysis

Once all dependencies and cargo arguments have been resolved, you can start analyzing the repositories. This can be done using the `analyze` command, specifying `enable_reuse`, to indicate we do not want to download the repositories again. 

At the end of this analysis, all extracted data will be added to the `.duckdb` file you specified, allowing you to run queries on it to analyze interesting features.

### 1.7 Interpreting The DuckDB Output

To analyze the output of our tool, use the DuckDB CLI or notebook functionality. Within this repository, we include the notebook used to find the answers to our research questions [in the duckdb directory](duckdb/notebook.db). For an entity-relationship diagram of the output file, please check our [research paper](https://repository.tudelft.nl/record/uuid:1ae0a0bc-9c1b-4cae-a7a3-f7e734ec1531).

Furthermore, a description of the categorization we did can be found [in the documents directory](documents/categorization.md).
