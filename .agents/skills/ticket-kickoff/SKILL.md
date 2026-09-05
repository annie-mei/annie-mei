---
name: ticket-kickoff
description: "Builds a safe execution contract for Linear-backed work across the Annie Mei bot, auth, and docs repositories. Use before inspecting or implementing an ANNIE ticket."
license: MIT
compatibility: Requires access to the Linear issue and relevant Git repositories; Amp thread and GitHub access improve overlap checks.
metadata:
  audience: maintainers
  workflow: linear
---

# Ticket kickoff

Turn a Linear issue into a compact execution contract before changing repository state.

## Inputs

Collect:

- the Linear issue identifier or URL;
- the repository or repositories in scope (`annie-mei`, `auth`, or `docs`);
- the user's requested outcome;
- every authorization explicitly granted in the current request.

If there is no existing issue, do not create one. Ask whether the user wants to provide an issue or explicitly authorize ticket creation.

## 1. Inspect the ticket and repositories

Inspection is read-only. Before inspecting sensitive data or private systems beyond the task's clear scope, ask for permission.

1. Retrieve the Linear issue, including its description, acceptance criteria, status, parent, relations, labels, project, and suggested Git branch name.
2. Read the applicable `AGENTS.md` files and repository guidance for every repository in scope.
3. Inspect each repository's current branch, worktree state, remotes, and relevant manifests or CI configuration. Preserve unrelated worktree changes.
4. Confirm the requested base ref from the user or repository guidance. Do not fetch, switch, create, reset, or update a branch unless that action is authorized.
5. Record any mismatch among the issue, user request, repository guidance, and observed code. The newest explicit user direction wins unless it would violate a safety constraint.

## 2. Check overlap and recent work

Use focused, read-only searches rather than assuming the ticket is isolated:

1. Search Linear for the parent issue, siblings, related or blocking issues, and recently updated or completed issues with the same subsystem or outcome.
2. Search recent Amp threads for the issue identifier, likely files, and the same behavior when thread search is available.
3. Inspect relevant local and remote branches, commits, and pull requests only when the required read access is available. Fetch only when authorized.
4. Compare actual scope and touched files. Similar titles alone do not establish overlap.

Classify each result as:

- **conflict** — concurrent work is likely to touch the same ownership area or files;
- **dependency** — this ticket relies on that work or should reuse its result;
- **related** — context is useful but implementation can proceed independently;
- **none found** — searches found no material overlap;
- **unknown** — a source could not be checked. State which source and why.

Do not claim there is no overlap when a required source was inaccessible.

## 3. Resolve scope and ambiguity

Derive acceptance criteria from the issue and the user's request. Rewrite them as observable outcomes without inventing requirements.

Identify ambiguity that could change any of these:

- user-visible behavior or product policy;
- repository ownership or cross-repository contracts;
- data model, API, migration, compatibility, or rollout scope;
- files owned by concurrent tickets or work;
- semantic version level or which repository owns the version change;
- destructive, shared, or remote actions.

Stop and ask one focused clarification when a product or scope choice would materially change the implementation. Present the viable choices and their consequences. Do not hide the choice inside an implementation assumption.

Proceed with a stated assumption only when it is low-risk, reversible, and does not alter product behavior or ticket scope.

## 4. Establish the execution contract

Before editing, report this compact contract:

```text
Ticket: <ID — title and status>
Suggested branch: <Linear branch name>
Repositories: <in-scope repositories>
Goal: <one outcome>
Acceptance criteria:
- <observable result>
Non-goals:
- <explicitly excluded adjacent work>
Overlap: <classification, issue/thread/PR references, and consequence>
Ambiguities and assumptions: <resolved, clarification needed, or none>
Expected files: <specific paths or ownership areas; mark uncertain paths>
Validation: <focused checks plus repository-required checks>
Version ownership: <repository and file, bump level, no bump, or clarification needed>
Authorization:
- inspect: <authorized/not authorized and source>
- fetch: <authorized/not authorized, authorization source, remote, and ref scope>
- edit: <authorized/not authorized and source>
- branch: <authorized/not authorized and source>
- commit: <authorized/not authorized and source>
- push: <authorized/not authorized and source>
- PR create/update: <authorized/not authorized and source>
- merge: <authorized/not authorized and source>
Stop conditions: <decisions or state changes that require returning to the user>
```

Keep expected files narrow. Name files that are likely to change and explain uncertainty instead of broadening scope preemptively.

Derive validation from the affected behavior and each repository's current guidance or CI. Prefer focused checks first, then required broader checks. Do not assume a canonical validation script exists.

Determine version ownership independently for each repository:

- `annie-mei` owns the bot version in its `Cargo.toml`;
- `auth` owns the auth service version in its `Cargo.toml`;
- `docs` does not own either application version unless its current guidance or manifest establishes a docs-specific version.

Apply the repository's current versioning policy. A cross-repository ticket does not imply that every repository gets a version change.

## 5. Enforce action authorization

Treat each action as a separate capability. Authorization for a later action never follows from an earlier one.

| Action | Required authorization |
| --- | --- |
| Inspect | The request clearly asks for investigation or implementation; ask before sensitive or out-of-scope inspection. |
| Fetch | Explicit approval naming the repository or remote and ref scope to fetch. |
| Edit files | The request clearly asks to implement or modify the scoped repositories. |
| Create or switch branch | Explicit branch instruction, or explicit approval after presenting the suggested branch. |
| Commit | Explicit request or approval to commit locally. |
| Push | Explicit request or approval naming the remote destination. |
| Create or update PR | Explicit request or approval for that PR action. |
| Merge | Explicit request or approval to merge after current readiness is known. |

Ticket creation is a separate action and always requires explicit approval. Never infer authorization for a branch, commit, push, PR, merge, ticket mutation, release, deployment, or destructive Git operation from a request to inspect or edit.

Authorization applies only to the action and scope stated. Later actions do not authorize fetching: permission to create a branch, commit, push, open a PR, or merge does not authorize a fetch. Likewise, permission to push does not authorize opening a PR, and permission to open a PR does not authorize merging it. If authorization is absent, complete all authorized work, report the exact next action, and stop.

## 6. Execute without drifting

After the contract is accepted or contains no blocking ambiguity:

1. Perform only authorized actions.
2. Re-check overlap if new evidence changes expected files or repository ownership.
3. Stop before touching an excluded ownership area or making an unapproved product choice.
4. Validate against the contract and report deviations, unavailable checks, and remaining unauthorized actions.

Do not create tickets, branches, commits, or remote changes merely because this skill was invoked.
