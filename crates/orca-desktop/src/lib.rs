pub fn run() {
    eprintln!(
        "This is the portable ORCA desktop stub from the default workspace build.\n\n\
         CLI executable:\n  cargo build -p orca-cli --release\n  ./target/release/orca\n\n\
         GUI executable:\n  npm install --prefix ui/desktop\n  npm run build --prefix ui/desktop\n  cargo build --manifest-path crates/orca-desktop-runtime/Cargo.toml --release\n  ./crates/orca-desktop-runtime/target/release/orca-desktop\n\n\
         The GUI executable requires Tauri's platform WebView libraries. On this machine the installed GLib/WebView stack is too old, so the real Tauri runtime is kept outside the default workspace build."
    );
}
