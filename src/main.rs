#![windows_subsystem = "windows"]

/*!
 * @file main.rs
 * @brief DVD Ripper binary entry point supporting both GUI and CLI modes for Movies & TV Series.
 */

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;

use dvd_ripper::application::cli_runner::CliApplicationRunner;
use dvd_ripper::cli::Args;
use dvd_ripper::daemon::run_daemon_with_container;
#[cfg(feature = "gui")]
use dvd_ripper::gui;
use dvd_ripper::{AppContainer, AppInitializer};

/// Main entry point executing GUI application or CLI transcoding process.
fn main() -> Result<()> {
    #[cfg(feature = "gui")]
    {
        // Check raw args to see if user requested CLI mode or passed specific CLI parameters
        let raw_args: Vec<String> = std::env::args().collect();
        let is_cli = raw_args.iter().any(|arg| {
            arg == "--cli"
                || arg == "-h"
                || arg == "--help"
                || arg == "-V"
                || arg == "--version"
                || arg == "--tv"
                || arg == "--daemon"
                || arg == "-s"
                || arg == "--search"
                || arg == "--imdb-id"
                || arg == "--select-index"
                || arg == "--iso"
        });

        if !is_cli {
            // Run native desktop GUI by default when feature is enabled
            if let Err(e) = gui::run_gui() {
                eprintln!("GUI Error: {}", e);
            }
            return Ok(());
        }
    }

    // Attach to parent console if running from terminal in CLI mode on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        unsafe extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
        }
        AttachConsole(0xFFFFFFFF);
    }

    // CLI mode execution path
    let mut args = Args::parse();
    let app_initializer = AppInitializer;
    let _cfg = app_initializer.bootstrap(&mut args);
    let container = Arc::new(AppContainer::build_from_args(&args));

    if args.daemon {
        return run_daemon_with_container(args, 10, container);
    }

    let runner = CliApplicationRunner::new(container);
    runner.run(args)
}
