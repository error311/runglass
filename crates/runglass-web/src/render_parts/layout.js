function buildApp() {
  ensureSelectedFile();
  syncRunDraft();

  document.getElementById('app').innerHTML = `
    <div class="app">
      <aside class="sidebar">
        <div class="brand">
          <div>
            <img class="brand-logo" src="/assets/runglass_wordmark.svg" alt="RunGlass" />
            <p>Receipts for what commands really do.</p>
          </div>
        </div>
        <div class="sidebar-block">
          <div class="sidebar-mode ${receiptModeClass()}">${icons.shield}<span>${receiptModeLabel()}</span></div>
        </div>
        <div class="sidebar-block">
          <button type="button" class="sidebar-section-toggle" data-toggle-quick-actions="true" aria-expanded="${state.quickActionsOpen ? 'true' : 'false'}">
            <span>Quick Actions</span>
            <span class="sidebar-toggle-caret">${state.quickActionsOpen ? 'Hide' : 'Show'}</span>
          </button>
          ${renderSidebarActions()}
        </div>
        <div class="sidebar-block">
          <div class="sidebar-section-title">Recent Receipts</div>
          <div class="recent-runs" id="recent-runs"></div>
        </div>
      </aside>
      <main class="main">
        <section class="section-anchor" id="overview">${renderTopbar()}</section>
        ${renderWorkspaceBanner()}
        ${renderOnboardingPanel()}
        <section class="grid summary-grid section-anchor" id="summary-grid"></section>
        ${renderGithubPanel()}
        <section class="grid content-grid">
          <div class="content-main-grid">
            <div class="content-column content-column-left">
              <div class="panel section-anchor" id="process-panel"></div>
              <div class="panel section-anchor" id="risk-panel"></div>
            </div>
            <div class="content-column content-column-center">
              <div class="panel section-anchor" id="files-panel"></div>
            </div>
            <div class="content-column content-column-right">
              <div class="panel section-anchor" id="network-panel"></div>
              <div class="panel section-anchor" id="docker-panel"></div>
            </div>
            <div class="panel section-anchor output-panel-wide" id="output-panel"></div>
            <div class="panel section-anchor diff-panel" id="diff-panel"></div>
          </div>
          <div class="content-column content-column-timeline">
            <div class="panel section-anchor timeline-panel" id="timeline-panel"></div>
          </div>
        </section>
      </main>
      ${renderNotices()}
      ${renderHelpOverlay()}
      ${renderRevertOverlay()}
    </div>
  `;

  renderSummary();
  renderProcessPanel();
  renderFilesPanel();
  renderNetworkPanel();
  renderRiskPanel();
  renderTimelinePanel();
  renderDiffPanel();
  renderOutputPanel();
  renderDockerPanel();
  renderRecentRuns();
  bindActions();
}

function renderSidebarActions() {
  return `
    <div class="sidebar-actions ${state.quickActionsOpen ? 'open' : 'collapsed'}">
      <button type="button" class="sidebar-action" data-copy="command">${icons.play}<span>Copy Command</span></button>
      <button type="button" class="sidebar-action" data-copy="report-id">${icons.output}<span>Copy Receipt ID</span></button>
      <a class="sidebar-action" href="${reportExportUrl(report.run.id)}" download="runglass-receipt-${escapeHtml(report.run.id)}.html">${icons.download}<span>Download HTML</span></a>
      <a class="sidebar-action" href="${reportMarkdownUrl(report.run.id)}" download="runglass-receipt-${escapeHtml(report.run.id)}.md" target="_blank" rel="noreferrer">${icons.files}<span>Download Markdown</span></a>
      <a class="sidebar-action" href="${reportJsonUrl(report.run.id)}" target="_blank" rel="noreferrer">${icons.settings}<span>Open Raw JSON</span></a>
    </div>
  `;
}

