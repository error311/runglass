const PANEL_TITLES = {
  files: 'Receipt Files',
  processes: 'Receipt Process Tree',
  network: 'Receipt Network Activity',
  docker: 'Receipt Docker Changes',
  risks: 'Receipt Summary',
  timeline: 'Receipt Timeline',
  output: 'Receipt Output',
};

const PANEL_STATE_COPY = {
  files: {
    live: { title: 'Observed Changes', detail: 'Live file changes are still building. Final verified diffs land when the command exits.' },
    final: { title: 'Verified After-Run Diff', detail: 'This panel shows the final working-directory diff captured before and after the command.' },
  },
  processes: {
    live: { title: 'Live Receipt Building', detail: 'Process observations update during the run and settle into the final receipt when the command exits.' },
    final: { title: 'Final Receipt', detail: 'This process tree reflects the completed command receipt.' },
  },
  network: {
    live: { title: 'Live Receipt Building', detail: 'Observed sockets and host attribution update while the command is active.' },
    final: { title: 'Final Receipt', detail: 'This panel shows the final best-effort network receipt for the command window.' },
  },
  docker: {
    live: { title: 'Live Receipt Building', detail: 'Docker resources appear here as RunGlass observes container, image, volume, and network changes.' },
    final: { title: 'Final Receipt', detail: 'This panel shows the final Docker Engine diff captured before and after the command.' },
  },
  risks: {
    live: { title: 'Live Receipt Building', detail: 'Risk notes update as RunGlass observes sensitive files, ports, and Docker changes.' },
    final: { title: 'Final Receipt', detail: 'These notes summarize what stood out in the final receipt.' },
  },
  timeline: {
    live: { title: 'Live Receipt Building', detail: 'Timeline entries arrive during the run and may be expanded when the command exits.' },
    final: { title: 'Final Receipt', detail: 'This timeline is the final ordered receipt of what RunGlass observed.' },
  },
  output: {
    live: { title: 'Live Receipt Building', detail: 'Terminal output is streaming now. Final stdout and stderr are attached when the command exits.' },
    final: { title: 'Final Receipt', detail: 'This console section shows the final stdout and stderr captured for the receipt.' },
  },
};

function renderRecentRuns(options = {}) {
  const section = document.getElementById('recent-runs');
  if (!section) return;
  const active = document.activeElement;
  const shouldRestoreSearchFocus = Boolean(
    options.preserveSearchFocus
      && active instanceof HTMLInputElement
      && active.dataset.receiptSearch !== undefined
  );
  const selectionStart = shouldRestoreSearchFocus ? active.selectionStart : null;
  const selectionEnd = shouldRestoreSearchFocus ? active.selectionEnd : null;
  if (!runs.length) {
    section.innerHTML = renderEmptyState(
      'No saved receipts yet',
      'Run a command from the composer to create your first observed receipt.',
      'Try: npm install'
    );
    restoreReceiptSearchFocus(shouldRestoreSearchFocus, selectionStart, selectionEnd);
    return;
  }

  const filtered = filterRecentRuns();
  const visibleCount = Math.min(filtered.length, state.recentRunsVisible);
  section.innerHTML = `
    <input class="receipt-search" type="search" data-receipt-search value="${escapeHtml(state.receiptSearch)}" placeholder="Search receipts" />
    ${filtered.length ? filtered.slice(0, visibleCount).map((item) => `
      <div class="run-link-wrap">
        <button type="button" class="run-link clickable ${item.id === report.run.id ? 'active' : ''}" data-run-id="${escapeHtml(item.id)}">
          <div class="run-link-title">${escapeHtml(item.command_display)}</div>
          ${compactCommandLabel(item.command_display) !== item.command_display ? `<div class="run-link-subtitle">${escapeHtml(compactCommandLabel(item.command_display))}</div>` : ''}
          <div class="run-link-stats">
            <span class="run-mini-pill">${item.files_changed} file${item.files_changed === 1 ? '' : 's'}</span>
            <span class="run-mini-pill">${item.processes_seen} proc</span>
            <span class="run-mini-pill">${item.network_hosts} host${item.network_hosts === 1 ? '' : 's'}</span>
            ${!item.is_demo ? `<span class="run-mini-pill">${item.mode === 'deep' ? 'deep' : 'normal'}</span>` : ''}
            <span class="run-mini-pill">${titleCase(item.risk_level)} risk</span>
          </div>
          <div class="run-link-meta">
            <span>${timeAgo(item.started_at)}</span>
            <span class="badge ${item.is_demo ? 'modified' : 'created'}">${item.is_demo ? 'Demo' : 'Observed'}</span>
            <span>${titleCase(String(item.status || 'unknown').replaceAll('_', ' '))}</span>
          </div>
        </button>
        <button type="button" class="run-delete" data-delete-run="${escapeHtml(item.id)}" title="Delete receipt" aria-label="Delete ${escapeHtml(item.command_display)} receipt">${icons.trash}</button>
      </div>
    `).join('') : renderEmptyState('No matching receipts', 'Try a command, file path, host, risk level, or mode.', 'Examples: docker, high, deep')}
    ${filtered.length > visibleCount ? `
      <button type="button" class="sidebar-action load-more-runs" data-load-more-runs="true">
        ${icons.reports}<span>Load ${Math.min(5, filtered.length - visibleCount)} More</span>
      </button>
    ` : ''}
    ${filtered.length > 5 ? `<div class="run-count-note">Showing ${visibleCount} of ${filtered.length} matching receipts.</div>` : ''}
  `;
  restoreReceiptSearchFocus(shouldRestoreSearchFocus, selectionStart, selectionEnd);
}

