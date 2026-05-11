# CI Receipt Workflows

RunGlass CI mode is for commands that should leave a durable receipt in a remote runner: tests, install scripts, deploy previews, Docker Compose checks, and agent-driven code changes.

```bash
runglass ci --provider github --output runglass-receipt -- npm test
```

RunGlass writes artifacts before returning the wrapped command's exit code. A failing command still fails the job, but the receipt directory remains available for upload.

## Stable Output Layout

`runglass ci` writes a stable directory layout:

```text
runglass-receipt/
  receipt.html
  receipt.md
  receipt.json
  summary.md
  ai-summary.txt
  reverse.patch
  bundle.tar
  artifacts/
    stdout.txt
    stderr.txt
    metadata.json
    diffs/
    file-snapshots/
```

`receipt.html` is the human-readable receipt. `summary.md` is compact enough for a CI job summary or PR comment. `ai-summary.txt` is deterministic text for handing receipt context back to a coding agent. `bundle.tar` contains the receipt files in a portable archive.

`reverse.patch` and `artifacts/file-snapshots/` support RunGlass file-revert workflows when the touched files were captured within snapshot limits. They do not undo Docker changes, network calls, database writes, external service mutations, package manager global changes, or commands run outside the watched working directory.

## GitHub Actions

Use the example at [`docs/examples/github-actions-runglass-receipt.yml`](examples/github-actions-runglass-receipt.yml).

The important pieces are:

```yaml
- name: Run command with RunGlass
  run: runglass ci --provider github --output runglass-receipt -- npm test

- name: Upload RunGlass receipt
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: runglass-receipt
    path: runglass-receipt/

- name: Comment RunGlass receipt on PR
  if: always() && github.event_name == 'pull_request'
  run: runglass github comment --receipt runglass-receipt/receipt.json --auto
  env:
    GITHUB_TOKEN: ${{ github.token }}
```

The PR comment API does not attach files directly. The workflow uploads `runglass-receipt/` as a GitHub Actions artifact, and the comment points reviewers to the CI run where that artifact lives.

RunGlass dogfoods this pattern in [`.github/workflows/runglass-receipt.yml`](../.github/workflows/runglass-receipt.yml). Pull requests run the workspace tests through `runglass ci`, upload the receipt directory, and update one RunGlass PR comment when the workflow has permission to write issue comments.

If GitHub returns `Resource not accessible by integration`, the receipt artifact is still the source of truth. The PR comment token did not receive comment-write permission, commonly because the workflow is running from a fork or repository Actions settings restrict write permissions.

## GitLab CI

Use the example at [`docs/examples/gitlab-runglass-receipt.yml`](examples/gitlab-runglass-receipt.yml).

GitLab support focuses on writing and publishing the receipt artifact directory:

```yaml
artifacts:
  when: always
  expire_in: 14 days
  paths:
    - runglass-receipt/
```

## Local Smoke Test

```bash
cargo run -- ci --output /tmp/runglass-receipt -- bash -lc 'echo ci && touch ci-file.txt'
ls -la /tmp/runglass-receipt
cargo run -- github comment --receipt /tmp/runglass-receipt/receipt.json --repo owner/repo --pr 1 --dry-run
```