function renderTopbar() {
  const duration = liveRunActive()
    ? formatElapsedMs(state.runJob?.elapsed_ms || 0)
    : formatDuration(report.run.duration_ms || 0);
  const commandDisplay = displayedCommand();
  const ci = report.ci || null;
  const subheadline = liveRunActive()
    ? 'Live receipt building for this command.'
    : ci
      ? `${ci.provider ? `${escapeHtml(ci.provider)} CI` : 'CI'} receipt${report.run.cwd ? ` from ${escapeHtml(report.run.cwd)}` : ''}.`
      : `${receiptSubheadline()}${report.run.cwd ? ` in ${escapeHtml(report.run.cwd)}` : ''}`;
  const heroSentence = receiptHeroSentence();
  const metaRow = liveRunActive()
    ? `
      <div>State: <strong>${state.runJob?.status === 'cancelling' ? 'stopping' : 'running'}</strong></div>
      <div>Elapsed: <strong>${duration}</strong></div>
      <div>Mode: <strong>${activeObservationMode()}</strong></div>
      <div>Shell: <strong>/bin/sh -lc</strong></div>
    `
    : `
      <div>Exit Code: <strong>${report.run.exit_code ?? 'n/a'}</strong></div>
      <div>Duration: <strong>${duration}</strong></div>
      <div>Mode: <strong>${activeObservationMode() === 'deep' ? 'deep' : 'normal'}</strong></div>
      <div>Shell: <strong>${escapeHtml(report.run.shell || 'unknown')}</strong></div>
    `;
  return `
    <section class="topbar">
      <div class="topbar-hero">
        <div class="topbar-hero-head">
          <div class="eyebrow-row">
            <div class="eyebrow">${receiptEyebrow()}</div>
            <div class="status-pill ${isDemoReport(report) ? 'demo' : activeObservationMode() === 'deep' ? 'deep' : 'normal'}">${icons.shield}<span>${receiptStatusLabel()}</span></div>
          </div>
          <div class="actions hero-actions">
            <button type="button" class="action-btn" data-copy="command"><span>${icons.play}</span><span>Copy Command</span></button>
            <a class="action-btn primary" href="${reportExportUrl(report.run.id)}" download="runglass-receipt-${escapeHtml(report.run.id)}.html">${icons.download} Download HTML</a>
            <a class="action-btn" href="${reportMarkdownUrl(report.run.id)}" download="runglass-receipt-${escapeHtml(report.run.id)}.md" target="_blank" rel="noreferrer">${icons.files} Download Markdown</a>
            <a class="action-btn icon" href="${reportJsonUrl(report.run.id)}" target="_blank" rel="noreferrer" title="Open Raw JSON">${icons.settings}</a>
          </div>
        </div>
        <div class="headline-row">
          <div class="headline-shell">
            <div class="headline-label">Receipt for</div>
            <h2 class="headline"><span class="headline-command" title="${escapeHtml(commandDisplay)}">${escapeHtml(commandDisplay)}</span></h2>
          </div>
        </div>
        <div class="hero-summary">${escapeHtml(heroSentence)}</div>
        ${renderInsightRail()}
      </div>
      <div class="topbar-lower">
        <div class="receipt-meta-band">
          <div class="run-composer">
            <div class="run-composer-label">Run a new command</div>
            <form class="run-form" data-run-form>
              <input class="run-input" type="text" name="command" data-run-draft value="${escapeHtml(state.runDraft)}" placeholder="npm install" ${state.runPending ? 'disabled' : ''} />
              <select class="run-mode-select" name="mode" data-run-mode ${state.runPending ? 'disabled' : ''}>
                <option value="normal" ${state.runMode === 'normal' ? 'selected' : ''}>Normal</option>
                <option value="deep" ${state.runMode === 'deep' ? 'selected' : ''}>Deep</option>
              </select>
              ${state.runPending ? `<button type="button" class="action-btn" data-stop-run="true">${stopButtonLabel()}</button>` : `<button type="submit" class="action-btn primary">Run Command</button>`}
            </form>
            <div class="run-note ${runNoteTone()}">${escapeHtml(runNoteText())}</div>
            ${renderRunLiveStats()}
          </div>
          <div class="receipt-meta-copy">
            <div class="receipt-meta-header">
              <div class="mode-pill ${receiptModeClass()}">${receiptModeLabel()}</div>
            </div>
            <div class="subheadline">${subheadline}</div>
            <div class="meta-row">${metaRow}</div>
            ${ci && !liveRunActive() ? renderCiMetaRow(ci) : ''}
          </div>
        </div>
        <div class="topbar-side">
          ${renderTopOutputCard()}
        </div>
      </div>
    </section>
  `;
}

