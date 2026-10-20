import { expect, test } from 'bun:test'
import type {
  PluginAPI,
  PluginToolDefinition,
  PluginToolContext,
} from '@ampcode/plugin'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import register from './index'

test('plugin requires human approval, launches only in the fixed project, and collects a revision-bound report', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'annie-factory-plugin-test-'))
  const previous = process.env.XDG_STATE_HOME
  process.env.XDG_STATE_HOME = directory
  let tool!: PluginToolDefinition
  let allowed = false
  let allocations = 0
  let result = ''
  const thread = {
    id: 'T-child',
    state: { get: async () => 'idle' },
    appendUserMessage: async () => {},
    messages: async () => [
      { role: 'assistant', content: [{ type: 'text', text: result }] },
    ],
    cancel: async () => {},
  }
  const amp = {
    registerTool: (definition: PluginToolDefinition) => {
      tool = definition
    },
    createAgent: () => ({
      createThread: async (options: Record<string, unknown>) => {
        expect(options.project).toBe('annie-mei/annie-mei')
        expect(options.executor).toBe('orb')
        expect(options.visibility).toBe('private')
        allocations++
        return thread
      },
    }),
    threads: { get: () => thread },
  } as unknown as PluginAPI
  const ctx = {
    thread: { id: 'T-owner' },
    ui: {
      confirm: async (options: { requireHuman?: boolean }) => {
        expect(options.requireHuman).toBe(true)
        return allowed
      },
    },
  } as unknown as PluginToolContext
  const input = {
    action: 'start',
    ticket: 'ANNIE-231',
    branch: 'annie-231-factory',
    goal: 'Inspect the acceptance contract.',
    role: 'chief',
    revision: 'c'.repeat(40),
    project: 'untrusted/other-project',
  }
  try {
    register(amp)
    expect(await tool.execute(input, ctx)).toBe(
      'Not approved; no job dispatched.',
    )
    expect(allocations).toBe(0)
    expect(await tool.execute({ action: 'status' }, ctx)).toBe('[]')
    allowed = true
    const job = JSON.parse((await tool.execute(input, ctx)) as string)
    expect(job.state).toBe('running')
    expect(allocations).toBe(1)
    register(amp)
    await tool.execute(input, ctx)
    expect(allocations).toBe(1)
    result = JSON.stringify({
      job: job.id,
      revision: 'c'.repeat(40),
      outcome: 'blocked',
      summary: 'Need product decision.',
      evidence: ['Goal leaves the response policy unspecified.'],
    })
    const collected = JSON.parse(
      (await tool.execute({ action: 'collect', job: job.id }, ctx)) as string,
    )
    expect(collected.state).toBe('reported')
    expect(collected.report.outcome).toBe('blocked')
  } finally {
    if (previous === undefined) delete process.env.XDG_STATE_HOME
    else process.env.XDG_STATE_HOME = previous
    rmSync(directory, { recursive: true, force: true })
  }
})

test('routing previews select by task and risk without accessing threads, approvals, or the ledger', async () => {
  let tool!: PluginToolDefinition
  register({
    registerTool: (definition: PluginToolDefinition) => { tool = definition },
  } as unknown as PluginAPI)
  const preview = async (overrides: Record<string, unknown>) => JSON.parse(
    await tool.execute({
      action: 'route', taskClass: 'mechanical', paths: ['README.md'],
      risk: 'low', ambiguity: false, escalation: 'none', ...overrides,
    }, {} as PluginToolContext) as string,
  )
  const cases: [Record<string, unknown>, string | null, string][] = [
    [{}, 'openai/gpt-6-luna', 'opencode-go'],
    [{ taskClass: 'bounded-implementation' }, 'zai/glm-5.3-flash', 'opencode-go'],
    [{ taskClass: 'correctness-review' }, 'zai/glm-5.3', 'opencode-go'],
    [{ taskClass: 'security-review' }, 'openai/gpt-6-sol', 'codex-with-reserve-or-explicit-amp-credits'],
    [{ taskClass: 'deterministic-check', risk: 'elevated' }, null, 'none'],
  ]
  for (const [input, model, desiredProvider] of cases) {
    expect(await preview(input)).toMatchObject({ model, desiredProvider, launchable: false })
  }
  for (const input of [
    { paths: [] }, { risk: 'unknown' }, { risk: 'elevated' },
    { ambiguity: true }, { escalation: 'stalled' }, { escalation: 'repeated-failure' },
    { taskClass: 'open-ended' }, { taskClass: 'complexity-review' },
    { taskClass: 'exploratory-qa' },
    ...['src/utils/oauth/mod.rs', 'src/commands/register/command.rs',
      'src/commands/unregister.rs', 'src/models/db/oauth_credential.rs',
      'src/utils/privacy/mod.rs', 'src/utils/tls.rs', '.env.example',
      '.github/workflows/build-release.yml', '.amp/plugins/annie-factory/index.ts',
      'migrations/new.up.sql', 'Cargo.lock', 'scripts/verify.sh',
    ].map((path) => ({ paths: ['README.md', path] })),
  ]) {
    expect(await preview(input)).toMatchObject({
      model: 'openai/gpt-6.1-sol', effort: 'high', launchable: false,
    })
  }
  for (const input of [
    { risk: undefined }, { ambiguity: undefined }, { escalation: undefined },
    { taskClass: 'arbitrary-model' }, { paths: undefined },
    { paths: ['src/../src/utils/oauth/mod.rs'] }, { paths: ['/etc/passwd'] },
    { paths: ['src\\utils\\oauth\\mod.rs'] },
  ]) {
    await expect(preview(input)).rejects.toThrow()
  }
})
