windows := "x86_64-pc-windows-msvc"

# Run the tests that need no hardware
test:
    cargo test --workspace

# Format check and lints, for Linux and Windows code paths
check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo xwin clippy --workspace --all-targets --target {{windows}} -- -D warnings

# Build the Windows executables into target/x86_64-pc-windows-msvc/release
build:
    cargo xwin build --workspace --release --target {{windows}}
