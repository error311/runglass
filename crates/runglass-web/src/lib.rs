use std::fs;
use std::path::Path;

use anyhow::Result;
use runglass_core::RunReport;

mod github;
mod http;
mod jobs;
mod script;
mod server;
mod style;
mod ui;

pub use server::{serve_report, serve_report_on_port};
pub use ui::render_html;

pub fn write_standalone_html(report: &RunReport, path: &Path) -> Result<()> {
    fs::write(path, render_html(report)?)?;
    Ok(())
}
