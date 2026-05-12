use crate::{
    FileChangeType, NetworkDirection, ObservationMode, RiskLevel, RunReport, RunStatus, Severity,
};

pub fn render_markdown_receipt(report: &RunReport) -> String {
    fn count_line(count: usize, singular: &str, plural: &str) -> String {
        format!("- {} {}", count, if count == 1 { singular } else { plural })
    }

    let mut lines = Vec::new();
    lines.push("# RunGlass Receipt".to_string());
    lines.push(String::new());
    lines.push(format!("> {}", receipt_narrative(report)));
    lines.push(String::new());
    lines.push("## Key Facts".to_string());
    lines.push(String::new());
    lines.push("| Field | Value |".to_string());
    lines.push("| --- | --- |".to_string());
    lines.push(format!("| Command | `{}` |", report.run.command_display));
    lines.push(format!("| Receipt ID | `{}` |", report.run.id));
    lines.push(format!("| Status | {} |", status_label(&report.run.status)));
    lines.push(format!(
        "| Exit Code | {} |",
        report.run.exit_code.unwrap_or(-1)
    ));
    lines.push(format!(
        "| Duration | {} |",
        report
            .run
            .duration_ms
            .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
            .unwrap_or_else(|| "n/a".to_string())
    ));
    lines.push(format!(
        "| Observation Mode | {} |",
        mode_label(report.run.mode)
    ));
    lines.push(format!("| Working Directory | `{}` |", report.run.cwd));
    lines.push(String::new());
    lines.push("## What Changed".to_string());
    lines.push(format!(
        "- Files: {} created, {} modified, {} deleted.",
        report.summary.files_created, report.summary.files_modified, report.summary.files_deleted
    ));
    lines.push(format!(
        "- Runtime: {} child processes, {} outbound hosts, {} listening ports.",
        report.summary.processes_seen, report.summary.network_hosts, report.summary.ports_opened
    ));
    lines.push(format!(
        "- Docker: {} containers, {} images, {} volumes.",
        report.summary.docker_containers_created,
        report.summary.docker_images_pulled,
        report.summary.docker_volumes_created
    ));
    lines.push(format!(
        "- Risk: {}.",
        match report.summary.risk_level {
            RiskLevel::None => "none",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    ));
    if let Some(risk) = report.risks.iter().find(|risk| {
        matches!(
            risk.severity,
            crate::Severity::Danger | crate::Severity::Warning
        )
    }) {
        lines.push(format!("- Review next: {} - {}", risk.title, risk.detail));
    }
    lines.push(String::new());
    lines.push("## Collector Confidence".to_string());
    for note in collector_confidence_notes(report) {
        lines.push(format!("- {}: {}", note.label, note.detail));
    }
    lines.push(String::new());
    lines.push("## Receipt Metadata".to_string());
    lines.push(format!("Command: `{}`", report.run.command_display));
    lines.push(format!("Receipt ID: `{}`", report.run.id));
    lines.push(format!("Observation Mode: {}", mode_label(report.run.mode)));
    lines.push(format!("Status: {}", status_label(&report.run.status)));
    lines.push(format!("Exit Code: {}", report.run.exit_code.unwrap_or(-1)));
    lines.push(format!(
        "Duration: {}",
        report
            .run
            .duration_ms
            .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
            .unwrap_or_else(|| "n/a".to_string())
    ));
    lines.push(format!("Working Directory: `{}`", report.run.cwd));
    lines.push(String::new());
    lines.push("## Receipt Summary".to_string());
    lines.push(count_line(
        report.summary.files_changed,
        "working-directory file change",
        "working-directory file changes",
    ));
    lines.push(count_line(
        report.summary.processes_seen,
        "child process observed",
        "child processes observed",
    ));
    lines.push(count_line(
        report.summary.network_hosts,
        "outbound host contacted",
        "outbound hosts contacted",
    ));
    lines.push(count_line(
        report.summary.ports_opened,
        "listening port observed",
        "listening ports observed",
    ));
    lines.push(count_line(
        report.summary.docker_containers_created,
        "container created",
        "containers created",
    ));
    lines.push(count_line(
        report.summary.docker_images_pulled,
        "image pulled",
        "images pulled",
    ));
    lines.push(count_line(
        report.summary.docker_volumes_created,
        "volume created",
        "volumes created",
    ));
    lines.push(format!(
        "- overall risk level: {}",
        match report.summary.risk_level {
            RiskLevel::None => "none",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    ));

    if !report.files.is_empty() {
        lines.push(String::new());
        lines.push("## File Changes".to_string());
        for file in report.files.iter().take(12) {
            lines.push(format!(
                "- {} {}",
                match file.change_type {
                    FileChangeType::Created => "created",
                    FileChangeType::Modified => "modified",
                    FileChangeType::Deleted => "deleted",
                },
                file.path
            ));
        }
    }

    if let Some(docker) = &report.docker {
        let mut docker_lines = Vec::new();
        for container in &docker.containers_created {
            docker_lines.push(format!(
                "- container created: {} ({})",
                container.name, container.image
            ));
        }
        for image in &docker.images_pulled {
            docker_lines.push(format!("- image pulled: {}", image.tag));
        }
        for volume in &docker.volumes_created {
            docker_lines.push(format!("- volume created: {}", volume.name));
        }
        for port in &docker.ports_published {
            docker_lines.push(format!(
                "- port published: {}:{} -> {}/{}",
                if port.host_ip.is_empty() {
                    "0.0.0.0"
                } else {
                    port.host_ip.as_str()
                },
                port.host_port,
                port.container_port,
                port.protocol
            ));
        }
        if !docker_lines.is_empty() {
            lines.push(String::new());
            lines.push("## Docker Changes".to_string());
            lines.extend(docker_lines);
        }
    }

    if !report.network.is_empty() {
        lines.push(String::new());
        lines.push("## Network Activity".to_string());
        for event in report.network.iter().take(10) {
            lines.push(format!(
                "- {}:{} ({})",
                event.host.as_deref().unwrap_or(event.ip.as_str()),
                event.port,
                match event.direction {
                    NetworkDirection::Outbound => "outbound",
                    NetworkDirection::Listening => "listening",
                    NetworkDirection::Unknown => "observed",
                }
            ));
        }
    }

    if !report.risks.is_empty() {
        lines.push(String::new());
        lines.push("## Risk Notes".to_string());
        for risk in report.risks.iter().take(8) {
            lines.push(format!("- {}: {}", risk.title, risk.detail));
        }
    }

    if !report.limitations.is_empty() {
        lines.push(String::new());
        lines.push("## Fidelity And Snapshot Notes".to_string());
        for limitation in report.limitations.iter().take(6) {
            lines.push(format!("- {}", limitation));
        }
    }

    lines.join("\n")
}

pub fn render_summary_markdown_receipt(report: &RunReport) -> String {
    let mut lines = Vec::new();
    lines.push("## RunGlass Receipt".to_string());
    lines.push(String::new());
    lines.push(format!("Command: `{}`", report.run.command_display));
    lines.push(format!(
        "Status: {}, exit {}, {}",
        outcome_label(report),
        report
            .run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
        duration_label(report.run.duration_ms)
    ));
    lines.push(format!("Mode: {}", mode_label(report.run.mode)));
    lines.push(format!("Receipt: `{}`", report.run.id));
    lines.push(String::new());
    lines.push("### Impact".to_string());
    lines.push(format!("- Files changed: {}", report.summary.files_changed));
    lines.push(format!(
        "- Processes observed: {}",
        report.summary.processes_seen
    ));
    lines.push(format!(
        "- Network hosts observed: {}",
        report.summary.network_hosts
    ));
    lines.push(format!("- Docker changes: {}", docker_change_count(report)));
    lines.push(format!("- Risk notes: {}", report.risks.len()));

    let review_next = review_next_items(report);
    if !review_next.is_empty() {
        lines.push(String::new());
        lines.push("### Review Next".to_string());
        for (index, item) in review_next.iter().take(5).enumerate() {
            lines.push(format!("{}. {}", index + 1, item));
        }
    }

    lines.push(String::new());
    lines.push("Full receipt: see attached artifact.".to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub fn render_ai_receipt_summary(report: &RunReport) -> String {
    let mut lines = Vec::new();
    lines.push("RUNGLASS_RECEIPT_SUMMARY".to_string());
    lines.push(format!("receipt_id: {}", report.run.id));
    lines.push(format!("command: {}", report.run.command_display));
    lines.push(format!("status: {}", outcome_label(report)));
    lines.push(format!(
        "exit_code: {}",
        report
            .run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "n/a".to_string())
    ));
    lines.push(format!(
        "duration_ms: {}",
        report.run.duration_ms.unwrap_or(0)
    ));
    lines.push(format!("mode: {}", mode_label(report.run.mode)));
    lines.push(format!("cwd: {}", report.run.cwd));
    lines.push(format!("files_changed: {}", report.summary.files_changed));
    lines.push(format!("files_created: {}", report.summary.files_created));
    lines.push(format!("files_modified: {}", report.summary.files_modified));
    lines.push(format!("files_deleted: {}", report.summary.files_deleted));
    lines.push(format!(
        "processes_observed: {}",
        report.summary.processes_seen
    ));
    lines.push(format!("network_hosts: {}", report.summary.network_hosts));
    lines.push(format!("listening_ports: {}", report.summary.ports_opened));
    lines.push(format!("docker_changes: {}", docker_change_count(report)));
    lines.push(format!(
        "risk_level: {}",
        risk_label(&report.summary.risk_level)
    ));
    lines.push("collector_confidence:".to_string());
    for note in collector_confidence_notes(report) {
        lines.push(format!(
            "- {}: {}",
            note.label.to_ascii_lowercase(),
            single_line(note.detail)
        ));
    }
    lines.push("risk_notes:".to_string());
    if report.risks.is_empty() {
        lines.push("- none".to_string());
    } else {
        for risk in report.risks.iter().take(6) {
            lines.push(format!(
                "- {}: {}",
                single_line(&risk.title),
                single_line(&risk.detail)
            ));
        }
    }

    if let Some(excerpt) = failed_output_excerpt(report) {
        lines.push("failed_output_excerpt:".to_string());
        for line in excerpt.lines() {
            lines.push(format!("  {}", line));
        }
    }

    let review_next = review_next_items(report);
    lines.push("review_next:".to_string());
    if review_next.is_empty() {
        lines.push("- inspect the full receipt artifact".to_string());
    } else {
        for item in review_next.iter().take(6) {
            lines.push(format!("- {}", item));
        }
    }
    lines.push("END_RUNGLASS_RECEIPT_SUMMARY".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn receipt_narrative(report: &RunReport) -> String {
    let mut clauses = Vec::new();
    clauses.push(format!(
        "changed {} working-directory file{}",
        report.summary.files_changed,
        if report.summary.files_changed == 1 {
            ""
        } else {
            "s"
        }
    ));
    clauses.push(format!(
        "observed {} child process{}",
        report.summary.processes_seen,
        if report.summary.processes_seen == 1 {
            ""
        } else {
            "es"
        }
    ));
    clauses.push(format!(
        "contacted {} outbound host{}",
        report.summary.network_hosts,
        if report.summary.network_hosts == 1 {
            ""
        } else {
            "s"
        }
    ));
    if report.summary.docker_containers_created
        + report.summary.docker_images_pulled
        + report.summary.docker_volumes_created
        > 0
    {
        clauses.push(format!(
            "changed Docker state with {} containers, {} images, and {} volumes",
            report.summary.docker_containers_created,
            report.summary.docker_images_pulled,
            report.summary.docker_volumes_created
        ));
    }
    format!("This command {}.", clauses.join(", "))
}

fn status_label(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Interrupted => "interrupted",
        RunStatus::FailedToStart => "failed to start",
        RunStatus::TimedOut => "timed out",
    }
}

fn outcome_label(report: &RunReport) -> &'static str {
    if report.run.exit_code.is_some_and(|code| code != 0) {
        "failed"
    } else {
        status_label(&report.run.status)
    }
}

fn mode_label(mode: ObservationMode) -> &'static str {
    match mode {
        ObservationMode::Normal => "normal",
        ObservationMode::Deep => "deep",
    }
}

fn risk_label(risk: &RiskLevel) -> &'static str {
    match risk {
        RiskLevel::None => "none",
        RiskLevel::Low => "low",
        RiskLevel::Medium => "medium",
        RiskLevel::High => "high",
    }
}

fn duration_label(duration_ms: Option<u64>) -> String {
    duration_ms
        .map(|ms| format!("{:.1}s", ms as f64 / 1000.0))
        .unwrap_or_else(|| "n/a".to_string())
}

fn docker_change_count(report: &RunReport) -> usize {
    report.summary.docker_containers_created
        + report.summary.docker_images_pulled
        + report.summary.docker_volumes_created
        + report
            .docker
            .as_ref()
            .map(|docker| docker.networks_created.len() + docker.ports_published.len())
            .unwrap_or(0)
}

struct ConfidenceNote {
    label: &'static str,
    detail: &'static str,
}

fn collector_confidence_notes(report: &RunReport) -> Vec<ConfidenceNote> {
    let (processes, network) = match report.run.mode {
        ObservationMode::Deep => (
            "higher confidence with strace-assisted command-tree exec tracing on Linux; not system-wide tracing",
            "higher confidence with strace-assisted socket tracing plus socket sampling; attribution can still be incomplete",
        ),
        ObservationMode::Normal => (
            "medium confidence from adaptive /proc polling; very short-lived child processes can be missed",
            "best-effort confidence from /proc socket polling plus ss sampling; PID attribution can be incomplete",
        ),
    };

    let docker = if report.docker.is_some() {
        "high confidence before/after Docker Engine diff when Docker is available"
    } else {
        "not observed in this receipt; RunGlass reports Docker changes only when Docker state is available"
    };

    vec![
        ConfidenceNote {
            label: "Files",
            detail: "high confidence within the watched working directory and snapshot limits",
        },
        ConfidenceNote {
            label: "Processes",
            detail: processes,
        },
        ConfidenceNote {
            label: "Network",
            detail: network,
        },
        ConfidenceNote {
            label: "Docker",
            detail: docker,
        },
    ]
}

fn review_next_items(report: &RunReport) -> Vec<String> {
    let mut items = Vec::new();

    for risk in report.risks.iter().filter(|risk| {
        matches!(
            risk.severity,
            Severity::Danger | Severity::Warning | Severity::Info
        )
    }) {
        items.push(format!(
            "{}: {}",
            single_line(&risk.title),
            single_line(&risk.detail)
        ));
        if items.len() >= 3 {
            break;
        }
    }

    for file in &report.files {
        items.push(format!(
            "{} {}",
            match file.change_type {
                FileChangeType::Created => "created",
                FileChangeType::Modified => "modified",
                FileChangeType::Deleted => "deleted",
            },
            file.path
        ));
        if items.len() >= 6 {
            break;
        }
    }

    if report.run.exit_code.is_some_and(|code| code != 0) {
        items.push("inspect failed command output".to_string());
    }

    items
}

fn failed_output_excerpt(report: &RunReport) -> Option<String> {
    if report.run.exit_code.is_none_or(|code| code == 0) {
        return None;
    }
    let source = report
        .stderr
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or(report.stdout.as_deref())?;
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(6)
        .map(|line| truncate_line(line, 220))
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn single_line(value: &str) -> String {
    truncate_line(&value.replace('\n', " "), 220)
}

fn truncate_line(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{
        render_ai_receipt_summary, render_markdown_receipt, render_summary_markdown_receipt,
    };
    use crate::fixture::sample_report;
    use crate::{ObservationMode, RunStatus};

    #[test]
    fn markdown_receipt_includes_receipt_metadata_and_sections() {
        let mut report = sample_report("markdown-export-test".to_string());
        report.run.mode = ObservationMode::Deep;
        let markdown = render_markdown_receipt(&report);

        assert!(markdown.contains("# RunGlass Receipt"));
        assert!(markdown.contains("Command: `docker compose up -d`"));
        assert!(markdown.contains("Receipt ID: `markdown-export-test`"));
        assert!(markdown.contains("Observation Mode: deep"));
        assert!(markdown.contains("## Key Facts"));
        assert!(markdown.contains("## What Changed"));
        assert!(markdown.contains("## Collector Confidence"));
        assert!(markdown.contains("Files: high confidence"));
        assert!(markdown.contains("## Receipt Summary"));
        assert!(markdown.contains("## File Changes"));
        assert!(markdown.contains("## Docker Changes"));
        assert!(markdown.contains("## Network Activity"));
        assert!(markdown.contains("## Risk Notes"));
        assert!(markdown.contains("## Fidelity And Snapshot Notes"));
    }

    #[test]
    fn compact_summary_is_short_and_review_oriented() {
        let report = sample_report("summary-export-test".to_string());
        let summary = render_summary_markdown_receipt(&report);

        assert!(summary.contains("## RunGlass Receipt"));
        assert!(summary.contains("### Impact"));
        assert!(summary.contains("### Review Next"));
        assert!(summary.contains("Full receipt: see attached artifact."));
    }

    #[test]
    fn ai_summary_is_deterministic_and_includes_failed_output_excerpt() {
        let mut report = sample_report("ai-export-test".to_string());
        report.run.status = RunStatus::Completed;
        report.run.exit_code = Some(1);
        report.stderr = Some("first failure line\nsecond failure line\n".to_string());

        let summary = render_ai_receipt_summary(&report);

        assert!(summary.starts_with("RUNGLASS_RECEIPT_SUMMARY\n"));
        assert!(summary.contains("receipt_id: ai-export-test"));
        assert!(summary.contains("status: failed"));
        assert!(summary.contains("exit_code: 1"));
        assert!(summary.contains("collector_confidence:"));
        assert!(summary.contains("failed_output_excerpt:\n  first failure line"));
        assert!(summary.contains("review_next:"));
        assert!(summary.contains("END_RUNGLASS_RECEIPT_SUMMARY"));
    }
}
