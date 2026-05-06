function parseUnifiedDiff(diff) {
  const rows = [];
  let beforeLine = 1;
  let afterLine = 1;

  diff.split('\n').forEach((line) => {
    if (!line) return;
    if (line.startsWith('@@')) {
      const match = /@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
      if (match) {
        beforeLine = Number(match[1]);
        afterLine = Number(match[2]);
      }
      return;
    }
    if (line.startsWith('+')) {
      rows.push({ kind: 'add', beforeNumber: '', beforeText: '', afterNumber: afterLine++, afterText: line.slice(1) });
      return;
    }
    if (line.startsWith('-')) {
      rows.push({ kind: 'remove', beforeNumber: beforeLine++, beforeText: line.slice(1), afterNumber: '', afterText: '' });
      return;
    }
    rows.push({ kind: 'context', beforeNumber: beforeLine++, beforeText: line.startsWith(' ') ? line.slice(1) : line, afterNumber: afterLine++, afterText: line.startsWith(' ') ? line.slice(1) : line });
  });
  return rows;
}

function renderUnifiedDiff(rows) {
  return `
    <div class="diff-shell">
      <div class="diff-header">
        <div>Unified Diff</div>
      </div>
      ${rows.map((row) => `
        <div class="diff-row unified ${row.kind}">
          <div class="diff-cell before">${row.kind === 'add' ? row.afterNumber || '' : row.beforeNumber || ''}</div>
          <div class="diff-marker">${row.kind === 'add' ? '+' : row.kind === 'remove' ? '-' : ' '}</div>
          <div class="diff-line before">${escapeHtml(row.kind === 'add' ? row.afterText : row.beforeText)}</div>
        </div>
      `).join('')}
    </div>
  `;
}

function renderSideBySideDiff(rows) {
  return `
    <div class="diff-shell">
      <div class="diff-header">
        <div>Before</div>
        <div>After</div>
      </div>
      ${rows.map((row) => `
        <div class="diff-row ${row.kind}">
          <div class="diff-cell before">${row.beforeNumber || ''}</div>
          <div class="diff-line before">${escapeHtml(row.beforeText)}</div>
          <div class="diff-cell after">${row.afterNumber || ''}</div>
          <div class="diff-line after">${escapeHtml(row.afterText)}</div>
        </div>
      `).join('')}
    </div>
  `;
}

function selectedFile() {
  const files = activeFiles();
  return files.find((file) => file.path === state.selectedFilePath) || files.find((file) => file.diff) || files[0] || null;
}

function ensureSelectedFile() {
  const files = activeFiles();
  if (state.selectedFilePath && files.some((file) => file.path === state.selectedFilePath)) {
    return;
  }
  const preferred = files.find((file) => file.diff) || files[0] || null;
  state.selectedFilePath = preferred ? preferred.path : null;
}

function resetViewState() {
  state.fileTab = 'all';
  state.networkTab = 'hosts';
  state.networkFocus = null;
  state.networkPortFocus = null;
  state.diffMode = 'unified';
  state.selectedFilePath = null;
  state.selectedProcessPid = null;
  state.selectedRiskId = null;
  state.selectedTimelineKey = null;
  state.summaryFocus = null;
  state.openHelpKind = null;
  state.revertPreview = null;
  state.revertSelectedPaths = [];
  state.revertSkippedPaths = [];
  state.revertFiles = [];
  state.revertBusy = false;
  state.revertMessage = null;
  state.revertStatus = null;
}

function syncRunDraft() {
  if (state.runDraftSourceId === report.run.id && state.runDraft) {
    return;
  }
  if (state.runPending && state.runDraft) {
    return;
  }
  state.runDraft = report.run.command_display;
  state.runDraftSourceId = report.run.id;
  state.runMode = reportObservationMode(report);
  state.runError = null;
}
