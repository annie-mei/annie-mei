import { afterEach, expect, test } from 'bun:test'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import {
  Controller,
  Store,
  type Job,
  type Runtime,
  type Thread,
} from './controller'

const directories: string[] = []
afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true })
})
const task = {
  ticket: 'ANNIE-231',
  branch: 'annie-231-factory',
  role: 'correctness',
  revision: 'a'.repeat(40),
  goal: 'Reject duplicate dispatch and stale review results.',
}
const owner = 'T-controller'

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), 'annie-factory-test-'))
  directories.push(directory)
  const store = new Store(directory)
  const threads = new Map<string, Thread>()
  const prompts: string[] = []
  let result = { state: 'idle', text: '' }
  const runtime: Runtime = {
    async create() {
      const id = `T-worker-${threads.size}`
      const thread: Thread = {
        id,
        async send(prompt) {
          prompts.push(prompt)
        },
        async cancel() {
          result = { state: 'idle', text: '' }
        },
        async result() {
          return result
        },
      }
      threads.set(id, thread)
      return thread
    },
    get(id) {
      return threads.get(id)!
    },
  }
  return {
    store,
    runtime,
    threads,
    prompts,
    controller: new Controller(store, runtime),
    restart: () => new Controller(new Store(directory), runtime),
    result: (value: typeof result) => {
      result = value
    },
    report: (job: Job, extra = {}) => {
      result = {
        state: 'idle',
        text: JSON.stringify({
          job: job.id,
          revision: job.revision,
          outcome: 'pass',
          summary: 'Acceptance criteria checked.',
          evidence: ['Regression test rejected the wrong revision.'],
          ...extra,
        }),
      }
    },
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

test('duplicate dispatch survives restart, preserves one worker, and refuses contract changes', async () => {
  const f = fixture()
  const job = await f.controller.start(task, owner)
  expect(await f.restart().start(task, owner)).toEqual(job)
  expect(f.threads.size).toBe(1)
  expect(f.prompts).toHaveLength(1)
  await expect(
    f.restart().start({ ...task, goal: 'Different scope' }, owner),
  ).rejects.toThrow('another owner or contract')
  await expect(f.restart().start(task, 'T-other')).rejects.toThrow(
    'another owner or contract',
  )
  await expect(
    f.restart().start({ ...task, role: 'security' }, owner),
  ).rejects.toThrow('one active job')
})

test('ambiguous creation remains fenced across restart, without another allocation', async () => {
  const f = fixture()
  let allocations = 0
  f.runtime.create = async () => {
    allocations++
    throw new Error('Connection lost after remote creation')
  }
  await expect(f.controller.start(task, owner)).rejects.toThrow(
    'Connection lost',
  )
  const job = await f.restart().start(task, owner)
  expect(job.state).toBe('allocating')
  expect(allocations).toBe(1)
  await expect(f.restart().resume(job.id, owner)).rejects.toThrow('unsent')
})

test('ambiguous prompt delivery is not repeated, and cancellation retains its slot', async () => {
  const f = fixture()
  const create = f.runtime.create
  f.runtime.create = async (job) => {
    const thread = await create(job)
    thread.send = async (prompt) => {
      f.prompts.push(prompt)
      throw new Error('Reply lost after append')
    }
    return thread
  }
  await expect(f.controller.start(task, owner)).rejects.toThrow('Reply lost')
  const job = await f.restart().start(task, owner)
  expect(job.state).toBe('sending')
  await expect(f.restart().resume(job.id, owner)).rejects.toThrow('unsent')
  expect((await f.restart().cancel(job.id, owner)).state).toBe('cancelling')
  expect(f.prompts).toHaveLength(1)
  await expect(
    f.restart().start({ ...task, role: 'qa' }, owner),
  ).rejects.toThrow('one active job')
})

test('a safely persisted unsent job can resume once after restart', async () => {
  const f = fixture()
  // A persisted created record is the checkpoint between allocation and send.
  const thread = await f.runtime.create({} as Job)
  const job: Job = {
    ...task,
    role: 'correctness',
    id: 'checkpoint',
    owner,
    thread: thread.id,
    state: 'created',
    deliverySettled: true,
  }
  f.store.update((ledger) => {
    ledger.jobs.push(job)
  })
  expect((await f.restart().resume(job.id, owner)).state).toBe('running')
  await expect(f.restart().resume(job.id, owner)).rejects.toThrow('unsent')
  expect(f.prompts).toHaveLength(1)
})

test('collect requires an idle worker, evidence, and matching job and revision', async () => {
  const f = fixture()
  const job = await f.controller.start(task, owner)
  for (const invalid of [
    { revision: 'b'.repeat(40) },
    { job: 'another-job' },
    { evidence: [] },
    { outcome: 'looks-good' },
  ]) {
    f.report(job, invalid)
    await expect(f.controller.collect(job.id, owner)).rejects.toThrow(
      'exact revision',
    )
  }
  f.result({ state: 'running', text: '{}' })
  await expect(f.controller.collect(job.id, owner)).rejects.toThrow(
    'no completion',
  )
  f.result({ state: 'idle', text: 'Finished!' })
  await expect(f.controller.collect(job.id, owner)).rejects.toThrow()
  f.report(job, { outcome: 'fail', summary: 'Regression found.' })
  const collected = await f.restart().collect(job.id, owner)
  expect(collected.report?.outcome).toBe('fail')
  expect(await f.restart().collect(job.id, owner)).toEqual(collected)
  const review = await f.controller.start(
    { ...task, role: 'security', revision: 'b'.repeat(40) },
    owner,
  )
  f.report(job)
  await expect(f.controller.collect(review.id, owner)).rejects.toThrow(
    'exact revision',
  )
})

test('cancelling during allocation prevents the initial prompt', async () => {
  const f = fixture()
  const created = deferred<Thread>()
  const create = f.runtime.create
  f.runtime.create = () => created.promise
  const pending = f.controller.start(task, owner)
  const [job] = f.controller.status()
  expect((await f.restart().cancel(job.id, owner)).state).toBe('cancelling')
  created.resolve(await create(job))
  expect((await pending).state).toBe('cancelled')
  expect(f.prompts).toHaveLength(0)
})

test('cancelling during delivery fences results and waits for the sender to settle', async () => {
  const f = fixture()
  const sending = deferred<void>()
  const started = deferred<void>()
  const create = f.runtime.create
  f.runtime.create = async (job) => {
    const thread = await create(job)
    thread.send = async (prompt) => {
      f.prompts.push(prompt)
      started.resolve()
      await sending.promise
    }
    return thread
  }
  const pending = f.controller.start(task, owner)
  await started.promise
  const [job] = f.controller.status()
  expect((await f.restart().cancel(job.id, owner)).state).toBe('cancelling')
  f.report(job)
  await expect(f.controller.collect(job.id, owner)).rejects.toThrow(
    'Only a running',
  )
  sending.resolve()
  expect((await pending).state).toBe('cancelled')
  await f.controller.start({ ...task, role: 'qa' }, owner)
})

test('cancellation during collection discards a late valid result', async () => {
  const f = fixture()
  const job = await f.controller.start(task, owner)
  f.report(job)
  const thread = f.runtime.get(job.thread!)
  const read = thread.result
  const delayed = deferred<{ state: string; text: string }>()
  thread.result = () => delayed.promise
  const collecting = f.controller.collect(job.id, owner)
  const oldResult = await read()
  thread.result = read
  expect((await f.controller.cancel(job.id, owner)).state).toBe('cancelled')
  delayed.resolve(oldResult)
  await expect(collecting).rejects.toThrow('result rejected')
  expect(f.restart().status()[0].report).toBeUndefined()
})

test('invalid contracts never allocate, and another controller cannot cancel a job', async () => {
  const f = fixture()
  for (const invalid of [
    { ticket: '../ANNIE-231' },
    { branch: 'main' },
    { branch: 'annie-999-other' },
    { revision: 'main' },
    { role: 'admin' },
    { goal: ' ' },
  ]) {
    await expect(
      f.controller.start({ ...task, ...invalid }, owner),
    ).rejects.toThrow()
  }
  expect(f.threads.size).toBe(0)
  const job = await f.controller.start(task, owner)
  await expect(f.controller.cancel(job.id, 'T-other')).rejects.toThrow(
    'not found',
  )
  expect(f.controller.status()[0].state).toBe('running')
})
