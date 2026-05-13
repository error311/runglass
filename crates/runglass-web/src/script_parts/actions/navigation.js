function bindActions() {
  if (state.bindingsReady) return;
  state.bindingsReady = true;

  document.getElementById('app').addEventListener('click', async (event) => {
    const target = event.target.closest('[data-nav-target], [data-copy], [data-file-path], [data-revert-toggle], [data-revert-conflict], [data-process-pid], [data-network-host], [data-network-port], [data-risk-id], [data-timeline-key], [data-help-kind], [data-help-close], [data-revert-close], [data-revert-apply], [data-revert-scope], [data-clear-network-focus], [data-stop-run], [data-tab-group], [data-diff-mode], [data-file-tab], [data-network-tab], [data-run-id], [data-delete-run], [data-load-more-runs], [data-toggle-quick-actions], [data-summary-action], [data-dismiss-run-output], [data-github-action]');
    if (!target) return;

    if (target.dataset.helpClose) {
      if (target.dataset.helpClose === 'overlay' && event.target !== target) {
        return;
      }
      state.openHelpKind = null;
      buildApp();
      return;
    }

    if (target.dataset.toggleQuickActions !== undefined) {
      event.preventDefault();
      state.quickActionsOpen = !state.quickActionsOpen;
      buildApp();
      return;
    }

    if (target.dataset.helpKind) {
      state.openHelpKind = decodeHtml(target.dataset.helpKind);
      buildApp();
      return;
    }

    if (target.dataset.revertClose) {
      if (target.dataset.revertClose === 'overlay' && event.target !== target) {
        return;
      }
      state.revertPreview = null;
      state.revertFiles = [];
      state.revertSkippedPaths = [];
      state.revertBusy = false;
      state.revertMessage = null;
      buildApp();
      return;
    }

    if (target.dataset.revertScope) {
      event.preventDefault();
      if (target.dataset.revertScope === 'single') {
        await openRevertPreview(state.selectedFilePath ? [state.selectedFilePath] : []);
      } else if (target.dataset.revertScope === 'selected') {
        await openRevertPreview(state.revertSelectedPaths);
      } else {
        await openRevertPreview([]);
      }
      return;
    }

    if (target.dataset.revertToggle) {
      event.preventDefault();
      toggleRevertPath(decodeHtml(target.dataset.revertToggle));
      renderFilesPanel();
      return;
    }

    if (target.dataset.revertConflict) {
      event.preventDefault();
      toggleRevertConflictPath(decodeHtml(target.dataset.revertConflict));
      buildApp();
      return;
    }

    if (target.dataset.revertApply) {
      event.preventDefault();
      await applyRevertAction(target.dataset.revertApply);
      return;
    }

    if (target.dataset.copy) {
      event.preventDefault();
      await handleCopy(target);
      return;
    }

    if (target.dataset.githubAction) {
      event.preventDefault();
      await handleGithubAction(target.dataset.githubAction);
      return;
    }

    if (target.dataset.runId) {
      event.preventDefault();
      await loadRun(target.dataset.runId);
      return;
    }

    if (target.dataset.deleteRun) {
      event.preventDefault();
      await deleteRun(target.dataset.deleteRun);
      return;
    }

    if (target.dataset.loadMoreRuns !== undefined) {
      event.preventDefault();
      state.recentRunsVisible += 5;
      renderRecentRuns();
      return;
    }

    if (target.dataset.summaryAction) {
      event.preventDefault();
      handleSummaryAction(target.dataset.summaryAction);
      return;
    }

    if (target.dataset.timelineKey) {
      state.selectedTimelineKey = decodeHtml(target.dataset.timelineKey);
      renderTimelinePanel();
    }

    if (target.dataset.riskId) {
      state.selectedRiskId = decodeHtml(target.dataset.riskId);
      state.summaryFocus = 'risks';
      renderSummary();
      renderRiskPanel();
    }

    if (target.dataset.stopRun !== undefined) {
      event.preventDefault();
      await cancelActiveRun();
      return;
    }

    if (target.dataset.dismissRunOutput !== undefined) {
      event.preventDefault();
      state.lastRunOutput = null;
      buildApp();
      return;
    }

    if (target.dataset.networkHost) {
      event.preventDefault();
      state.selectedRiskId = target.dataset.riskId ? decodeHtml(target.dataset.riskId) : null;
      state.summaryFocus = state.selectedRiskId ? 'risks' : state.summaryFocus;
      state.networkFocus = decodeHtml(target.dataset.networkHost);
      state.networkPortFocus = target.dataset.networkPort ? Number(target.dataset.networkPort) : null;
      state.networkTab = 'connections';
      renderNetworkPanel();
      renderSummary();
      renderRiskPanel();
      renderTimelinePanel();
      scrollToSection('network-panel');
      return;
    }

    if (target.dataset.clearNetworkFocus !== undefined) {
      event.preventDefault();
      state.networkFocus = null;
      state.networkPortFocus = null;
      renderNetworkPanel();
      return;
    }

    if (target.dataset.tabGroup === 'file') {
      state.fileTab = target.dataset.tabValue;
      renderFilesPanel();
      renderDiffPanel();
    }

    if (target.dataset.tabGroup === 'network') {
      state.networkTab = target.dataset.tabValue;
      renderNetworkPanel();
    }

    if (target.dataset.fileTab) {
      state.fileTab = target.dataset.fileTab;
      renderFilesPanel();
    }

    if (target.dataset.networkTab) {
      state.networkTab = target.dataset.networkTab;
      renderNetworkPanel();
    }

    if (target.dataset.filePath) {
      state.selectedRiskId = target.dataset.riskId ? decodeHtml(target.dataset.riskId) : null;
      state.summaryFocus = state.selectedRiskId ? 'risks' : state.summaryFocus;
      state.selectedFilePath = decodeHtml(target.dataset.filePath);
      state.selectedProcessPid = null;
      renderFilesPanel();
      renderSummary();
      renderRiskPanel();
      renderDiffPanel();
      renderTimelinePanel();
    }

    if (target.dataset.processPid) {
      state.selectedRiskId = null;
      state.selectedProcessPid = Number(target.dataset.processPid);
      state.summaryFocus = 'processes';
      renderSummary();
      renderProcessPanel();
      renderNetworkPanel();
      renderRiskPanel();
      renderTimelinePanel();
    }

    if (target.dataset.diffMode) {
      state.diffMode = target.dataset.diffMode;
      renderDiffPanel();
    }

    if (target.dataset.navTarget) {
      scrollToSection(target.dataset.navTarget);
    }
  });

  document.getElementById('app').addEventListener('input', (event) => {
    const target = event.target;
    if (target instanceof HTMLInputElement && target.dataset.runDraft !== undefined) {
      state.runDraft = target.value;
      state.runDraftSourceId = report.run.id;
      return;
    }
    if (target instanceof HTMLSelectElement && target.dataset.runMode !== undefined) {
      state.runMode = target.value === 'deep' ? 'deep' : 'normal';
      buildApp();
      return;
    }
    if (target instanceof HTMLInputElement && target.dataset.receiptSearch !== undefined) {
      state.receiptSearch = target.value;
      state.recentRunsVisible = 5;
      renderRecentRuns({ preserveSearchFocus: true });
      return;
    }
    if (target instanceof HTMLInputElement && target.dataset.githubRepo !== undefined) {
      state.githubRepo = target.value;
      state.githubPreview = null;
      state.githubMessage = null;
      return;
    }
    if (target instanceof HTMLInputElement && target.dataset.githubPr !== undefined) {
      state.githubPr = target.value;
      state.githubPreview = null;
      state.githubMessage = null;
    }
  });

  document.getElementById('app').addEventListener('submit', async (event) => {
    const target = event.target;
    if (!(target instanceof HTMLFormElement) || target.dataset.runForm === undefined) {
      return;
    }
    event.preventDefault();
    await submitRunCommand();
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && state.openHelpKind) {
      state.openHelpKind = null;
      buildApp();
    }
  });
}

