async function openRevertPreview(files) {
  state.revertBusy = true;
  state.revertMessage = null;
  buildApp();
  const preview = await fetchJson(revertPreviewUrl(), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      run_id: report.run.id,
      files,
    }),
  });
  state.revertBusy = false;
  if (!preview) {
    state.revertPreview = null;
    state.revertFiles = [];
    state.revertMessage = 'Failed to build revert preview.';
    pushNotice(state.revertMessage, 'error');
    buildApp();
    return;
  }
  state.revertPreview = preview;
  state.revertFiles = files;
  state.revertSkippedPaths = (preview.conflicts || []).map((item) => item.path);
  pushNotice(
    preview.conflicts?.length
      ? `Revert preview ready. ${preview.conflicts.length} file${preview.conflicts.length === 1 ? '' : 's'} changed again since the receipt.`
      : `Revert preview ready for ${preview.target_count} file change${preview.target_count === 1 ? '' : 's'}.`,
    preview.conflicts?.length ? 'warning' : 'info'
  );
  buildApp();
}

async function applyRevertAction(_policy) {
  if (!state.revertPreview) return;
  const files = revertApplyPaths(state.revertPreview);
  const policy = revertIncludedConflictCount(state.revertPreview) ? 'force' : 'abort';
  state.revertBusy = true;
  buildApp();
  const result = await fetchJson(revertApplyUrl(), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      run_id: report.run.id,
      files,
      policy,
    }),
  });
  state.revertBusy = false;
  if (!result) {
    state.revertMessage = 'Failed to apply revert.';
    pushNotice(state.revertMessage, 'error');
    buildApp();
    return;
  }
  state.revertPreview = result;
  state.revertMessage = 'Supported file revert applied in the working directory. This receipt remains the original historical record.';
  await refreshRevertWorkspaceStatus();
  pushNotice('Supported file revert applied in the working directory.', 'success');
  buildApp();
}

async function initializeRuns() {
  runs = await fetchJson('/runs.json') || [];
  const requestedRunId = new URLSearchParams(window.location.search).get('run');
  if (requestedRunId && requestedRunId !== report.run.id) {
    const next = await fetchJson(reportJsonUrl(requestedRunId));
    if (next) {
      report = next;
      resetViewState();
    }
  }
  buildApp();
  void refreshRevertWorkspaceStatus();
}

async function refreshRevertWorkspaceStatus() {
  if (!revertSnapshotsAvailable()) {
    state.revertStatus = null;
    buildApp();
    return;
  }
  const preview = await fetchJson(revertPreviewUrl(), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      run_id: report.run.id,
      files: [],
    }),
  });
  if (!preview || preview.receipt_id !== report.run.id) {
    return;
  }
  state.revertStatus = preview;
  buildApp();
}
