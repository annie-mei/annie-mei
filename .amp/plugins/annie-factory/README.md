# Annie factory pilot

A manual, single-controller rehearsal for ANNIE-231. The `annie_factory` tool
launches one human-approved Orb job at a time and keeps a local execution ledger.
Loading the plugin does not launch agents, register webhooks, or change Linear.
There is no automatic planning-to-implementation pipeline or shipping capability.

## Preview task-aware routing

`annie_factory` with `action: "route"` returns an advisory model/effort/provider
choice without accessing threads, prompting for approval, or writing the ledger:

```json
{
  "action": "route",
  "taskClass": "mechanical",
  "paths": ["README.md"],
  "risk": "low",
  "ambiguity": false,
  "escalation": "none"
}
```

The chief supplies the task class and complete expected bot-repository paths, not
a model name. Low-risk mechanical work selects Luna; bounded implementation
selects GLM-5.3 Flash; correctness review selects GLM-5.3 at max effort. These
prefer OpenCode Go. Open-ended work, complexity review, and exploratory QA select
Sol 6.1; focused security review selects Sol 6. Existing deterministic checks
require no model. These are pilot hypotheses, not Annie-specific quality results.

Known sensitive paths (OAuth, database models, privacy/TLS, credentials,
dependencies, migrations, automation and release scripts) impose a Sol floor
even when labelled low-risk. Unknown/empty scope, elevated risk, ambiguity, or
`stalled`/`repeated-failure` escalation also impose that floor. A single understood
compiler/test failure is not itself escalation. No route selects Opus, Astra,
Fable, or Wonder. Path rules are conservative, bot-specific and not exhaustive;
they cannot detect sensitive behavior hidden in omitted or otherwise ordinary files.

**Every result is `launchable: false`.** This preview does not inspect Git, pin a
billing connection, read Codex quota, authorize credit fallback, or feed `start`.
The existing manual launcher below still uses default high mode. Before connecting
the two, verify complete scope against the checkout, enforce the selected provider
and model (including auxiliary agents), and implement fresh quota checks with
headroom protecting the owner's 35% weekly reserve. Missing quota must not spend
the subscription; use an explicitly enabled credit route or pause. Do not silently
downgrade high-risk work when a provider is unavailable. No such enforcement or
automatic escalation is installed by this preview.

## Jev classification prompts

`classification.ts` holds the directly tested `classificationQuestions` for
`amp.ai.decide({ model: 'typesafe/jev', state, questions: classificationQuestions })`.
Importing it performs no inference. It is not connected to `route` or `start`.

The state contains `task`, canonical `paths`, `scopeComplete`, and
`approvedDisposableCheck`. The caller must establish the last two independently;
ticket text cannot claim complete scope or authorize a check. Expected labels,
provider preferences, and budgets are not classifier inputs. Recorded escalation
stays with the deterministic controller, not Jev.

The three independent choices are task class, risk, and owner clarification:
`needed`, `not-needed`, or `unknown`. Mechanical work preserves behavior; a
one-line validation removal does not. Root-cause diagnosis is not a read-only
correctness review. Missing implementation details do not require an owner
decision, while missing intended behavior does. Missing referenced requirements
are `unknown`, distinct from an owner needing to choose a new policy.

Direct probes showed improved distinctions but retained conservative risk and
clarification mismatches, including an unnecessary clarification on an injected
OAuth request. This is a hand-tuned smoke test, not a calibrated accuracy estimate.
Keep full probabilities and model version with evidence; do not interpret model
confidence as an error rate or invent a cutoff from these cases.

Before connecting these prompts to routing, enforce incomplete scope, sensitive
paths, and recorded escalation in code. Resolve top-probability ties conservatively;
an unresolved clarification must not be treated as permission to invent policy.
Check-only classification never authorizes executing a command. Provider and quota
enforcement remain prerequisites to automatic dispatch.

## Run a job

Load `.amp/plugins/annie-factory/index.ts` into an Amp thread in this repository.
Ask the agent to call `annie_factory` with `action: "start"` and:

| Field | Value |
| --- | --- |
| `ticket` | An existing `ANNIE-…` ticket |
| `branch` | That ticket's exact Linear-suggested branch name |
| `goal` | Bounded task, non-goals, and observable acceptance criteria |
| `revision` | Full Git commit SHA already available in the child checkout |
| `role` | `chief`, `worker`, `security`, `complexity`, `correctness`, or `qa` |

The human confirmation names the goal, branch, revision, cost exposure and
permissions. Declining it creates no job. The project is fixed to
`annie-mei/annie-mei`; inputs cannot select another project's secrets. Agents use
the built-in high mode's default model without premium features, and the project's
default Orb size. There is no hard spending cap or automatic deadline in this pilot.

