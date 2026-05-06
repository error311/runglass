async function submitRunCommand() {
  const command = state.runDraft.trim();
  if (!command || state.runPending) return;

  closeRunStream();
  state.runPending = true;
  state.runJob = null;
  state.runError = null;
  state.lastRunOutput = null;
  state.runDraftSourceId = report.run.id;
  buildApp();

  const job = await fetchJson('/api/run', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ command, mode: state.runMode }),
  }, true);

  if (!job) {
    state.runPending = false;
    buildApp();
    return;
  }

  state.runJob = job;
  pushNotice(
    `Started ${job.mode === 'deep' ? 'deep' : 'normal'} receipt for: ${job.command}`,
    'info'
  );
  buildApp();
  openRunStream(job.id);
}

async function cancelActiveRun() {
  if (!(state.runPending && state.runJob && state.runJob.id)) return;
  const next = await fetchJson(cancelJobUrl(state.runJob.id), { method: 'POST' }, true);
  if (!next) {
    buildApp();
    return;
  }
  state.runJob = next;
  buildApp();
}

function closeRunStream() {
  if (!state.runEventSource) return;
  state.runEventSource.close();
  state.runEventSource = null;
}

function openRunStream(jobId) {
  closeRunStream();
  if (!window.EventSource) {
    pollRunJob(jobId);
    return;
  }

  const stream = new EventSource(jobEventsUrl(jobId));
  state.runEventSource = stream;

  stream.onmessage = async (event) => {
    if (state.runEventSource !== stream) return;
    let job;
    try {
      job = JSON.parse(event.data);
    } catch (_) {
      return;
    }
    await applyRunJobUpdate(job);
  };

  stream.onerror = () => {
    if (state.runEventSource !== stream) return;
    closeRunStream();
    if (!(state.runPending && state.runJob && state.runJob.id === jobId)) return;
    window.setTimeout(() => {
      if (state.runPending && state.runJob && state.runJob.id === jobId) {
        pollRunJob(jobId);
      }
    }, 250);
  };
}

async function applyRunJobUpdate(job) {
  if (!state.runJob || state.runJob.id !== job.id) {
    return;
  }

  state.runJob = job;
  if (job.status === 'running' || job.status === 'cancelling') {
    buildApp();
    return;
  }

  closeRunStream();

  if (job.status === 'failed') {
    state.runPending = false;
    state.lastRunOutput = buildRunOutputSummary({
      status: 'failed',
      command: job.command,
      error: job.error || 'Run failed.',
      preview: `${job.stdout_preview || ''}${job.stderr_preview ? `\n\n[stderr]\n${job.stderr_preview}` : ''}`.trim(),
    });
    state.runJob = null;
    state.runError = job.error || 'Run failed.';
    pushNotice(state.runError, 'error');
    buildApp();
    return;
  }

  if (job.status !== 'completed' && job.status !== 'cancelled') {
    state.runPending = false;
    state.runJob = null;
    state.runError = `Unknown run status: ${job.status}`;
    pushNotice(state.runError, 'error');
    buildApp();
    return;
  }

  const next = await fetchJson(reportJsonUrl(job.run_id), undefined, true);
  if (!next) {
    state.runPending = false;
    state.runJob = null;
    if (!state.runError) {
      state.runError = 'Run finished, but the receipt could not be loaded.';
    }
    buildApp();
    return;
  }

  report = next;
  runs = await fetchJson('/runs.json') || runs;
  state.runPending = false;
  state.lastRunOutput = buildRunOutputSummary({
    status: job.status,
    command: job.command,
    preview: `${next.stdout || ''}${next.stderr ? `\n\n[stderr]\n${next.stderr}` : ''}`.trim(),
    runId: next.run.id,
  });
  state.runJob = null;
  resetViewState();
  syncRunDraft();
  pushNotice(
    job.status === 'cancelled'
      ? 'Receipt stopped. Partial observations were saved.'
      : `Receipt ready for: ${report.run.command_display}`,
    job.status === 'cancelled' ? 'warning' : 'success'
  );
  buildApp();
  updateLocation(report.run.id);
  void refreshRevertWorkspaceStatus();
}

async function pollRunJob(jobId) {
  const job = await fetchJson(jobStatusUrl(jobId), undefined, true);
  if (!job) {
    closeRunStream();
    state.runPending = false;
    state.lastRunOutput = buildRunOutputSummary({
      status: 'failed',
      command: state.runDraft,
      error: 'Lost contact with the local run job.',
      preview: state.runJob ? `${state.runJob.stdout_preview || ''}${state.runJob.stderr_preview ? `\n\n[stderr]\n${state.runJob.stderr_preview}` : ''}`.trim() : '',
    });
    state.runJob = null;
    if (!state.runError) {
      state.runError = 'Lost contact with the local run job.';
      pushNotice(state.runError, 'error');
    }
    buildApp();
    return;
  }

  if (!state.runJob || state.runJob.id !== jobId) {
    return;
  }

  await applyRunJobUpdate(job);
  if (job.status === 'running' || job.status === 'cancelling') {
    window.setTimeout(() => pollRunJob(jobId), 250);
  }
}
