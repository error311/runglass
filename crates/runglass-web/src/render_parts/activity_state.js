function filterFiles() {
  const files = activeFiles();
  if (state.fileTab === 'all') return files;
  return files.filter((file) => file.change_type === state.fileTab);
}

function liveRunActive() {
  return Boolean(state.runPending && state.runJob && state.runJob.status === 'running');
}

function activeSummary() {
  return liveRunActive() && state.runJob && state.runJob.summary ? state.runJob.summary : report.summary;
}

function activeFiles() {
  return liveRunActive() && Array.isArray(state.runJob.files) ? state.runJob.files : report.files;
}

function activeProcesses() {
  return liveRunActive() && Array.isArray(state.runJob.processes) ? state.runJob.processes : report.processes;
}

function activeNetwork() {
  return liveRunActive() && Array.isArray(state.runJob.network) ? state.runJob.network : report.network;
}

function activeDocker() {
  return liveRunActive() && state.runJob && state.runJob.docker ? state.runJob.docker : report.docker;
}

function activeRisks() {
  return liveRunActive() && Array.isArray(state.runJob.risks) ? state.runJob.risks : report.risks;
}

function activeTimelineEvents() {
  return liveRunActive() && Array.isArray(state.runJob.events) ? state.runJob.events : report.events;
}

function revertSnapshotsAvailable() {
  return !isDemoReport(report)
    && Array.isArray(report.files)
    && report.files.length > 0
    && report.files.some((file) => file.before_artifact_path || file.after_artifact_path);
}

function revertActionsEnabled() {
  return revertSnapshotsAvailable();
}

function revertStatusNote() {
  if (!Array.isArray(report.files) || !report.files.length) {
    return '';
  }
  if (isDemoReport(report)) {
    return 'Revert actions are unavailable for demo receipts. Run a real observed command to store reversible file snapshots.';
  }
  if (!revertSnapshotsAvailable()) {
    return 'This older receipt does not include stored revert snapshots. New observed receipts can be reverted.';
  }
  if (state.revertSelectedPaths.length) {
    return `${state.revertSelectedPaths.length} file${state.revertSelectedPaths.length === 1 ? '' : 's'} selected for revert preview.`;
  }
  return 'Select file changes to preview a targeted revert, or use Revert All.';
}

function revertPathIsSelected(path) {
  return state.revertSelectedPaths.includes(path);
}

function currentWorkspaceFileStatus(path) {
  if (!state.revertStatus) return null;
  for (const item of state.revertStatus.safe || []) {
    if (item.path === path) return { tone: 'receipt', text: 'Current: matches receipt' };
  }
  for (const item of state.revertStatus.already_reverted || []) {
    if (item.path === path) return { tone: 'reverted', text: 'Current: reverted from receipt' };
  }
  for (const item of state.revertStatus.conflicts || []) {
    if (item.path === path) return { tone: 'changed', text: 'Current: changed since receipt' };
  }
  for (const item of state.revertStatus.missing_artifacts || []) {
    if (item.path === path) return { tone: 'missing', text: 'Current: not revertable from this receipt' };
  }
  return null;
}

function workspaceBannerDetails() {
  if (liveRunActive()) {
    return {
      tone: 'info',
      title: 'Live receipt building',
      detail: 'RunGlass is capturing output, files, processes, network activity, and Docker changes while this command runs.',
    };
  }
  if (isDemoReport(report)) {
    return {
      tone: 'info',
      title: 'You are viewing a demo receipt',
      detail: 'Run a real command from the composer to create an observed receipt with live output, revert snapshots, and exportable change history.',
    };
  }
  if (!state.revertStatus || !revertSnapshotsAvailable()) {
    return null;
  }
  const preview = state.revertStatus;
  if (preview.target_count && (preview.already_reverted || []).length === preview.target_count) {
    return {
      tone: 'success',
      title: 'Working directory reverted from this receipt',
      detail: 'All tracked file changes from this receipt now match their stored before-run state.',
    };
  }
  if (preview.target_count && (preview.safe || []).length === preview.target_count) {
    return {
      tone: 'info',
      title: 'Working directory still matches this receipt',
      detail: 'The tracked file changes from this receipt still match the current working directory.',
    };
  }
  if ((preview.conflicts || []).length) {
    return {
      tone: 'warning',
      title: 'Working directory diverged since this receipt',
      detail: `${preview.conflicts.length} file${preview.conflicts.length === 1 ? '' : 's'} changed again after the receipt finished.`,
    };
  }
  return {
    tone: 'info',
    title: 'Working directory partially matches this receipt',
    detail: 'Some tracked file changes from this receipt have been reverted or changed since the command finished.',
  };
}