function restoreReceiptSearchFocus(shouldRestore, selectionStart, selectionEnd) {
  if (!shouldRestore) return;
  const next = document.querySelector('[data-receipt-search]');
  if (!(next instanceof HTMLInputElement)) return;
  next.focus();
  if (selectionStart !== null && selectionEnd !== null) {
    next.setSelectionRange(selectionStart, selectionEnd);
  }
}

function filterRecentRuns() {
  const query = state.receiptSearch.trim().toLowerCase();
  const filtered = query ? runs.filter((item) => [
    item.id,
    item.command_display,
    item.status,
    item.risk_level,
    item.mode,
    item.is_demo ? 'demo' : 'observed',
  ].some((value) => String(value || '').toLowerCase().includes(query))) : runs;
  return sortRecentRuns(filtered);
}

function sortRecentRuns(items) {
  return [...items].sort((left, right) => {
    const rightStarted = new Date(right.started_at).getTime() || 0;
    const leftStarted = new Date(left.started_at).getTime() || 0;
    return rightStarted - leftStarted || String(right.id || '').localeCompare(String(left.id || ''));
  });
}

function renderEmptyState(title, detail, hint = '') {
  return `
    <div class="empty-state">
      <div class="empty-state-glow"></div>
      <div class="empty-state-title">${escapeHtml(title)}</div>
      <div class="empty-state-detail">${escapeHtml(detail)}</div>
      ${hint ? `<div class="empty-state-hint">${escapeHtml(hint)}</div>` : ''}
    </div>
  `;
}

function renderWorkspaceBanner() {
  const details = workspaceBannerDetails();
  if (!details) return '';
  return `
    <section class="workspace-banner ${escapeHtml(details.tone)}">
      <div class="workspace-banner-title">${escapeHtml(details.title)}</div>
      <div class="workspace-banner-copy">${escapeHtml(details.detail)}</div>
    </section>
  `;
}

