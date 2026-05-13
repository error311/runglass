function renderGithubPanel() {
  if (liveRunActive() || isDemoReport(report)) return '';
  const ci = report.ci || {};
  const repo = state.githubRepo || ci.repository || '';
  const pr = state.githubPr || (ci.pull_request ? String(ci.pull_request) : '');
  const preview = state.githubPreview;
  const context = preview?.context || null;
  const snippets = preview?.snippets || githubFallbackSnippets(repo, pr);
  const tokenLabel = context
    ? context.token_available
      ? `Token: ${context.token_source}`
      : 'Token: not detected'
    : 'Token: checked when preview runs';
  const statusClass = state.githubStatus === 'error' ? 'error' : state.githubStatus === 'success' ? 'success' : '';
  return `
    <section class="panel github-panel section-anchor" id="github-panel">
      <div class="panel-header has-actions">
        <div class="panel-title">
          <div class="panel-title-wrap">${icons.reports}<span>GitHub PR Receipt</span></div>
        </div>
        <div class="panel-header-actions github-actions-row">
          <button type="button" class="action-btn" data-copy="github-dry-run">Copy Dry Run</button>
          <button type="button" class="action-btn" data-copy="github-ci-auto">Copy CI Auto</button>
          <button type="button" class="action-btn" data-copy="github-workflow">Copy Workflow</button>
          ${context?.pr_url ? `<a class="action-btn" href="${escapeHtml(context.pr_url)}" target="_blank" rel="noreferrer">Open PR</a>` : ''}
          ${context?.run_url ? `<a class="action-btn" href="${escapeHtml(context.run_url)}" target="_blank" rel="noreferrer">Open CI Run</a>` : ''}
        </div>
      </div>
      <div class="section-body github-body">
        <div class="github-form-row">
          <label class="github-field">
            <span>Repository</span>
            <input type="text" value="${escapeHtml(repo)}" placeholder="owner/repo" data-github-repo />
          </label>
          <label class="github-field pr-field">
            <span>Pull Request</span>
            <input type="number" min="1" value="${escapeHtml(pr)}" placeholder="123" data-github-pr />
          </label>
          <div class="github-button-row">
            <button type="button" class="action-btn" data-github-action="preview" ${state.githubBusy ? 'disabled' : ''}>Preview Comment</button>
            <button type="button" class="action-btn primary" data-github-action="post" ${state.githubBusy ? 'disabled' : ''}>Post / Update</button>
          </div>
        </div>
        <div class="github-context-row">
          <span class="badge ${context?.can_post ? 'created' : 'modified'}">${escapeHtml(tokenLabel)}</span>
          <span class="badge normal">Receipt ${escapeHtml(report.run.id)}</span>
          <span class="github-note">Preview first. Posting uses server-side token discovery only; tokens are never entered in the browser.</span>
        </div>
        ${state.githubMessage ? `<div class="github-message ${statusClass}">${escapeHtml(state.githubMessage)}</div>` : ''}
        ${preview ? `
          <div class="github-preview-grid">
            <div class="github-preview">
              <div class="github-preview-title">PR Comment Preview</div>
              <pre>${escapeHtml(preview.body || '')}</pre>
            </div>
            <div class="github-snippets">
              <div class="github-preview-title">Copy Helpers</div>
              <button type="button" class="snippet-btn" data-copy="github-dry-run"><span>Local dry run</span><code>${escapeHtml(snippets.dry_run || '')}</code></button>
              <button type="button" class="snippet-btn" data-copy="github-ci-auto"><span>CI auto comment</span><code>${escapeHtml(snippets.ci_auto || '')}</code></button>
              <button type="button" class="snippet-btn" data-copy="github-workflow"><span>GitHub Actions workflow</span><code>docs/examples/github-actions-runglass-receipt.yml</code></button>
            </div>
          </div>
        ` : ''}
      </div>
    </section>
  `;
}

function githubFallbackSnippets(repo, pr) {
  const repoValue = repo || 'owner/repo';
  const prValue = pr || '123';
  return {
    dry_run: `runglass github comment --receipt ${report.run.id} --repo ${repoValue} --pr ${prValue} --dry-run`,
    post: `runglass github comment --receipt ${report.run.id} --repo ${repoValue} --pr ${prValue}`,
    ci_auto: 'runglass github comment --receipt runglass-receipt/receipt.json --auto',
    workflow: `name: RunGlass PR Receipt

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  issues: write
  pull-requests: write

jobs:
  receipt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install RunGlass
        run: cargo install runglass --locked
      - name: Run command with RunGlass
        run: runglass ci --provider github --output runglass-receipt -- npm test
      - name: Upload RunGlass receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: runglass-receipt
          path: runglass-receipt/
      - name: Comment RunGlass receipt on PR
        if: always() && github.event_name == 'pull_request' && !github.event.pull_request.head.repo.fork && hashFiles('runglass-receipt/receipt.json') != ''
        run: runglass github comment --receipt runglass-receipt/receipt.json --auto
        env:
          GITHUB_TOKEN: \${{ github.token }}`,
  };
}