function renderCiMetaRow(ci) {
  const items = [];
  items.push(`CI: <strong>${escapeHtml(ci.provider || 'unknown')}</strong>`);
  if (ci.repository) items.push(`Repo: <strong>${escapeHtml(ci.repository)}</strong>`);
  if (ci.pull_request) items.push(`PR: <strong>#${escapeHtml(ci.pull_request)}</strong>`);
  if (ci.commit_sha) items.push(`Commit: <strong>${escapeHtml(String(ci.commit_sha).slice(0, 12))}</strong>`);
  if (ci.run_url) {
    items.push(`<a class="text-link" href="${escapeHtml(ci.run_url)}" target="_blank" rel="noreferrer">CI run</a>`);
  }
  if (ci.artifact_name) items.push(`Artifact: <strong>${escapeHtml(ci.artifact_name)}</strong>`);
  return `<div class="meta-row ci-meta-row">${items.map((item) => `<div>${item}</div>`).join('')}</div>`;
}

function renderInsightRail() {
  const insights = receiptInsightCards();
  if (!insights.length) return '';
  return `
    <div class="insight-rail" aria-label="Guided receipt insights">
      ${insights.map((insight) => `
        <button type="button" class="insight-card ${insight.tone}" data-summary-action="${escapeHtml(insight.action)}">
          <span class="insight-kicker">${escapeHtml(insight.kicker)}</span>
          <span class="insight-title">${escapeHtml(insight.title)}</span>
          <span class="insight-detail">${escapeHtml(insight.detail)}</span>
        </button>
      `).join('')}
    </div>
  `;
}

function receiptInsightCards() {
  const summary = activeSummary();
  const docker = activeDocker();
  const risks = activeRisks();
  const files = activeFiles();
  const network = activeNetwork();
  const insights = [];

  if (summary.files_changed > 0) {
    insights.push({
      kicker: 'What changed',
      title: `${summary.files_created} created, ${summary.files_modified} modified, ${summary.files_deleted} deleted`,
      detail: files[0] ? `Start with ${files[0].path}` : 'Open the file receipt.',
      action: 'files',
      tone: 'files',
    });
  } else {
    insights.push({
      kicker: 'What changed',
      title: 'No working-directory file changes',
      detail: 'The command left the watched directory unchanged.',
      action: 'files',
      tone: 'quiet',
    });
  }

  if (docker && hasDockerChanges(docker)) {
    insights.push({
      kicker: 'Environment',
      title: `${docker.containers_created.length} containers, ${docker.images_pulled.length} images`,
      detail: docker.ports_published.length ? `${docker.ports_published.length} published port${docker.ports_published.length === 1 ? '' : 's'} need review.` : 'Docker changed without published ports.',
      action: 'docker',
      tone: 'docker',
    });
  } else if (network.length) {
    insights.push({
      kicker: 'Network',
      title: `${countUniqueOutboundHosts(network)} outbound host${countUniqueOutboundHosts(network) === 1 ? '' : 's'}`,
      detail: 'Review host and socket attribution.',
      action: 'network-hosts',
      tone: 'network',
    });
  }

  const notableRisk = risks.find((risk) => risk.severity === 'danger' || risk.severity === 'warning') || risks[0];
  if (notableRisk) {
    insights.push({
      kicker: 'Review next',
      title: notableRisk.title,
      detail: notableRisk.recommendation || notableRisk.detail,
      action: 'risks',
      tone: statusTone(notableRisk.severity),
    });
  } else {
    insights.push({
      kicker: 'Review next',
      title: 'No notable risks generated',
      detail: 'RunGlass did not flag sensitive files, public ports, Docker exposure, or failed exit status.',
      action: 'risks',
      tone: 'success',
    });
  }

  return insights.slice(0, 3);
}

function receiptHeroSentence() {
  const summary = activeSummary();
  const processes = activeProcesses();
  const network = activeNetwork();
  const risks = activeRisks();
  const filesChanged = summary.files_changed || 0;
  const processesSeen = liveRunActive() && state.runJob
    ? (state.runJob.processes_seen ?? processes.length)
    : (summary.processes_seen ?? processes.length);
  const networkHosts = liveRunActive() && state.runJob
    ? (state.runJob.network_hosts ?? countUniqueOutboundHosts(network))
    : (summary.network_hosts ?? countUniqueOutboundHosts(network));
  const openedPorts = liveRunActive() && state.runJob
    ? (state.runJob.ports_opened ?? network.filter((item) => item.direction === 'listening').length)
    : network.filter((item) => item.direction === 'listening').length;
  const riskNotes = risks.length;
  const start = liveRunActive() ? 'So far, this command' : 'This command';
  return `${start} changed ${filesChanged} ${pluralize(filesChanged, 'file')}, observed ${processesSeen} ${pluralizeWord(processesSeen, 'process', 'processes')}, contacted ${networkHosts} ${pluralize(networkHosts, 'host')}, opened ${openedPorts} ${pluralize(openedPorts, 'port')}, and generated ${riskNotes} ${pluralizeWord(riskNotes, 'risk note', 'risk notes')}.`;
}

