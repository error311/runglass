function reportJsonUrl(runId) {
  return `/reports/${encodeURIComponent(runId)}.json`;
}

function reportExportUrl(runId) {
  return `/reports/${encodeURIComponent(runId)}/export.html`;
}

function reportMarkdownUrl(runId) {
  return `/reports/${encodeURIComponent(runId)}/receipt.md`;
}

function reportReversePatchUrl(runId) {
  return `/reports/${encodeURIComponent(runId)}/reverse.patch`;
}

function jobStatusUrl(jobId) {
  return `/api/jobs/${encodeURIComponent(jobId)}.json`;
}

function jobEventsUrl(jobId) {
  return `/api/jobs/${encodeURIComponent(jobId)}/events`;
}

function cancelJobUrl(jobId) {
  return `/api/jobs/${encodeURIComponent(jobId)}/cancel`;
}

function revertPreviewUrl() {
  return '/api/revert/preview';
}

function revertApplyUrl() {
  return '/api/revert/apply';
}

function updateLocation(runId) {
  const next = new URL(window.location.href);
  next.searchParams.set('run', runId);
  window.history.replaceState({}, '', next);
}

function isDemoReport(value) {
  return String(value.run.id).endsWith('_demo-receipt') || value.limitations.some((item) => String(item).toLowerCase().includes('fixture'));
}

function reportObservationMode(value = report) {
  return value?.run?.mode === 'deep' ? 'deep' : 'normal';
}

function activeObservationMode() {
  if (state.runPending && state.runJob && state.runJob.mode === 'deep') {
    return 'deep';
  }
  return reportObservationMode(report);
}

function receiptModeLabel() {
  if (liveRunActive()) return activeObservationMode() === 'deep' ? 'Live Deep Receipt' : 'Live Normal Receipt';
  if (isDemoReport(report)) return 'Demo Receipt';
  return reportObservationMode(report) === 'deep' ? 'Deep Receipt' : 'Normal Receipt';
}

function receiptModeClass() {
  if (isDemoReport(report)) return 'demo';
  return reportObservationMode(report) === 'deep' ? 'deep' : 'normal';
}

function displayedCommand() {
  if (liveRunActive() && state.runDraft.trim()) {
    return state.runDraft.trim();
  }
  return report.run.command_display;
}

function stopButtonLabel() {
  if (!state.runPending) return 'Stop Run';
  if (state.runJob && state.runJob.status === 'cancelling') return 'Stopping...';
  return 'Stop Run';
}

function runNoteTone() {
  if (state.runError) return 'error';
  if (state.runPending && state.runJob && state.runJob.status === 'cancelling') return 'warning';
  if (state.runPending) return 'running';
  return '';
}

function runNoteText() {
  if (state.runError) return state.runError;
  if (state.runPending && state.runJob && state.runJob.status === 'cancelling') {
    return `Stopping live receipt build: ${state.runJob.command}`;
  }
  if (state.runPending && state.runJob) {
    return `Live ${state.runJob.mode === 'deep' ? 'deep' : 'normal'} receipt building for: ${state.runJob.command} (${formatElapsedMs(state.runJob.elapsed_ms || 0)})`;
  }
  if (state.runPending) {
    return 'Preparing local receipt build...';
  }
  return `Runs locally through /bin/sh -lc in ${state.runMode === 'deep' ? 'deep' : 'normal'} mode and swaps in the finished observed receipt.`;
}

