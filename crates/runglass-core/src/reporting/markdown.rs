use crate::{FileChangeType, NetworkDirection, ObservationMode, RiskLevel, RunReport, RunStatus};

pub fn render_markdown_receipt(report: &RunReport) -> String {
    fn count_line(count: usize, singular: &str, plural: &str) -> String {
        format!("- {} {}", count, if count == 1 { singular } else { plural })
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

    fn mode_label(mode: ObservationMode) -> &'static str {
        match mode {
            ObservationMode::Normal => "normal",
            ObservationMode::Deep => "deep",
        }
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

#[cfg(test)]
mod tests {
    use super::render_markdown_receipt;
    use crate::fixture::sample_report;
    use crate::ObservationMode;

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
        assert!(markdown.contains("## Receipt Summary"));
        assert!(markdown.contains("## File Changes"));
        assert!(markdown.contains("## Docker Changes"));
        assert!(markdown.contains("## Network Activity"));
        assert!(markdown.contains("## Risk Notes"));
        assert!(markdown.contains("## Fidelity And Snapshot Notes"));
    }
}