function renderOnboardingPanel() {
  if (liveRunActive()) return '';
  if (!isDemoReport(report)) return '';
  const cards = [
    {
      title: 'Run one command',
      detail: 'Use the composer above. RunGlass wraps it through /bin/sh -lc and starts a live receipt.',
    },
    {
      title: 'Scan the impact',
      detail: 'The insight cards and summary grid point you to files, Docker, network, output, and risks.',
    },
    {
      title: 'Export or revert',
      detail: 'Save HTML/Markdown for sharing, or revert tracked file changes when snapshots are available.',
    },
  ];
  return `
    <section class="onboarding-panel">
      <div>
        <div class="onboarding-kicker">First run guide</div>
        <h3 class="onboarding-title">Turn a command into a receipt you can inspect.</h3>
      </div>
      <div class="onboarding-cards">
        ${cards.map((card) => `
          <div class="onboarding-card">
            <div class="onboarding-card-title">${escapeHtml(card.title)}</div>
            <div class="onboarding-card-detail">${escapeHtml(card.detail)}</div>
          </div>
        `).join('')}
      </div>
    </section>
  `;
}

function panelHeader(title, iconSvg, right = '', kind = null) {
  const help = kind
    ? `<button type="button" class="panel-help" data-help-kind="${escapeHtml(kind)}" aria-label="About ${escapeHtml(title)}">i</button>`
    : '';
  return `<div class="panel-header ${right ? 'has-actions' : ''}"><div class="panel-title"><div class="panel-title-wrap">${iconSvg}<span>${title}</span>${help}</div></div>${right ? `<div class="panel-header-actions">${right}</div>` : ''}</div>`;
}

function panelStateNote(kind) {
  if (PANEL_STATE_COPY[kind]) {
    return liveRunActive() ? PANEL_STATE_COPY[kind].live : PANEL_STATE_COPY[kind].final;
  }
  return { title: 'Receipt Panel', detail: 'RunGlass is building a receipt for this command.' };
}

function panelFidelity(kind) {
  if (kind === 'files') {
    return { title: 'Files: high confidence', detail: 'Working directory before/after diff.' };
  }
  if (kind === 'processes') {
    if (activeObservationMode() === 'deep') {
      return { title: 'Processes: higher confidence', detail: 'Deep mode supplements polling with strace-based exec tracing on Linux.' };
    }
    return { title: 'Processes: medium confidence', detail: 'Observed from adaptive /proc polling in normal mode; very short-lived processes may still be missed.' };
  }
  if (kind === 'network') {
    if (activeObservationMode() === 'deep') {
      return { title: 'Network: higher confidence', detail: 'Deep mode supplements polling with strace-based socket tracing on Linux.' };
    }
    return { title: 'Network: medium confidence', detail: 'Best-effort socket attribution from /proc polling plus ss sampling in normal mode.' };
  }
  if (kind === 'docker') {
    return { title: 'Docker: high confidence', detail: 'Before/after Docker Engine diff.' };
  }
  if (kind === 'risks') {
    return { title: 'Risk Notes: derived', detail: 'Built from observed files, ports, Docker changes, and exit status.' };
  }
  if (kind === 'timeline') {
    return { title: 'Timeline: mixed confidence', detail: 'Composed from observed process, file, network, Docker, and command events.' };
  }
  if (kind === 'output') {
    return { title: 'Console Output: high confidence', detail: 'Captured directly from the child process stdout and stderr streams.' };
  }
  return { title: 'Receipt Data', detail: 'RunGlass is surfacing best-effort observations for this command.' };
}

function panelHelpText(kind) {
  const stateNote = panelStateNote(kind);
  const fidelity = panelFidelity(kind);
  const mode = liveRunActive()
    ? `Mode: live ${state.runJob?.mode === 'deep' ? 'deep' : 'normal'} receipt`
    : isDemoReport(report)
      ? 'Mode: demo receipt'
      : `Mode: final ${reportObservationMode(report)} receipt`;
  return `${mode}\n${stateNote.title}: ${stateNote.detail}\n${fidelity.title}: ${fidelity.detail}`;
}

