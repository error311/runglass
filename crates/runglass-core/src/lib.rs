mod collectors;
pub mod fixture;
mod reporting;
mod revert;
mod runtime;
mod storage;
mod types;

pub use collectors::{
    capture_docker_snapshot, diff_docker_snapshots, diff_snapshots, read_network_samples,
    read_network_samples_ss, read_process_tree_sample, snapshot_directory,
    snapshot_directory_with_stats, snapshot_file_byte_limit, summarize_network_samples,
};
pub use reporting::{
    build_command_report, render_ai_receipt_summary, render_markdown_receipt,
    render_summary_markdown_receipt, unique_hosts,
};
pub use revert::{apply_revert, preview_revert, render_reverse_patch};
pub use runtime::{
    run_observed_command, run_observed_command_in_mode, run_observed_command_with_control,
    run_observed_command_with_control_in_mode, run_observed_command_with_progress,
    run_observed_shell_command, run_observed_shell_command_in_mode,
    run_observed_shell_command_with_control, run_observed_shell_command_with_control_in_mode,
    run_observed_shell_command_with_progress,
};
pub use storage::{
    delete_report, home_dir, latest_report, list_reports, load_report, make_run_id,
    prepare_run_paths, prune_reports, report_run_dir, reports_dir, write_report_bundle,
};
pub use types::*;
