<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install --cask codex</code></p>
<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Team, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Process Mode (fork-specific, experimental)

This fork adds an experimental `process` command group to bootstrap process-native runs:

```shell
codex process run --task "Implement X"
codex process status --run-id <id>
codex process pr-comments --repo owner/repo --pr 123
codex process pr-comments --repo owner/repo --pr 123 --act
codex process pr-comments --repo owner/repo --pr 123 --act --dry-run
codex process pr-comments --repo owner/repo --pr 123 --gh-max-attempts 7 --gh-base-backoff-ms 750
codex process issues watch --repo owner/repo --label process:auto-fix --limit 20
codex process issues watch --repo owner/repo --label process:auto-fix --limit 20 --act
codex process issues watch --repo owner/repo --label process:auto-fix --limit 20 --act --dry-run
codex process issues watch --repo owner/repo --label process:auto-fix --limit 20 --act --max-concurrency 3 --queue-delay-ms 400 --max-act-items 10
```

The command currently scaffolds machine-readable artifacts under `.process/runs/<run-id>/` for contract/red/verify/evidence stages.
The `pr-comments` subcommand performs live GitHub comment ingestion via `gh`, capturing unresolved PR review comments (including `reviewThreadId`) and issue comments into `.process/runs/<run-id>/pr-comments.json`.
Passing `--act` enables triage and follow-up automation: comments are classified (`quick_fix`, `needs_issue`, `question`) and written to `.process/runs/<run-id>/triage.json`.
Passing `--act --dry-run` performs the same ingestion + triage planning flow but skips all external mutations (no Codex subprocesses, no branch/commit/push/PR operations, and no GitHub comment/thread/issue updates).
For `quick_fix` items, Codex runs targeted `exec` subprocesses in isolated git worktrees/branches (`process/quick-fix-pr-<pr>-<comment-id-short>`), creates one commit per successful item, then attempts to push each quick-fix branch and open a follow-up PR (`base` = source PR base branch when detectable, otherwise `main`). For review-comment quick fixes with a successful follow-up PR, it also attempts to resolve the original review thread via `gh api graphql` (`resolveReviewThread`). The triage artifact records per-item execution/commit metadata plus push/PR metadata (`quickFixPushed`, `quickFixRemoteBranch`, `quickFixPrUrl`, `quickFixPrNumber`, `quickFixPushError`, `quickFixPrError`) and thread-resolution metadata when applicable (`quickFixThreadResolved`, `quickFixThreadResolveError`).
When at least one quick fix succeeds, the command posts one concise PR update comment through `gh pr comment` summarizing applied items (including files/verification status, commit links, and created follow-up PR links when available), and stores that comment URL in the triage artifact when it can be detected.
For `needs_issue` items, follow-up issues are opened via `gh issue create` and linked in the artifact.
`triage.json` now includes run-level `dryRun` and per-item `plannedAction`.
The `issues watch` subcommand fetches matching open issues and writes `.process/runs/<run-id>/issues-watch.json` with `fetchedAt`, `repo`, `label`, `openIssues[]`, and `suggestedActions[]`.
With `--act`, `issues watch` triages each issue (`quick_fix` or `needs_manual`) and isolates each quick-fix attempt so one issue failure does not abort the rest. Matching issues are first queued and then started with bounded concurrency (`--max-concurrency`, default `1`) plus inter-start throttling (`--queue-delay-ms`, default `250`) to reduce GitHub/git bursts. Use `--max-act-items` to cap how many queued issues are acted on in a run (remaining issues are recorded as skipped with an explicit reason). Successful quick-fix attempts run targeted `codex exec` in an isolated worktree branch, commit/push changes, open a follow-up PR, and post a status comment back on the issue via `gh issue comment`. Non-successful attempts post a concise manual-follow-up comment with the failure reason. `--act --dry-run` keeps ingestion/triage/queue planning but skips all mutation steps. Action runs write `.process/runs/<run-id>/issues-watch-act.json` with run-level fields `dryRun`, `fetchedAt`, `repo`, `label`, `maxConcurrency`, `queueDelayMs`, `maxActItems`, `actedCount`, `skippedCount`, and per-issue `issueActions[]` entries: `issueNumber`, `issueUrl`, `decision`, `plannedAction`, `skippedReason`, `attempted`, `success`, `branch`, `commitSha`, `commitUrl`, `prUrl`, `prNumber`, `updateCommentUrl`, and `error`.
`gh` process-mode calls now retry transient API failures (including rate limiting/abuse/secondary-limit signals) with exponential backoff + jitter. Tune behavior with `--gh-max-attempts` (default `5`) and `--gh-base-backoff-ms` (default `500`).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
