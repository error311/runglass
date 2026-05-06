function renderSummary() {
  const summary = activeSummary();
  const files = activeFiles();
  const risks = activeRisks();
  const configFiles = files.filter((file) => file.risk_tags.includes('config') || file.path.includes('.config') || file.path.endsWith('rc')).length;
  const processes = activeProcesses();
  const network = activeNetwork();
  const processesSeen = liveRunActive() && state.runJob
    ? (state.runJob.processes_seen ?? processes.length)
    : summary.processes_seen;
  const spawnedProcesses = processes.filter((process) => process.ppid !== null).length;
  const exitedProcesses = liveRunActive() ? 0 : report.processes.filter((process) => process.exited_at).length;
  const outboundConnections = network.filter((item) => item.direction === 'outbound').reduce((sum, item) => sum + item.count, 0);
  const listeningPorts = liveRunActive() && state.runJob
    ? (state.runJob.ports_opened ?? network.filter((item) => item.direction === 'listening').length)
    : report.network.filter((item) => item.direction === 'listening').length;
  const networkHosts = liveRunActive() && state.runJob
    ? (state.runJob.network_hosts ?? countUniqueOutboundHosts(network))
    : summary.network_hosts;
  const firstFile = files[0];
  const firstProcess = processes[0];
  const firstHost = aggregateHosts(network)[0];
  const firstListening = network.find((item) => item.direction === 'listening');
  const firstConfig = files.find((file) =>
    file.risk_tags.includes('config') || file.path.includes('.config') || file.path.endsWith('rc')
  );
  const firstRisk = risks[0];
  const cards = [
    {
      action: 'files',
      icon: icons.folder,
      accent: '#6e63ff',
      value: summary.files_changed,
      label: 'Files Changed',
      metric: `${summary.files_created} created, ${summary.files_modified} modified`,
      hint: firstFile ? `Open ${firstFile.path}` : 'Open files panel',
    },
    {
      action: 'processes',
      icon: icons.process,
      accent: '#59db88',
      value: processesSeen,
      label: 'Processes',
      metric: liveRunActive() ? `${spawnedProcesses} observed child processes` : `${spawnedProcesses} child processes, ${exitedProcesses} observed exits`,
      hint: firstProcess ? `Focus ${firstProcess.command} (${firstProcess.pid})` : 'Open process tree',
    },
    {
      action: 'network-hosts',
      icon: icons.network,
      accent: '#4ed2ff',
      value: networkHosts,
      label: 'Network Hosts',
      metric: `${outboundConnections} observed outbound connections`,
      hint: firstHost ? `Filter to ${firstHost.name}` : 'Open network activity',
    },
    {
      action: 'network-ports',
      icon: icons.environment,
      accent: '#ff9d57',
      value: listeningPorts,
      label: 'Ports Opened',
      metric: `${listeningPorts} observed listeners`,
      hint: firstListening ? `Focus ${firstListening.host || firstListening.ip}:${firstListening.port}` : 'Open listening sockets',
    },
    {
      action: 'config-files',
      icon: icons.settings,
      accent: '#f0bf4b',
      value: configFiles,
      label: 'Config Files',
      metric: `${summary.files_modified} modified files in receipt`,
      hint: firstConfig ? `Open ${firstConfig.path}` : 'Open config-related changes',
    },
    {
      action: 'risks',
      icon: icons.shield,
      accent: '#74a1ff',
      value: titleCase(summary.risk_level),
      label: 'Risk Level',
      metric: `${risks.length} notes generated`,
      hint: firstRisk ? `Review ${firstRisk.title}` : 'Open risk summary',
    },
  ];

  document.getElementById('summary-grid').innerHTML = cards.map((card) => `
    <button type="button" class="panel card summary-card ${summaryCardIsActive(card.action) ? 'active' : ''}" data-summary-action="${card.action}" title="${escapeHtml(summaryCardHelp(card.action))}">
      <div class="card-top">
        <div class="card-icon" style="color:${card.accent}; background:${hexToRgba(card.accent, 0.16)}">${card.icon}</div>
        <div>
          <div class="card-value">${card.value}</div>
          <div class="card-label">${card.label}</div>
          <div class="card-metric">${escapeHtml(card.metric)}</div>
        </div>
      </div>
      <div class="card-sub"><strong>Opens</strong>${escapeHtml(card.hint)}</div>
    </button>
  `).join('');
}