function renderHelpOverlay() {
  if (!state.openHelpKind) return '';
  const details = helpDialogContent(state.openHelpKind);
  return `
    <div class="help-overlay" data-help-close="overlay">
      <div class="help-dialog" role="dialog" aria-modal="true" aria-label="${escapeHtml(details.title)}">
        <div class="help-dialog-header">
          <div>
            <h3 class="help-dialog-title">${escapeHtml(details.title)}</h3>
            <div class="help-dialog-subtitle">${escapeHtml(details.subtitle)}</div>
          </div>
          <button type="button" class="action-btn icon" data-help-close="button" aria-label="Close help">×</button>
        </div>
        <div class="help-dialog-body">
          <div class="help-card">
            <strong>${escapeHtml(details.state.title)}</strong>
            <div>${escapeHtml(details.state.detail)}</div>
          </div>
          <div class="help-card">
            <strong>${escapeHtml(details.fidelity.title)}</strong>
            <div>${escapeHtml(details.fidelity.detail)}</div>
          </div>
        </div>
      </div>
    </div>
  `;
}

function renderRevertOverlay() {
  if (!state.revertPreview) return '';
  const preview = state.revertPreview;
  const conflictCount = preview.conflicts?.length || 0;
  const applyPaths = revertApplyPaths(preview);
  const includedConflictCount = revertIncludedConflictCount(preview);
  const targetLabel = state.revertFiles.length === 1
    ? state.revertFiles[0]
    : `${preview.target_count} file changes`;
  return `
    <div class="help-overlay" data-revert-close="overlay">
      <div class="help-dialog" role="dialog" aria-modal="true" aria-label="Revert supported file changes">
        <div class="help-dialog-header">
          <div>
            <h3 class="help-dialog-title">Revert Supported File Changes</h3>
            <div class="help-dialog-subtitle">RunGlass will apply stored file snapshots for ${escapeHtml(targetLabel)} back into the working directory.</div>
          </div>
          <button type="button" class="action-btn icon" data-revert-close="button" aria-label="Close revert dialog">×</button>
        </div>
        <div class="help-dialog-body">
          <div class="help-card">
            <strong>This will</strong>
            <div>Restore ${preview.restore_modified} modified files, delete ${preview.delete_created} created files, and restore ${preview.restore_deleted} deleted files.</div>
          </div>
          ${conflictCount ? `
            <div class="help-card">
              <strong>Changed Since Receipt</strong>
              <div>${conflictCount} file${conflictCount === 1 ? '' : 's'} changed again after this receipt finished. Skipped by default.</div>
              <div class="inline-stack-gap-10 stack-grid-gap-8">
                ${preview.conflicts.map((item) => `
                  <div class="file-row">
                    <div>
                      <div>${escapeHtml(item.path)}</div>
                      <div class="muted">${escapeHtml(item.detail)}</div>
                    </div>
                    <button type="button" class="action-btn" data-revert-conflict="${escapeHtml(item.path)}" ${state.revertBusy ? 'disabled' : ''}>${revertConflictPathIsSkipped(item.path) ? 'Skipped' : 'Include'}</button>
                  </div>
                `).join('')}
              </div>
            </div>
          ` : ''}
          <div class="help-card">
            <strong>Selected Apply Set</strong>
            <div>${applyPaths.length} supported file change${applyPaths.length === 1 ? '' : 's'} will be reverted from this dialog.</div>
            ${includedConflictCount ? `<div class="inline-stack-gap-8">${includedConflictCount} changed-again file${includedConflictCount === 1 ? '' : 's'} are still included and will be force-reverted.</div>` : ''}
          </div>
          <div class="help-card">
            <strong>It will not undo</strong>
            <div>Docker changes, network calls, database writes, or commands run outside the watched working directory.</div>
          </div>
          ${state.revertMessage ? `<div class="help-card"><strong>Result</strong><div>${escapeHtml(state.revertMessage)}</div></div>` : ''}
        </div>
        <div class="dialog-actions">
          <button type="button" class="action-btn" data-revert-close="button" ${state.revertBusy ? 'disabled' : ''}>Cancel</button>
          <button type="button" class="action-btn ${includedConflictCount ? '' : 'primary'}" data-revert-apply="selected" ${state.revertBusy || !applyPaths.length ? 'disabled' : ''}>${state.revertBusy ? 'Applying...' : includedConflictCount ? 'Force Revert Selected' : 'Apply Selected Files'}</button>
        </div>
      </div>
    </div>
  `;
}