function renderRunLiveStats() {
  if (!(state.runPending && state.runJob && state.runJob.status === 'running')) {
    return '';
  }
  return `
    <div class="run-live-stats">
      <span class="badge created">Processes ${state.runJob.processes_seen ?? 0}</span>
      <span class="badge network">Hosts ${state.runJob.network_hosts ?? 0}</span>
      <span class="badge modified">Ports ${state.runJob.ports_opened ?? 0}</span>
    </div>
  `;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function renderTerminalOutput(value) {
  return escapeHtml(formatTerminalOutput(value));
}

function formatTerminalOutput(value) {
  const input = String(value || '').replaceAll('\r\n', '\n');
  const rows = [[]];
  let row = 0;
  let col = 0;
  let savedRow = 0;
  let savedCol = 0;

  const ensureRow = (index) => {
    while (rows.length <= index) rows.push([]);
  };
  const writeChar = (char) => {
    ensureRow(row);
    while (rows[row].length < col) rows[row].push(' ');
    rows[row][col] = char;
    col += 1;
  };
  const eraseLine = (mode) => {
    ensureRow(row);
    if (mode === 1) {
      rows[row].splice(0, col + 1);
      return;
    }
    if (mode === 2) {
      rows[row] = [];
      return;
    }
    rows[row].splice(col);
  };

  for (let index = 0; index < input.length; index += 1) {
    const char = input[index];

    if (char === '\x1b') {
      if (input[index + 1] === '[') {
        const match = input.slice(index + 2).match(/^([?=>!0-9;:]*)([@-~])/);
        if (match) {
          const params = match[1].replace(/[?=>!]/g, '').split(/[;:]/).filter(Boolean).map((part) => Number(part));
          const count = Math.max(1, params[0] || 1);
          const final = match[2];
          if (final === 'A') row = Math.max(0, row - count);
          else if (final === 'B') row += count;
          else if (final === 'C') col += count;
          else if (final === 'D') col = Math.max(0, col - count);
          else if (final === 'E') { row += count; col = 0; }
          else if (final === 'F') { row = Math.max(0, row - count); col = 0; }
          else if (final === 'G') col = Math.max(0, count - 1);
          else if (final === 'H' || final === 'f') { row = Math.max(0, (params[0] || 1) - 1); col = Math.max(0, (params[1] || 1) - 1); }
          else if (final === 'J' && (params[0] || 0) === 2) rows.splice(0, rows.length, []);
          else if (final === 'K') eraseLine(params[0] || 0);
          else if (final === 's') { savedRow = row; savedCol = col; }
          else if (final === 'u') { row = savedRow; col = savedCol; }
          ensureRow(row);
          index += match[0].length + 1;
          continue;
        }
      }

      if (input[index + 1] === ']') {
        const oscEnd = input.slice(index + 2).search(/\x07|\x1b\\/);
        if (oscEnd >= 0) {
          index += oscEnd + (input[index + oscEnd + 2] === '\x1b' ? 3 : 2);
          continue;
        }
      }

      index += 1;
      continue;
    }

    if (char === '\n') {
      row += 1;
      col = 0;
      ensureRow(row);
    } else if (char === '\r') {
      col = 0;
    } else if (char === '\b') {
      col = Math.max(0, col - 1);
    } else if (char === '\t') {
      const spaces = 8 - (col % 8);
      for (let offset = 0; offset < spaces; offset += 1) writeChar(' ');
    } else if (char >= ' ' || char === '\u001b') {
      writeChar(char);
    }
  }

  return rows
    .map((line) => line.join('').trimEnd())
    .join('\n')
    .replace(/\n+$/, '');
}

function decodeHtml(value) {
  const node = document.createElement('textarea');
  node.innerHTML = value;
  return node.value;
}

function formatDuration(ms) {
  return `${(ms / 1000).toFixed(2)}s`;
}

function formatElapsedMs(ms) {
  return `${Math.max(0, ms / 1000).toFixed(1)}s`;
}

function formatClock(value) {
  return new Date(value).toLocaleTimeString([], { hour12: false });
}

function timeAgo(value) {
  const seconds = Math.max(1, Math.round((Date.now() - new Date(value).getTime()) / 1000));
  if (seconds < 60) return `${seconds} seconds ago`;
  if (seconds < 3600) return `${Math.round(seconds / 60)} minutes ago`;
  return `${Math.round(seconds / 3600)} hours ago`;
}

function compactCommandLabel(value) {
  const normalized = String(value || '').replace(/\s+/g, ' ').trim();
  if (normalized.length <= 72) return normalized;
  return `${normalized.slice(0, 42).trimEnd()} ... ${normalized.slice(-22).trimStart()}`;
}

function titleCase(value) {
  return String(value).replaceAll('_', ' ').replace(/\b\w/g, (char) => char.toUpperCase());
}

function severityColor(severity) {
  if (severity === 'danger') return '#f16b71';
  if (severity === 'warning') return '#f0bf4b';
  if (severity === 'success') return '#59db88';
  return '#6a9bff';
}

function hexToRgba(hex, alpha) {
  const clean = hex.replace('#', '');
  const value = clean.length === 3 ? clean.split('').map((char) => char + char).join('') : clean;
  const numeric = parseInt(value, 16);
  const r = (numeric >> 16) & 255;
  const g = (numeric >> 8) & 255;
  const b = numeric & 255;
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

initializeRuns();
