#!/usr/bin/bash

# This checks all downloaded repositories to check for compilation failures.
# Failures here need to be resolved by:
# 1. installing missing dependencies
# 2. adding necessary args to Repos.cargo_args in the duckdb file

ls | xargs -I{} cargo -Z unstable-options -C {} check &>out