function pluralize(count, noun) {
  return count === 1 ? noun : `${noun}s`;
}

function pluralizeWord(count, singular, plural) {
  return count === 1 ? singular : plural;
}

function renderNotices() {
  if (!state.notices.length) return '';
  return `
    <div class="notice-stack" aria-live="polite">
      ${state.notices.map((notice) => `
        <div class="notice ${escapeHtml(notice.tone)}">${escapeHtml(notice.message)}</div>
      `).join('')}
    </div>
  `;
}

function renderTopOutputCard() {
  const summary = visibleRunOutputSummary();
  const liveStdout = state.runPending && state.runJob && (state.runJob.status === 'running' || state.runJob.status === 'cancelling')
    ? (state.runJob.stdout_preview || 'Waiting for stdout...')
    : null;
  const liveStderr = state.runPending && state.runJob && state.runJob.stderr_preview
    ? state.runJob.stderr_preview
    : null;
  if (liveStdout === null && !summary) return '';

  const liveBody = `${liveStdout}${liveStderr ? `\n\n[stderr]\n${liveStderr}` : ''}`;
  if (liveStdout !== null) {
    return `
      <div class="top-output">
        <div class="top-output-header">
          <div class="top-output-title">${icons.output}<span>Live Command Output</span></div>
          <a class="text-link" href="#output-panel">Open Full Output</a>
        </div>
        <div class="top-output-note">Live output for the command running from the receipt composer.</div>
        <pre>${renderTerminalOutput(liveBody)}</pre>
      </div>
    `;
  }

  return `
    <div class="top-output top-output-summary ${summary.tone}">
      <div class="top-output-header">
        <div class="top-output-title">${icons.output}<span>${escapeHtml(summary.title)}</span></div>
        <div class="top-output-actions">
          <a class="text-link" href="#output-panel">Open Full Output</a>
          <button type="button" class="text-link top-output-dismiss" data-dismiss-run-output="true">Dismiss</button>
        </div>
      </div>
      <div class="top-output-status-row">
        <span class="top-output-status-pill ${summary.tone}">${escapeHtml(summary.statusLabel)}</span>
        <span class="top-output-status-meta">${escapeHtml(summary.command)}</span>
      </div>
      <div class="top-output-note">${escapeHtml(summary.note)}</div>
      ${summary.preview ? `<pre>${renderTerminalOutput(summary.preview)}</pre>` : ''}
    </div>
  `;
}

function visibleRunOutputSummary() {
  if (!state.lastRunOutput) return null;
  if (!state.lastRunOutput.runId) return state.lastRunOutput;
  if (state.lastRunOutput.runId !== report.run.id) return null;
  return state.lastRunOutput;
}
function buildRunOutputSummary(options) {
  const command = options.command || displayedCommand();
  const preview = compactOutputPreview(options.preview || options.stdout || options.stderr || options.error || '');
  if (options.status === 'failed') {
    return {
      title: 'Last Command Output',
      statusLabel: 'Failed',
      tone: 'error',
      command,
      note: options.error || 'The command failed before a final receipt could be loaded.',
      preview,
      runId: null,
    };
  }
  if (options.status === 'cancelled') {
    return {
      title: 'Last Command Output',
      statusLabel: 'Stopped',
      tone: 'warning',
      command,
      note: 'This receipt was stopped early. Partial observations were saved.',
      preview,
      runId: options.runId || null,
    };
  }
  return {
    title: 'Last Command Output',
    statusLabel: 'Completed',
    tone: 'success',
    command,
    note: 'The command finished. Final stdout and stderr are attached to the receipt output below.',
    preview,
    runId: options.runId || null,
  };
}

function compactOutputPreview(text) {
  const lines = formatTerminalOutput(text)
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter((line) => line.trim().length > 0)
    .slice(0, 6);
  return lines.join('\n');
}
