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
  } else if (target.dataset.copy === 'github-dry-run') {
    text = githubCurrentSnippets().dry_run;
    label = 'Copied Dry Run';
  } else if (target.dataset.copy === 'github-ci-auto') {
    text = githubCurrentSnippets().ci_auto;
    label = 'Copied CI Command';
  } else if (target.dataset.copy === 'github-workflow') {
    text = githubCurrentSnippets().workflow;
    label = 'Copied Workflow';
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

function githubCurrentSnippets() {
  return state.githubPreview?.snippets || githubFallbackSnippets(
    state.githubRepo || report.ci?.repository || '',
    state.githubPr || (report.ci?.pull_request ? String(report.ci.pull_request) : '')
  );
}

async function handleGithubAction(action) {
  if (action === 'preview') {
    await requestGithubPreview();
    return;
  }
  if (action === 'post') {
    if (!state.githubPreview) {
      await requestGithubPreview();
    }
    if (!window.confirm('Post or update the RunGlass PR comment for this receipt?')) {
      return;
    }
    await requestGithubPost();
  }
}

function githubPayload(confirm = false) {
  const ci = report.ci || {};
  const repo = (state.githubRepo || ci.repository || '').trim();
  const pr = (state.githubPr || (ci.pull_request ? String(ci.pull_request) : '')).trim();
  return {
    run_id: report.run.id,
    repo: repo || null,
    pr: pr ? Number(pr) : null,
    confirm,
  };
}

async function requestGithubPreview() {
  state.githubBusy = true;
  state.githubMessage = null;
  state.githubStatus = null;
  buildApp();
  const response = await fetchJson('/api/github/preview', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(githubPayload(false)),
  }, true);
  state.githubBusy = false;
  if (response) {
    state.githubPreview = response;
    state.githubRepo = response.context?.repo || state.githubRepo;
    state.githubPr = response.context?.pr ? String(response.context.pr) : state.githubPr;
    state.githubStatus = response.context?.can_post ? 'success' : 'warning';
    state.githubMessage = response.context?.can_post
      ? 'Preview ready. This receipt can be posted with the detected context.'
      : 'Preview ready. Posting still needs repo, PR, and an available GitHub token.';
  } else {
    state.githubStatus = 'error';
    state.githubMessage = 'GitHub preview failed.';
  }
  buildApp();
}

async function requestGithubPost() {
  state.githubBusy = true;
  state.githubMessage = 'Posting PR comment...';
  state.githubStatus = null;
  buildApp();
  const response = await fetchJson('/api/github/comment', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(githubPayload(true)),
  }, true);
  state.githubBusy = false;
  if (response) {
    state.githubStatus = 'success';
    state.githubMessage = `RunGlass PR comment ${response.action}.`;
  } else {
    state.githubStatus = 'error';
    state.githubMessage = 'GitHub comment failed. Check repo, PR, token, and workflow permissions.';
  }
  buildApp();
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
