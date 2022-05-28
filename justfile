_default:
    @just --list

# Runs clippy on the source
check:
    cargo clippy -- -D warnings

# builds the repo
build:
    cargo build

# Runs the non interactive tests
test:
    cargo test tests::non_interactive