Before approving, check that the branch matches Linear and that project/personal
credentials and setup are appropriate for the task. Branch syntax is checked;
Linear metadata is not fetched by the controller. Children can use the read-only
Linear get/list issue tools for ticket kickoff when that connection is available;
missing required context must be reported as blocked. A child may create the approved
local branch and run checks; only a worker may edit source. Commits, fetches,
pushes, PR actions, deployments, production access and delegation are not granted.

Use `action: "status"` to inspect jobs, then `action: "collect", job: "…"` when a
child finishes. Collection accepts only an idle child's JSON report matching the
job and revision, with a pass/fail/blocked outcome and nonempty evidence. Idle by
itself is not completion. A report is an agent's assertion, not independent proof
that commands ran or that source was unchanged; inspect its evidence and run CI.

## Handoffs are explicit

The chief returns a plan; the operator selects the bounded worker task. A worker
leaves an **uncommitted** patch in its own Orb. Retrieve that work through Amp's
file-transfer/Changes tools and validate it before integration. Messages and a
baseline SHA do not transfer a patch.

For specialist jobs, `revision` must identify the actual candidate commit, not the
worker's baseline. Preparing that commit and making it available in another Orb
requires separate authorization and explicit Git/file transfer. This plugin does
not perform that step. If the exact commit is unavailable, the child must report
blocked rather than reviewing main. No end-to-end uncommitted-patch handoff is
automated yet. Local reviews in the implementation thread remain useful meanwhile.

Each role reports independently; no majority vote, all-clear aggregation, merge
decision or Linear completion is inferred. Failed/blocked reports remain failed/
blocked. A new candidate revision creates distinct review jobs and cannot consume
old reports. Only one job for each ticket/role/revision is allowed; changing its
goal or owner is rejected. Retrying a finished job is intentionally not supported.

## Restart and cancellation

State lives in `$XDG_STATE_HOME/amp/annie-factory/ledger.json`, falling back to
`~/.local/state/amp/annie-factory/ledger.json`, outside the Git checkout. Writes use
an exclusive directory lock, a synced temporary file, atomic rename and directory
sync. This coordinates processes sharing that filesystem, **not separate Orbs**.
Use exactly one controller Orb. Deleting it loses this ledger; copy/backup and
external durable state are future work.

- `allocating`: thread creation may have happened. Do not allocate again.
- `created`: thread ID persisted and no send attempted. `resume` can send the
  recorded contract after human confirmation, including after process restart.
- `sending`: prompt may have arrived. Do not resend, even after an API error.
- `running`: inspect/collect the existing child, never replace it automatically.
- `reported`: a validated report exists, including fail/blocked outcomes.
- `cancelling`: results are fenced out; stop requested, possibly still uncertain.
- `cancelled`: send settled and the child was observed idle/error after cancellation.

`cancel` fences results **before** calling Amp. If a send is in flight, the sender
requests cancellation again after it settles. If a remote call or process dies at
an uncertain boundary, the job remains fenced and occupies the active slot. A stop
request is best effort: it cannot undo edits or guarantee termination of shell
processes/services already started by the child. Inspect and stop those separately.

For a stuck `allocating`, `sending`, or `cancelling` record, inspect the owning
thread and its children before any repair; do not delete the record and retry.
There is deliberately no automatic stale-record eviction or takeover command.
Likewise, remove a crash-held `lock/` directory only after verifying no writer is
alive. Never reset a corrupt or unfamiliar ledger to an empty queue.

## Safety boundary and remaining work

The configured tool list permits only two read-only Linear MCP tools, no plugin
or delegation tools; reviewers lack dedicated edit tools. **Shell access still permits writes and
network access. This is not a security sandbox or an enforceable no-shipping
boundary.** Only run owner-supervised tasks with appropriate credentials. Existing
repository instructions and platform approvals still apply.

Before unattended use: add credential isolation/enforceable capabilities, a
recoverable external ledger, verified source transfer, bounded retries/budgets,
Linear OAuth/intake, independent QA gates, and an authorized integration owner.
Do not run this as a production factory or install a live webhook yet.

## Validation

```sh
bun test ./.amp/plugins/annie-factory/
scripts/verify.sh
```

The plugin tests use a real temporary filesystem and simulated Amp thread APIs.
They cover approval denial, duplicate dispatch/restart, ambiguous remote outcomes,
safe resume, revision-bound reports and cancellation races. They do not spend
model credits or prove real Orb provisioning/model behavior. Plugin discovery and
the read-only status tool can be checked live without creating a child.