function receiptEyebrow() {
  return liveRunActive() ? 'Live Receipt Building...' : 'Final Receipt';
}

function receiptStatusLabel() {
  if (liveRunActive()) return 'Running';
  if (report.run.status === 'interrupted') return 'Interrupted';
  return titleCase(report.run.status);
}

function receiptSubheadline() {
  if (liveRunActive()) {
    return 'Some details may update when the command exits.';
  }
  const exitCode = report.run.exit_code ?? 'n/a';
  const duration = formatDuration(report.run.duration_ms || 0);
  return `Command exited ${exitCode} after ${duration}`;
}

function renderProcessPanel() {
  const roots = buildTree(activeProcesses());
  document.getElementById('process-panel').innerHTML = `
    ${panelHeader('Receipt Process Tree', icons.tree, '', 'processes')}
    <div class="section-body">
      ${roots.length ? `<div class="process-tree">${roots.map(renderNode).join('')}</div>` : renderEmptyState(liveRunActive() ? 'Waiting for process observations' : 'No process observations', liveRunActive() ? 'RunGlass is polling /proc while this command runs.' : 'No process observations were captured for this receipt.', activeObservationMode() === 'deep' ? 'Deep mode can improve short-lived process visibility.' : 'Normal mode may miss very short-lived processes.')}
    </div>
  `;
}

function renderNode(node) {
  const tag = node.command.includes('curl') ? '<span class="badge network">network</span>' : node.command.includes('systemctl') ? '<span class="badge service">service</span>' : '';
  return `
    <div class="tree-node">
      <button type="button" class="tree-item clickable ${state.selectedProcessPid === node.pid ? 'selected' : ''}" data-process-pid="${node.pid}">
        ${icons.files}<span>${escapeHtml(node.command)}</span><span class="muted">pid ${node.pid}</span>${tag}
      </button>
      ${node.children.length ? `<div class="tree-children">${node.children.map(renderNode).join('')}</div>` : ''}
    </div>
  `;
}

function renderFilesPanel() {
  const files = filterFiles();
  document.getElementById('files-panel').innerHTML = `
    ${panelHeader('Receipt Files', icons.folder, filePanelControls(), 'files')}
    <div class="section-body">
      ${files.length ? `<div class="files-list">
        ${files.map((file) => {
          const workspaceStatus = currentWorkspaceFileStatus(file.path);
          return `
          <button type="button" class="file-row clickable ${changeTone(file.change_type)} ${file.path === state.selectedFilePath ? 'selected' : ''}" data-file-path="${escapeHtml(file.path)}">
            <div class="file-row-main">
              ${revertSnapshotsAvailable() ? `<span class="file-select ${revertPathIsSelected(file.path) ? 'active' : ''}" data-revert-toggle="${escapeHtml(file.path)}" aria-label="${revertPathIsSelected(file.path) ? 'Remove from revert selection' : 'Add to revert selection'}"></span>` : ''}
              <div>
                <div>${escapeHtml(file.path)}</div>
                ${file.risk_tags.length ? `<div class="muted">${file.risk_tags.map(escapeHtml).join(' · ')}</div>` : ''}
                ${workspaceStatus ? `<div class="muted">${escapeHtml(workspaceStatus.text)}</div>` : ''}
              </div>
            </div>
            <span class="badge ${changeTone(file.change_type)}">${titleCase(file.change_type)}</span>
          </button>
        `;
        }).join('')}
      </div>
      ${revertStatusNote() ? `<div class="file-select-note">${escapeHtml(revertStatusNote())}</div>` : ''}` : renderEmptyState(liveRunActive() ? 'Watching for file changes' : 'No matching file changes', liveRunActive() ? 'RunGlass is watching the working directory while this command runs.' : 'No file changes matched the current filter for this receipt.', state.fileTab === 'all' ? 'Try another command or review snapshot notes.' : 'Switch back to All to see every file change.')}
    </div>
  `;
}

