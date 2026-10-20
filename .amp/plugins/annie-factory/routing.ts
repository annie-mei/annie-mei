// Advisory only: this does not select a billing connection or authorize execution.
export const taskClasses = [
  'mechanical',
  'bounded-implementation',
  'open-ended',
  'correctness-review',
  'complexity-review',
  'security-review',
  'exploratory-qa',
  'deterministic-check',
] as const

export function previewRoute(input: Record<string, unknown>) {
  const { taskClass, paths, risk, ambiguity, escalation } = input
  if (!taskClasses.includes(taskClass as (typeof taskClasses)[number]))
    throw new Error('A known taskClass is required.')
  if (!['low', 'elevated', 'unknown'].includes(risk as string))
    throw new Error('Risk must be low, elevated, or unknown.')
  if (typeof ambiguity !== 'boolean')
    throw new Error('Explicit ambiguity assessment is required.')
  if (!['none', 'stalled', 'repeated-failure'].includes(escalation as string))
    throw new Error('Explicit escalation evidence category is required.')
  if (
    !Array.isArray(paths) ||
    paths.some(
      (path) =>
        typeof path !== 'string' ||
        !path ||
        path.startsWith('/') ||
        path.includes('\\') ||
        path.split('/').some((part: string) => !part || part === '.' || part === '..'),
    )
  )
    throw new Error('Paths must be canonical repository-relative paths.')

  // These bot-owned areas impose a floor even if classified as low-risk.
  // The caller must supply complete scope; this preview does not inspect Git.
  const sensitive = paths.some((path: string) =>
    /^(?:\.env(?:\.|$)|(?:\.github|\.amp|\.agents|migrations|scripts)\/|Cargo\.(?:toml|lock)$|src\/utils\/(?:oauth\/|privacy\/|tls\.rs$)|src\/models\/db\/|src\/commands\/(?:register\/|unregister\.rs$|whoami\.rs$))/.test(path),
  )
  let model: string | null = 'openai/gpt-6.1-sol'
  let effort: 'high' | 'max' | null = 'high'
  let desiredProvider = 'codex-with-reserve-or-explicit-amp-credits'
  let reason = 'Open-ended work requires Sol; no automatic flagship escalation.'

  if (taskClass === 'deterministic-check') {
    model = null
    effort = null
    desiredProvider = 'none'
    reason = 'Run an existing approved check in code; this does not approve its result or authorize side effects.'
  } else if (taskClass === 'security-review') {
    model = 'openai/gpt-6-sol'
    reason = 'Focused security review uses Sol; executable security checks remain required.'
  } else if (sensitive || risk !== 'low' || ambiguity || paths.length === 0 || escalation !== 'none') {
    reason = 'Sol floor: sensitive or unknown scope, elevated risk, ambiguity, or documented stalled/repeatedly failing work.'
  } else if (taskClass === 'mechanical') {
    model = 'openai/gpt-6-luna'
    desiredProvider = 'opencode-go'
    reason = 'Low-risk mechanical work with explicit scope; independent verification is still required.'
  } else if (taskClass === 'bounded-implementation') {
    model = 'zai/glm-5.3-flash'
    desiredProvider = 'opencode-go'
    reason = 'Bounded, low-risk implementation with explicit scope and acceptance checks.'
  } else if (taskClass === 'correctness-review') {
    model = 'zai/glm-5.3'
    effort = 'max'
    desiredProvider = 'opencode-go'
    reason = 'Independent correctness review; findings require concrete evidence, not a vote.'
  }

  return {
    policy: 'task-routing-preview-v1',
    taskClass,
    model,
    effort,
    desiredProvider,
    reason,
    launchable: false,
    limitation: 'Advisory only. Scope is caller-supplied. Provider pinning, fresh quota checks protecting the 35% weekly reserve, and explicit credit fallback are not implemented. No job was launched.',
  }
}
