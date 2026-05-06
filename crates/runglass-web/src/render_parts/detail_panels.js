function renderDiffPanel() {
  const selected = selectedFile();
  const diffRows = selected && selected.diff ? parseUnifiedDiff(selected.diff.content) : [];

  document.getElementById('diff-panel').innerHTML = `
    ${panelHeader('Diff Viewer', icons.diff, diffControls(), 'files')}
    <div class="section-body">
      ${selected ? `
        <div class="file-detail-header">
          <span class="inline-icon">${icons.files}</span>
          <div>${escapeHtml(selected.path)}</div>
          <span class="badge ${changeTone(selected.change_type)}">${titleCase(selected.change_type)}</span>
        </div>
        ${selected.diff ? `
          ${state.diffMode === 'unified' ? renderUnifiedDiff(diffRows) : renderSideBySideDiff(diffRows)}
        ` : `<div class="muted">${liveRunActive() ? 'Live file selection found no verified text diff yet. The final receipt may add one when the command exits.' : 'No text diff is available for this file in the current receipt.'}</div>`}
      ` : renderEmptyState('No file selected', 'Select a file change to inspect its diff and revert options.', 'Use the file summary card to jump here.')}
    </div>
  `;
}

function renderOutputPanel() {
  const liveOutput = state.runPending && state.runJob && state.runJob.status === 'running'
    ? `
      <div>
        <div class="muted output-preview-label">Active Run Output</div>
        <pre>${renderTerminalOutput(state.runJob.stdout_preview || 'Waiting for stdout...')}</pre>
        ${state.runJob.stderr_preview ? `<pre>${renderTerminalOutput(state.runJob.stderr_preview)}</pre>` : ''}
      </div>
    `
    : '';
  document.getElementById('output-panel').innerHTML = `
    ${panelHeader('Receipt Output', icons.output, '', 'output')}
    <div class="section-body console">
      ${liveOutput}
      <pre>${renderTerminalOutput(report.stdout || 'No stdout was captured for this receipt.')}</pre>
      ${report.stderr ? `<pre>${renderTerminalOutput(report.stderr)}</pre>` : ''}
      ${renderSnapshotControlNotes()}
      ${report.limitations.length ? `
        <div class="limitations">
          <div class="limitations-title">Receipt Notes</div>
          <ul class="limitations-list">
            ${report.limitations.map((item) => `<li class="limitations-item">${escapeHtml(item)}</li>`).join('')}
          </ul>
        </div>
      ` : ''}
    </div>
  `;
}

function renderDockerPanel() {
  const docker = activeDocker();
  const section = document.getElementById('docker-panel');
  if (!section) return;

  if (!hasDockerChanges(docker)) {
    section.innerHTML = `
      ${panelHeader('Receipt Docker Changes', icons.environment, '', 'docker')}
      <div class="section-body">
        ${renderEmptyState(liveRunActive() ? 'Watching Docker' : 'No Docker changes', liveRunActive() ? 'Container, image, volume, and network changes will appear here if this command modifies Docker state.' : 'No Docker changes were captured for this receipt.', 'Docker collection is a before/after Engine diff.')}
      </div>
    `;
    return;
  }

  const containers = [...docker.containers_created, ...docker.containers_changed];
  const ports = docker.ports_published || [];
  const volumes = docker.volumes_created || [];
  const images = docker.images_pulled || [];

  section.innerHTML = `
    ${panelHeader('Receipt Docker Changes', icons.environment, '', 'docker')}
    <div class="section-body">
      <div class="files-list">
        ${containers.length ? containers.map((container) => `
          <button type="button" class="file-row docker ${dockerContainerTarget(container) ? 'clickable' : ''}" ${dockerContainerAttrs(container)}>
            <div>
              <div>${escapeHtml(container.name)}</div>
              <div class="muted">${escapeHtml(container.image)} · ${escapeHtml(container.state || 'unknown')}</div>
            </div>
            <span class="badge created">Container</span>
          </button>
        `).join('') : '<div class="muted">No containers created or changed.</div>'}
        ${ports.map((port) => `
          <button type="button" class="file-row clickable docker" data-network-host="${escapeHtml(port.host_ip || '0.0.0.0')}" data-network-port="${port.host_port}" data-nav-target="network-panel">
            <div>
              <div>${escapeHtml(port.host_ip || '0.0.0.0')}:${port.host_port} -&gt; ${port.container_port}/${escapeHtml(port.protocol)}</div>
              <div class="muted">Published port</div>
            </div>
            <span class="badge modified">Port</span>
          </button>
        `).join('')}
        ${images.map((image) => `
          <div class="file-row docker">
            <div>
              <div>${escapeHtml(image.tag)}</div>
              <div class="muted">${escapeHtml(image.digest || 'image pulled')}</div>
            </div>
            <span class="badge created">Image</span>
          </div>
        `).join('')}
        ${volumes.map((volume) => `
          <div class="file-row docker">
            <div>
              <div>${escapeHtml(volume.name)}</div>
              <div class="muted">${escapeHtml(volume.mountpoint || 'volume created')}</div>
            </div>
            <span class="badge created">Volume</span>
          </div>
        `).join('')}
      </div>
    </div>
  `;
}

function renderSnapshotControlNotes() {
  const snapshotNotes = (report.limitations || []).filter((item) => String(item).startsWith('Snapshot controls:') || String(item).includes('larger than'));
  if (!snapshotNotes.length) return '';
  return `
    <div class="snapshot-controls">
      <div>
        <div class="snapshot-controls-title">Snapshot Controls</div>
        <div class="snapshot-controls-copy">These settings affect which working-directory files can appear in the receipt.</div>
      </div>
      <div class="snapshot-control-list">
        ${snapshotNotes.map((item) => `<div class="snapshot-control-item">${escapeHtml(item.replace('Snapshot controls: ', ''))}</div>`).join('')}
      </div>
    </div>
  `;
}
