//! MANUAL-ONLY interactive demo of the `aterm::tui` prompt wrappers.
//!
//! This is NOT part of the test suite and is never run by CI or the
//! orchestrator — those environments are headless (no TTY), and every prompt
//! below enables raw mode, which hangs or errors without a real terminal. CI
//! only *compiles* this file (via `cargo build --all-targets` / clippy) to keep
//! the wrappers honest.
//!
//! To try it yourself, in an interactive terminal:
//!
//! ```text
//! cargo run --example tui_demo
//! ```
//!
//! Use the arrow keys to move, type to filter, Enter to select, Space to toggle
//! multiselect items, and Esc / Ctrl-C to cancel any prompt.

use aterm::tui;

fn main() {
    let fruits: Vec<String> = ["Apple", "Banana", "Cherry", "Date", "Elderberry"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    match tui::select("Pick a fruit", &fruits) {
        Ok(choice) => println!("selected: {}  {}", choice, tui::green_check()),
        Err(e) => println!("select: {e}  {}", tui::red_cross()),
    }

    match tui::multiselect("Pick any fruits", &fruits) {
        Ok(choices) => println!("selected: {choices:?}"),
        Err(e) => println!("multiselect: {e}"),
    }

    match tui::confirm("Continue?", true) {
        Ok(true) => println!("{}", tui::green("yes")),
        Ok(false) => println!("{}", tui::red("no")),
        Err(e) => println!("confirm: {e}"),
    }

    match tui::input("A short note") {
        Ok(note) => println!("note: {note}"),
        Err(e) => println!("input: {e}"),
    }

    match tui::input_with_default("Your name", "anonymous") {
        Ok(name) => println!("hello, {}", tui::bold(&name)),
        Err(e) => println!("input: {e}"),
    }

    match tui::required_input("Operation slug (required)") {
        Ok(slug) => println!("slug: {slug}"),
        Err(e) => println!("required_input: {e}"),
    }

    match tui::password("API secret key") {
        Ok(secret) => println!("captured {} masked chars", secret.len()),
        Err(e) => println!("password: {e}"),
    }
}
