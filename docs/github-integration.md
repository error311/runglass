# GitHub Integration

RunGlass can attach a compact command receipt summary to a pull request.

## Commands

Detect local or CI context:

```bash
runglass github detect
```

Preview the exact Markdown comment without calling the GitHub API:

```bash
runglass github comment --receipt latest --repo error311/runglass --pr 123 --dry-run
```

Post or update the RunGlass PR comment:

```bash
runglass github comment --receipt latest --repo error311/runglass --pr 123
```

In GitHub Actions pull-request workflows:

```bash
runglass github comment --receipt runglass-receipt/receipt.json --auto
```

RunGlass uses a hidden marker to update an existing RunGlass comment instead of adding a duplicate:

```md
<!-- runglass-receipt-comment:v1 -->
```

## Authentication

RunGlass looks for a token in this order:

1. `GITHUB_TOKEN`
2. `GH_TOKEN`
3. `gh auth token`

RunGlass does not accept GitHub tokens as CLI arguments. This is intentional: command-line arguments can appear in shell history, CI logs, and process listings.

Tokens are used only for the API request. RunGlass does not write tokens into receipt JSON, Markdown, HTML, bundles, or summary artifacts.

## Permissions

The GitHub REST API creates PR comments through issue comment endpoints because every pull request is also an issue. The token needs permission to write issue or pull request comments.

For GitHub Actions, set permissions explicitly:

```yaml
permissions:
  contents: read
  issues: write
  pull-requests: write
```

## GitHub Actions Example

```yaml
name: RunGlass Receipt

on:
  pull_request:

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

      - name: Run command with a receipt
        run: runglass ci --provider github --output runglass-receipt -- npm test

      - name: Upload RunGlass receipt
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: runglass-receipt
          path: runglass-receipt/

      - name: Comment RunGlass receipt on PR
        if: always()
        run: runglass github comment --receipt runglass-receipt/receipt.json --auto
        env:
          GITHUB_TOKEN: ${{ github.token }}
```

The comment command posts Markdown only. Upload `runglass-receipt/` with `actions/upload-artifact` so reviewers can open the full receipt from the CI run.

## API Behavior

RunGlass calls the GitHub REST API with:

- `Accept: application/vnd.github+json`
- `X-GitHub-Api-Version: 2026-03-10`
- `User-Agent: runglass`

It lists issue comments for the pull request, searches for the RunGlass marker, updates that comment when found, and creates a new comment otherwise.