function renderNetworkPanel() {
  const body = state.networkTab === 'hosts' ? renderHostRows() : renderConnectionRows();
  const right = `
    <div class="panel-subactions">
      ${networkTabs()}
      ${(state.networkFocus || state.networkPortFocus) ? `<button type="button" class="text-link clickable" data-clear-network-focus="true">Clear Filter</button>` : ''}
    </div>
  `;
  document.getElementById('network-panel').innerHTML = `
    ${panelHeader('Receipt Network Activity', `<span class="network-mini-icon">${icons.network}</span>`, right, 'network')}
    <div class="section-body">
      ${(state.networkFocus || state.networkPortFocus) ? `<div class="muted network-filter-note">Filtered to ${escapeHtml(networkFocusLabel())}</div>` : ''}
      ${body ? `<div class="network-list">${body}</div>` : renderEmptyState(liveRunActive() ? 'Watching sockets' : 'No network activity', liveRunActive() ? 'Best-effort socket attribution will appear here while the command runs.' : 'No network activity was observed for this receipt.', activeObservationMode() === 'deep' ? 'Deep mode traces connect and bind calls.' : 'Normal mode uses /proc and ss sampling.')}
    </div>
  `;
}

function renderRiskPanel() {
  const summary = activeSummary();
  const risks = activeRisks();
  const tone = titleCase(summary.risk_level);
  document.getElementById('risk-panel').innerHTML = `
    ${panelHeader('Receipt Summary', icons.shield, '', 'risks')}
    <div class="section-body">
      ${risks.length ? `<div class="risk-list">
        ${risks.map((risk) => `
          <button type="button" class="risk-row ${statusTone(risk.severity)} ${riskRowIsNavigable(risk) ? 'clickable' : ''} ${riskRowIsSelected(risk) ? 'selected' : riskRowIsLinked(risk) ? 'linked' : ''}" data-risk-id="${escapeHtml(risk.id)}" ${riskRowAttrs(risk)}>
            <span class="risk-icon ${statusTone(risk.severity)}"></span>
            <div class="risk-copy">
              <div class="risk-title">${escapeHtml(risk.title)}</div>
              <div class="risk-detail">${escapeHtml(riskPrimaryDetail(risk))}</div>
            </div>
            <div class="row-side">
              <span class="severity-pill ${statusTone(risk.severity)}">${titleCase(risk.severity)}</span>
              <div class="row-linkhint">${escapeHtml(riskNavigationHint(risk))}</div>
            </div>
          </button>
        `).join('')}
      </div>` : renderEmptyState(liveRunActive() ? 'No risks yet' : 'No notable risks', liveRunActive() ? 'RunGlass will add notes here if it sees sensitive files, ports, Docker changes, or failures.' : 'RunGlass did not flag sensitive files, public ports, Docker exposure, or failed exit status.', 'Review the raw receipt if you need a full audit.')}
      <div class="risk-footer">
        <div class="muted">Risk Level:</div>
        <span class="badge modified">${tone}</span>
      </div>
    </div>
  `;
}

function renderTimelinePanel() {
  const events = activeTimelineEvents();
  document.getElementById('timeline-panel').innerHTML = `
    ${panelHeader('Receipt Timeline', icons.timeline, '', 'timeline')}
    <div class="section-body timeline-wrap">
      ${events.length ? `<div class="timeline-stream"><div class="timeline-list">
        ${events.map((event) => `
          <button type="button" class="timeline-row ${statusTone(event.severity)} ${timelineEventIsSelected(event) ? 'selected' : timelineEventIsLinked(event) ? 'linked' : ''} ${timelineEventIsNavigable(event) ? 'clickable' : ''}" ${timelineEventAttrs(event)}>
            <div class="timeline-dot" style="background:${severityColor(event.severity)}"></div>
            <div class="timeline-copy">
              <div class="timeline-meta">
                <div class="muted">${formatClock(event.at)}</div>
                <span class="severity-pill ${statusTone(event.severity)}">${escapeHtml(timelineKindLabel(event.kind))}</span>
              </div>
              <div class="timeline-title">${escapeHtml(event.title)}</div>
              ${event.detail ? `<div class="timeline-detail">${escapeHtml(event.detail)}</div>` : ''}
            </div>
            <div class="row-side">${timelineEventIsNavigable(event) ? `<div class="row-linkhint">${escapeHtml(timelineNavigationHint(event))}</div>` : ''}</div>
          </button>
        `).join('')}
      </div></div>` : renderEmptyState(liveRunActive() ? 'Building timeline' : 'No timeline events', liveRunActive() ? 'Process, file, network, and Docker milestones will appear here.' : 'No timeline events were captured for this receipt.', 'Command start and exit events should appear once the receipt settles.')}
    </div>
  `;
}