function handleSummaryAction(action) {
  state.summaryFocus = action;
  state.selectedRiskId = null;

  if (action === 'files') {
    state.fileTab = 'all';
    state.networkFocus = null;
    state.networkPortFocus = null;
    state.selectedProcessPid = null;
    state.selectedFilePath = activeFiles()[0]?.path || null;
    renderSummary();
    renderFilesPanel();
    renderRiskPanel();
    renderDiffPanel();
    scrollToSection('files-panel');
    return;
  }

  if (action === 'processes') {
    state.selectedFilePath = null;
    state.selectedProcessPid = activeProcesses()[0]?.pid || null;
    renderSummary();
    renderProcessPanel();
    renderRiskPanel();
    scrollToSection('process-panel');
    return;
  }

  if (action === 'network-hosts') {
    state.networkTab = 'hosts';
    state.networkFocus = aggregateHosts(activeNetwork())[0]?.name || null;
    state.networkPortFocus = null;
    renderSummary();
    renderNetworkPanel();
    scrollToSection('network-panel');
    return;
  }

  if (action === 'network-ports') {
    state.networkTab = 'connections';
    const firstListening = activeNetwork().find((item) => item.direction === 'listening');
    state.networkFocus = firstListening ? (firstListening.host || firstListening.ip) : null;
    state.networkPortFocus = firstListening ? firstListening.port : null;
    renderSummary();
    renderNetworkPanel();
    scrollToSection('network-panel');
    return;
  }

  if (action === 'docker') {
    renderSummary();
    renderDockerPanel();
    scrollToSection('docker-panel');
    return;
  }

  if (action === 'config-files') {
    state.networkFocus = null;
    state.networkPortFocus = null;
    const configFile = activeFiles().find((file) =>
      file.risk_tags.includes('config') || file.path.includes('.config') || file.path.endsWith('rc')
    );
    const fallbackFile = activeFiles().find((file) => file.change_type === 'modified') || activeFiles()[0] || null;
    const targetFile = configFile || fallbackFile;
    state.fileTab = targetFile ? targetFile.change_type : 'all';
    state.selectedFilePath = targetFile?.path || null;
    renderSummary();
    renderFilesPanel();
    renderRiskPanel();
    renderDiffPanel();
    scrollToSection('files-panel');
    return;
  }

  if (action === 'risks') {
    const firstRisk = activeRisks()[0];
    if (firstRisk) {
      state.selectedRiskId = firstRisk.id;
      const target = riskPrimaryTarget(firstRisk) || { type: 'output' };
      if (target.type === 'file') {
        state.selectedFilePath = target.path;
      } else if (target.type === 'network') {
        state.networkTab = 'connections';
        state.networkFocus = target.host;
        state.networkPortFocus = target.port;
      }
    }
    renderSummary();
    renderFilesPanel();
    renderNetworkPanel();
    renderRiskPanel();
    renderDiffPanel();
    scrollToSection('risk-panel');
  }
}

async function loadRun(runId) {
  const next = await fetchJson(reportJsonUrl(runId));
  if (!next) return;
  report = next;
  resetViewState();
  buildApp();
  updateLocation(runId);
  void refreshRevertWorkspaceStatus();
}

async function deleteRun(runId) {
  const deletingActiveRun = runId === report.run.id;
  const response = await fetchJson(`/api/reports/${encodeURIComponent(runId)}`, { method: 'DELETE' }, true);
  if (!response) return;
  runs = await fetchJson('/runs.json') || runs.filter((item) => item.id !== runId);
  pushNotice('Receipt deleted.', 'success');
  if (deletingActiveRun) {
    const nextRun = sortRecentRuns(runs).find((item) => item.id !== runId);
    if (nextRun) {
      await loadRun(nextRun.id);
      return;
    }
  }
  renderRecentRuns();
}
