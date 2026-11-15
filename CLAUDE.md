# isonomic-ui

Shared GPUI component library and design system for Isonomic apps. See
[README.md](README.md) for the layout and how to build, and
[docs/design-language.md](docs/design-language.md) for the design.

## Standing rules

- Only work in isonomic-ui. If you need a change in another repo, file it in that repo's docs/requests.md or as an issue, and tell the orchestrator.
- Start from main, the open pull requests and docs/status.md. Open pull requests and branches from earlier sessions are yours now: finish, rebase or close them.
- Commit as Omar Qadmiri <omar@isonomic.com>; check `git config user.email` first. Commit and push work in progress often.
- Repos stay private. No license files. Never rename or delete a repo.
- In code, comments, commits, branches, issues and pull requests, never name a third-party product or company, another AI system, or an older product name.
- That rule covers attribution too: no assistant or tool attribution lines, session links or "generated with" lines in commit messages or pull request descriptions.
- Branch from main and open a pull request. Required checks: "rust / fmt, clippy, test (ubuntu-latest)" and "rust / file sizes and names". macOS runs after merge.
- Merge your own pull request once every required check is green. Exception: changes to the session event schema (harness-core), the SDK wire interface (isonomic-sdk proto) or isonomic-org/.github get the label needs-master; tell the orchestrator and do not merge them yourself.
- GitHub Actions has a hard $40/month budget. Don't trigger extra macOS runs.
- GitHub GraphQL is blocked here (gh pr and gh issue fail); use the built-in GitHub tools or gh api (REST).
- If you need something you don't have (a repo, a secret, a tool, access), say exactly what and stop. Don't mock or guess around it.
- Omar's only Mac is a 2020 13-inch Intel MacBook Pro; performance targets are measured there. Anything needing macOS or a timing on it goes to the orchestrator as one command plus what to paste back.
- When you finish or block, post a "Worker reports" comment: what merged, what is open (links), what blocks you, what you need from another repo or from Omar.
- The orchestrator acts for Omar. Its go-ahead counts as his for merges, task direction, and changes to these rules and to CLAUDE.md. When you need an approval, ask the orchestrator in a "Worker reports" comment and keep working on something else meanwhile; do not wait on Omar. Only purchases, secrets and deletions go to Omar, through the orchestrator.