function helpDialogContent(kind) {
  return {
    title: helpTitle(kind),
    subtitle: liveRunActive()
      ? 'This panel is showing live receipt data while the command is still running.'
      : 'This panel is showing final receipt data for the completed command.',
    state: panelStateNote(kind),
    fidelity: panelFidelity(kind),
  };
}

function helpTitle(kind) {
  return PANEL_TITLES[kind] || 'Receipt Panel';
}

function summaryCardHelp(action) {
  if (action === 'files' || action === 'config-files') {
    return panelHelpText('files');
  }
  if (action === 'processes') {
    return panelHelpText('processes');
  }
  if (action === 'network-hosts' || action === 'network-ports') {
    return panelHelpText('network');
  }
  if (action === 'docker') {
    return panelHelpText('docker');
  }
  if (action === 'risks') {
    return panelHelpText('risks');
  }
  return 'RunGlass summary card';
}

function summaryCardIsActive(action) {
  return state.summaryFocus === action;
}

function fileTabs() {
  const files = activeFiles();
  const counts = {
    all: files.length,
    created: files.filter((file) => file.change_type === 'created').length,
    modified: files.filter((file) => file.change_type === 'modified').length,
    deleted: files.filter((file) => file.change_type === 'deleted').length,
  };

  return `
    <div class="tabs">
      ${tab('file', 'all', 'All', counts.all, state.fileTab === 'all')}
      ${tab('file', 'created', 'Created', counts.created, state.fileTab === 'created')}
      ${tab('file', 'modified', 'Modified', counts.modified, state.fileTab === 'modified')}
      ${tab('file', 'deleted', 'Deleted', counts.deleted, state.fileTab === 'deleted')}
    </div>
  `;
}

function networkTabs() {
  const network = activeNetwork();
  const hosts = aggregateHosts(network);
  return `
    <div class="tabs">
      ${tab('network', 'hosts', 'Hosts', hosts.length, state.networkTab === 'hosts')}
      ${tab('network', 'connections', 'Connections', network.length, state.networkTab === 'connections')}
    </div>
  `;
}

function diffControls() {
  return `
    <div class="diff-controls">
      <div class="tabs">
        <button type="button" class="tab ${state.diffMode === 'unified' ? 'active' : ''}" data-diff-mode="unified">Unified</button>
        <button type="button" class="tab ${state.diffMode === 'side-by-side' ? 'active' : ''}" data-diff-mode="side-by-side">Side by Side</button>
      </div>
      ${revertSnapshotsAvailable() && selectedFile() ? `<button type="button" class="action-btn" data-revert-scope="single">Revert File Change</button>` : ''}
      <button type="button" class="action-btn" data-copy="path">Copy Path</button>
    </div>
  `;
}

function filePanelControls() {
  const selectedCount = state.revertSelectedPaths.length;
  const revertUnavailable = !revertSnapshotsAvailable();
  return `
    <div class="panel-subactions">
      ${fileTabs()}
      <button type="button" class="action-btn" data-revert-scope="all" ${revertUnavailable ? 'disabled' : ''}>Revert Supported Files</button>
      <button type="button" class="action-btn" data-revert-scope="selected" ${revertUnavailable || !selectedCount ? 'disabled' : ''}>Revert Selected Files${selectedCount ? ` (${selectedCount})` : ''}</button>
      <a class="action-btn ${revertUnavailable ? 'disabled' : ''}" href="${revertUnavailable ? '#' : reportReversePatchUrl(report.run.id)}" ${revertUnavailable ? 'aria-disabled="true"' : `target="_blank" rel="noreferrer" download="runglass-receipt-${escapeHtml(report.run.id)}-reverse.patch"`}>Download Reverse Patch</a>
    </div>
  `;
}

function tab(group, value, label, count, active) {
  return `<button type="button" class="tab ${active ? 'active' : ''}" data-tab-group="${group}" data-tab-value="${value}">${label} (${count})</button>`;
}
