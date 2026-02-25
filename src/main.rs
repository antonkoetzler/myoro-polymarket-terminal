#![allow(dead_code)]

mod config;
mod copy_trading;
mod discover;
mod live;
mod pm;
mod shared;
mod sports;
mod strategies;
mod tui;
mod weather;

fn main() -> anyhow::Result<()> {
    let config = config::load()?;
    tui::run(config)
}
