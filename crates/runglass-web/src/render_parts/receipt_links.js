function timelineEventIsNavigable(event) {
  return Boolean(event.related_path || event.related_pid || event.kind.startsWith('command_'));
}

function timelineEventIsSelected(event) {
  return state.selectedTimelineKey === timelineEventKey(event);
}

function timelineEventIsLinked(event) {
  return Boolean(
    (event.related_path && event.related_path === state.selectedFilePath)
    || (event.related_pid && event.related_pid === state.selectedProcessPid)
  );
}

function timelineEventAttrs(event) {
  const attrs = [];
  attrs.push(`data-timeline-key="${escapeHtml(timelineEventKey(event))}"`);
  if (event.kind.startsWith('command_')) {
    attrs.push('data-nav-target="output-panel"');
  } else if (event.related_path) {
    attrs.push(`data-file-path="${escapeHtml(event.related_path)}"`);
    attrs.push('data-nav-target="diff-panel"');
  } else if (event.related_pid) {
    attrs.push(`data-process-pid="${event.related_pid}"`);
    attrs.push('data-nav-target="process-panel"');
  }
  return attrs.join(' ');
}

function timelineEventKey(event) {
  return `${event.at}|${event.kind}|${event.title}`;
}

function timelineKindLabel(kind) {
  return titleCase(String(kind || '').replace(/^command_/, 'command ').replace(/^process_/, 'process ').replace(/^docker_/, 'docker ').replace(/^file_/, 'file ').replace(/^port_/, 'network '));
}

function timelineNavigationHint(event) {
  if (event.kind.startsWith('command_')) {
    return 'Open Output';
  }
  if (event.related_path) {
    return 'Open Diff';
  }
  if (event.related_pid) {
    return 'Open Process';
  }
  return 'Open';
}

function riskRowIsNavigable(risk) {
  return true;
}

function riskRowIsSelected(risk) {
  return state.selectedRiskId === risk.id;
}

function riskRowIsLinked(risk) {
  const target = riskPrimaryTarget(risk);
  if (!target) return false;
  if (target.type === 'file') {
    return target.path === state.selectedFilePath;
  }
  if (target.type === 'network') {
    return state.networkFocus === target.host && state.networkPortFocus === target.port;
  }
  return false;
}

function riskRowAttrs(risk) {
  const target = riskPrimaryTarget(risk) || { type: 'output' };
  if (target.type === 'file') {
    return `data-file-path="${escapeHtml(target.path)}" data-nav-target="diff-panel"`;
  }
  if (target.type === 'network') {
    return `data-network-host="${escapeHtml(target.host)}" data-network-port="${target.port}" data-nav-target="network-panel"`;
  }
  if (target.type === 'output') {
    return 'data-nav-target="output-panel"';
  }
  return '';
}

function riskPrimaryDetail(risk) {
  const primary = (risk.evidence || [])[0];
  if (primary?.path) {
    return primary.path;
  }
  if (primary?.value) {
    return primary.value;
  }
  if (risk.recommendation) {
    return risk.recommendation;
  }
  return risk.detail;
}

function riskNavigationHint(risk) {
  const target = riskPrimaryTarget(risk) || { type: 'output' };
  if (target.type === 'file') {
    return 'Open Diff';
  }
  if (target.type === 'network') {
    return 'Open Network';
  }
  return 'Open Output';
}

function riskPrimaryTarget(risk) {
  for (const evidence of risk.evidence || []) {
    if (evidence.path) {
      return { type: 'file', path: evidence.path };
    }
    if (evidence.kind === 'network_port' || evidence.kind === 'docker_port') {
      const portTarget = parseEvidencePortTarget(evidence.value);
      if (portTarget) return portTarget;
    }
    if (evidence.kind === 'exit_code') {
      return { type: 'output' };
    }
  }
  return null;
}

function parseEvidencePortTarget(value) {
  const dockerMatch = /^([^:]+):(\d+)->(\d+)\/([a-z0-9]+)$/i.exec(String(value));
  if (dockerMatch) {
    return { type: 'network', host: dockerMatch[1], port: Number(dockerMatch[2]) };
  }
  const networkMatch = /^([^:]+):(\d+)\/([a-z0-9]+)$/i.exec(String(value));
  if (networkMatch) {
    return { type: 'network', host: networkMatch[1], port: Number(networkMatch[2]) };
  }
  return null;
}
