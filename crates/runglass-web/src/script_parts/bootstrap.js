let report = JSON.parse(document.getElementById('report-data').textContent);
let runs = [];
const state = {
  fileTab: 'all',
  networkTab: 'hosts',
  networkFocus: null,
  networkPortFocus: null,
  diffMode: 'unified',
  selectedFilePath: null,
  selectedProcessPid: null,
  selectedRiskId: null,
  selectedTimelineKey: null,
  summaryFocus: null,
  openHelpKind: null,
  revertPreview: null,
  revertSelectedPaths: [],
  revertSkippedPaths: [],
  revertFiles: [],
  revertBusy: false,
  revertMessage: null,
  revertStatus: null,
  runDraft: '',
  runDraftSourceId: null,
  runMode: 'normal',
  runJob: null,
  runEventSource: null,
  runPending: false,
  runError: null,
  lastRunOutput: null,
  notices: [],
  bindingsReady: false,
  receiptSearch: '',
  recentRunsVisible: 5,
  quickActionsOpen: false,
};

const icons = {
  overview: icon('M3 12l9-8 9 8v8a2 2 0 0 1-2 2h-4v-6H9v6H5a2 2 0 0 1-2-2z'),
  process: icon('M4 5h6v6H4zM14 5h6v6h-6zM9 14h6v6H9zM7 8h8M12 11v3'),
  files: icon('M7 3h7l5 5v13a1 1 0 0 1-1 1H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm6 1v4h4'),
  network: icon('M12 3a9 9 0 1 0 9 9 9 9 0 0 0-9-9zm0 0c2.3 2.3 3.6 5.6 3.6 9S14.3 18.7 12 21c-2.3-2.3-3.6-5.6-3.6-9S9.7 5.3 12 3zm-8.2 9h16.4M5.8 7.2h12.4M5.8 16.8h12.4'),
  environment: icon('M4 7h8v10H4zM12 11h8v6h-8zM10 7l4-4 4 4'),
  diff: icon('M8 5v14M16 5v14M4 9h8M12 15h8'),
  timeline: icon('M12 8v5l3 2M12 3a9 9 0 1 0 9 9 9 9 0 0 0-9-9z'),
  output: icon('M4 6h16v12H4zm3 3h5m-5 3h10'),
  reports: icon('M7 4h10l3 3v13a1 1 0 0 1-1 1H7a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm8 0v4h4'),
  trash: icon('M4 7h16M10 11v6M14 11v6M6 7l1 14h10l1-14M9 7V4h6v3'),
  settings: icon('M12 8.5A3.5 3.5 0 1 1 8.5 12 3.5 3.5 0 0 1 12 8.5zm8 3.5-2 .5a7.6 7.6 0 0 1-.7 1.7l1.1 1.7-1.7 1.7-1.7-1.1a7.6 7.6 0 0 1-1.7.7L12 20l-1.5-2a7.6 7.6 0 0 1-1.7-.7L7.1 18.4 5.4 16.7l1.1-1.7a7.6 7.6 0 0 1-.7-1.7L4 12l2-.5a7.6 7.6 0 0 1 .7-1.7L5.4 8.1l1.7-1.7 1.7 1.1a7.6 7.6 0 0 1 1.7-.7L12 4l1.5 2a7.6 7.6 0 0 1 1.7.7l1.7-1.1 1.7 1.7-1.1 1.7a7.6 7.6 0 0 1 .7 1.7z'),
  play: icon('M8 6l10 6-10 6z'),
  download: icon('M12 3v12m0 0 4-4m-4 4-4-4M5 19h14'),
  tree: icon('M6 3h4v4H6zM14 17h4v4h-4zM14 3h4v4h-4zM8 7v5a2 2 0 0 0 2 2h6M16 7v10'),
  shield: icon('M12 3l7 3v5c0 5.2-3 8.7-7 10-4-1.3-7-4.8-7-10V6z'),
  folder: icon('M3 7h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z'),
  globe: icon('M12 3a9 9 0 1 0 9 9 9 9 0 0 0-9-9zm0 0c2.3 2.3 3.6 5.6 3.6 9S14.3 18.7 12 21c-2.3-2.3-3.6-5.6-3.6-9S9.7 5.3 12 3z'),
};

function icon(path) {
  return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="${path}" /></svg>`;
}

function statusTone(severity) {
  if (severity === 'danger') return 'danger';
  if (severity === 'warning') return 'warning';
  if (severity === 'success') return 'success';
  return 'info';
}

function changeTone(changeType) {
  if (changeType === 'created') return 'created';
  if (changeType === 'deleted') return 'deleted';
  return 'modified';
}
