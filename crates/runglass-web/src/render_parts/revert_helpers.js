function toggleRevertPath(path) {
  if (!revertSnapshotsAvailable()) return;
  if (revertPathIsSelected(path)) {
    state.revertSelectedPaths = state.revertSelectedPaths.filter((item) => item !== path);
    return;
  }
  state.revertSelectedPaths = [...state.revertSelectedPaths, path];
}

function revertConflictPathIsSkipped(path) {
  return state.revertSkippedPaths.includes(path);
}

function toggleRevertConflictPath(path) {
  if (revertConflictPathIsSkipped(path)) {
    state.revertSkippedPaths = state.revertSkippedPaths.filter((item) => item !== path);
    return;
  }
  state.revertSkippedPaths = [...state.revertSkippedPaths, path];
}

function revertPreviewStatuses(preview) {
  return [
    ...(preview.safe || []),
    ...(preview.conflicts || []),
    ...(preview.already_reverted || []),
    ...(preview.missing_artifacts || []),
  ];
}

function revertApplyPaths(preview) {
  return revertPreviewStatuses(preview)
    .filter((item) => item.status !== 'AlreadyReverted' && item.status !== 'MissingArtifacts')
    .filter((item) => !revertConflictPathIsSkipped(item.path))
    .map((item) => item.path);
}

function revertIncludedConflictCount(preview) {
  return (preview.conflicts || []).filter((item) => !revertConflictPathIsSkipped(item.path)).length;
}
