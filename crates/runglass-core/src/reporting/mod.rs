mod build;
mod markdown;
mod risks;
mod timeline;

pub use build::build_command_report;
pub use markdown::{
    render_ai_receipt_summary, render_markdown_receipt, render_summary_markdown_receipt,
};
pub use risks::unique_hosts;
pub(crate) use risks::{build_summary, derive_risk_level, derive_risks, empty_summary, risk_tags};
pub(crate) use timeline::network_events;
