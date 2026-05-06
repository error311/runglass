function hasDockerChanges(docker) {
  if (!docker) return false;
  return [
    docker.containers_created,
    docker.containers_removed,
    docker.containers_changed,
    docker.images_pulled,
    docker.volumes_created,
    docker.networks_created,
    docker.ports_published,
  ].some((items) => Array.isArray(items) && items.length);
}

function countUniqueOutboundHosts(items) {
  return new Set(items.filter((item) => item.direction === 'outbound').map((item) => item.host || item.ip)).size;
}

function renderHostRows() {
  const hosts = aggregateHosts(activeNetwork());
  const max = Math.max(...hosts.map((host) => host.count), 1);
  return hosts.map((host) => `
    <button type="button" class="network-row clickable outbound ${state.networkFocus === host.name && !state.networkPortFocus ? 'selected' : ''}" data-network-host="${escapeHtml(host.name)}">
      <div>
        <div>${escapeHtml(host.name)}</div>
        <div class="network-bar"><span style="width:${Math.round((host.count / max) * 100)}%"></span></div>
      </div>
      <div>${host.count}</div>
    </button>
  `).join('');
}

function renderConnectionRows() {
  const items = filterNetworkItems();
  return items.map((item) => `
    <button type="button" class="network-row clickable ${escapeHtml(item.direction)} ${networkItemIsSelected(item) ? 'selected' : ''}" ${item.pid ? `data-process-pid="${item.pid}" data-nav-target="process-panel"` : ''}>
      <div>
        <div>${escapeHtml(item.host || item.ip)}:${item.port}</div>
        <div class="muted">${escapeHtml(item.direction)} · ${escapeHtml(item.process_name || 'unknown')}</div>
      </div>
      <div>${item.count}</div>
    </button>
  `).join('');
}
function dockerContainerTarget(container) {
  return parseContainerPublishedPort(container.ports && container.ports[0]);
}

function dockerContainerAttrs(container) {
  const target = dockerContainerTarget(container);
  if (!target) return '';
  return `data-network-host="${escapeHtml(target.host)}" data-network-port="${target.port}" data-nav-target="network-panel"`;
}

function parseContainerPublishedPort(value) {
  if (!value) return null;
  const match = /^([^:]+):(\d+)->(\d+)\/([a-z0-9]+)$/i.exec(String(value));
  if (!match) return null;
  return { host: match[1], port: Number(match[2]) };
}

function filterNetworkItems() {
  return activeNetwork().filter((item) => {
    const hostMatch = !state.networkFocus || (item.host || item.ip) === state.networkFocus;
    const portMatch = !state.networkPortFocus || item.port === state.networkPortFocus;
    return hostMatch && portMatch;
  });
}

function networkItemIsSelected(item) {
  if (state.selectedProcessPid && item.pid === state.selectedProcessPid) {
    return true;
  }
  return (!state.networkFocus || (item.host || item.ip) === state.networkFocus)
    && (!state.networkPortFocus || item.port === state.networkPortFocus)
    && Boolean(state.networkFocus || state.networkPortFocus);
}

function networkFocusLabel() {
  if (state.networkFocus && state.networkPortFocus) {
    return `${state.networkFocus}:${state.networkPortFocus}`;
  }
  if (state.networkFocus) {
    return state.networkFocus;
  }
  return String(state.networkPortFocus || '');
}

function aggregateHosts(items) {
  const map = new Map();
  items.filter((item) => item.direction === 'outbound').forEach((item) => {
    const key = item.host || item.ip;
    const current = map.get(key) || { name: key, count: 0 };
    current.count += item.count;
    map.set(key, current);
  });
  return [...map.values()].sort((a, b) => b.count - a.count);
}
