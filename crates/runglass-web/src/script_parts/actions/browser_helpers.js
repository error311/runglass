async function handleCopy(target) {
  let text = '';
  let label = 'Copied';

  if (target.dataset.copy === 'command') {
    text = report.run.argv.join(' ');
    label = 'Copied Command';
  } else if (target.dataset.copy === 'path') {
    text = selectedFile()?.path || '';
    label = 'Copied Path';
  } else if (target.dataset.copy === 'report-id') {
    text = report.run.id;
    label = 'Copied Receipt ID';
  }

  if (!text) return;

  try {
    await navigator.clipboard.writeText(text);
    flashLabel(target, label);
    pushNotice(label, 'success');
  } catch (_) {
    flashLabel(target, text);
    pushNotice('Clipboard copy failed in this browser session.', 'error');
  }
}

function flashLabel(target, text) {
  const labelNode = target.querySelector('span:last-child') || target;
  const original = labelNode.textContent;
  labelNode.textContent = text;
  window.setTimeout(() => {
    labelNode.textContent = original;
  }, 1400);
}

function scrollToSection(id) {
  const node = document.getElementById(id);
  if (node) {
    node.classList.remove('focused-panel');
    window.requestAnimationFrame(() => {
      node.classList.add('focused-panel');
      window.setTimeout(() => {
        node.classList.remove('focused-panel');
      }, 1400);
    });
    node.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
}

async function fetchJson(url, options, setRunError = false) {
  try {
    const response = await fetch(url, options);
    if (!response.ok) {
      let detail = '';
      try {
        const payload = await response.json();
        detail = payload.error ? ` ${payload.error}` : '';
      } catch (_) {
      }
      throw new Error(`Request failed: ${response.status}.${detail}`);
    }
    return await response.json();
  } catch (error) {
    console.error(error);
    if (setRunError) {
      state.runError = error instanceof Error ? error.message : 'Request failed.';
      pushNotice(state.runError, 'error');
    }
    return null;
  }
}

function pushNotice(message, tone = 'info') {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  state.notices = [...state.notices, { id, message, tone }].slice(-4);
  buildApp();
  window.setTimeout(() => dismissNotice(id), 3200);
}

function dismissNotice(id) {
  const next = state.notices.filter((notice) => notice.id !== id);
  if (next.length === state.notices.length) return;
  state.notices = next;
  buildApp();
}
