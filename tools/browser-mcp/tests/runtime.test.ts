import { execFile as execFileCallback } from 'node:child_process';
import { createServer, type Server } from 'node:http';
import { AddressInfo } from 'node:net';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { promisify } from 'node:util';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { BrowserRuntime } from '../src/runtime.js';

let server: Server;
let baseUrl: string;
let runtime: BrowserRuntime;
const execFile = promisify(execFileCallback);

async function createAttachedRuntimeFixture(): Promise<{
  tempRoot: string;
  traceStreamPath: string;
  alternateTraceStreamPath: string;
  descriptorPath: string;
  bindingArtifactPath: string;
  handoffPath: string;
  resumeManifestPath: string;
}> {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'browser-mcp-attach-'));
  const transportDir = path.join(tempRoot, 'data', 'runtime_event_transports');
  const traceStreamPath = path.join(tempRoot, 'TRACE_EVENTS.jsonl');
  const alternateTraceStreamPath = path.join(tempRoot, 'TRACE_EVENTS_ALT.jsonl');
  const descriptorPath = path.join(tempRoot, 'runtime-attach-descriptor.json');
  const bindingArtifactPath = path.join(transportDir, 'session-1__job-1.json');
  const handoffPath = path.join(tempRoot, 'ATTACHED_RUNTIME_EVENT_HANDOFF.json');
  const resumeManifestPath = path.join(tempRoot, 'TRACE_RESUME_MANIFEST.json');

  await mkdir(transportDir, { recursive: true });
  await writeFile(
    traceStreamPath,
    [
      JSON.stringify({
        sink_schema_version: 'runtime-trace-sink-v2',
        event: {
          ts: '2026-04-22T10:00:00.000Z',
          event_id: 'evt-1',
          kind: 'job.started',
        },
      }),
      JSON.stringify({
        ts: '2026-04-22T10:00:01.000Z',
        event_id: 'evt-2',
        kind: 'job.completed',
      }),
    ].join('\n') + '\n',
      'utf8',
    );
  await writeFile(
    alternateTraceStreamPath,
    JSON.stringify({
      ts: '2026-04-22T10:00:02.000Z',
      event_id: 'evt-alt-1',
      kind: 'job.alternate',
    }) + '\n',
    'utf8',
  );
  await writeFile(
    bindingArtifactPath,
    JSON.stringify(
      {
        schema_version: 'runtime-event-transport-v1',
        session_id: 'session-1',
        job_id: 'job-1',
        handoff_method: 'describe_runtime_event_handoff',
        replay_supported: true,
        cleanup_preserves_replay: true,
        binding_backend_family: 'filesystem',
        binding_artifact_path: bindingArtifactPath,
      },
      null,
      2,
    ),
    'utf8',
  );
  await writeFile(
    resumeManifestPath,
    JSON.stringify(
      {
        schema_version: 'runtime-resume-manifest-v1',
        session_id: 'session-1',
        job_id: 'job-1',
        status: 'completed',
        trace_stream_path: traceStreamPath,
        event_transport_path: bindingArtifactPath,
        artifact_paths: [bindingArtifactPath, traceStreamPath],
        updated_at: '2026-04-22T10:00:01.000Z',
      },
      null,
      2,
    ),
    'utf8',
  );
  await writeFile(
    handoffPath,
    JSON.stringify(
      {
        schema_version: 'runtime-event-handoff-v1',
        session_id: 'session-1',
        job_id: 'job-1',
        checkpoint_backend_family: 'filesystem',
        trace_stream_path: traceStreamPath,
        resume_manifest_path: resumeManifestPath,
        cleanup_preserves_replay: true,
        attach_target: {
          handoff_method: 'describe_runtime_event_handoff',
        },
        transport: {
          binding_backend_family: 'filesystem',
          binding_artifact_path: bindingArtifactPath,
        },
      },
      null,
      2,
    ),
    'utf8',
  );
  await writeFile(
    descriptorPath,
    JSON.stringify(
      {
        schema_version: 'runtime-event-attach-descriptor-v1',
        attach_mode: 'process_external_artifact_replay',
        artifact_backend_family: 'filesystem',
        attach_capabilities: {
          artifact_replay: true,
          live_remote_stream: false,
          cleanup_preserves_replay: true,
        },
        recommended_entrypoint: 'describe_runtime_event_handoff',
        resolved_artifacts: {
          trace_stream_path: traceStreamPath,
        },
      },
      null,
      2,
    ),
    'utf8',
  );

  return {
    tempRoot,
    traceStreamPath,
    alternateTraceStreamPath,
    descriptorPath,
    bindingArtifactPath,
    handoffPath,
    resumeManifestPath,
  };
}

async function createAttachedRuntimeSqliteFixture(): Promise<{
  tempRoot: string;
  traceStreamPath: string;
  descriptorPath: string;
  bindingArtifactPath: string;
  handoffPath: string;
  resumeManifestPath: string;
}> {
  const tempRoot = await mkdtemp(path.join(tmpdir(), 'browser-mcp-attach-sqlite-'));
  const runtimeRoot = path.join(tempRoot, 'runtime-data');
  const transportDir = path.join(runtimeRoot, 'runtime_event_transports');
  const traceStreamPath = path.join(runtimeRoot, 'TRACE_EVENTS.jsonl');
  const descriptorPath = path.join(tempRoot, 'runtime-attach-descriptor.json');
  const bindingArtifactPath = path.join(transportDir, 'session-sqlite__job-sqlite.json');
  const handoffPath = path.join(runtimeRoot, 'ATTACHED_RUNTIME_EVENT_HANDOFF.json');
  const resumeManifestPath = path.join(runtimeRoot, 'TRACE_RESUME_MANIFEST.json');
  const dbPath = path.join(runtimeRoot, 'runtime_checkpoint_store.sqlite3');

  await mkdir(transportDir, { recursive: true });
  await execFile(
    'python3',
    [
      '-c',
      `
import json, sqlite3, sys
from pathlib import Path

root = Path(sys.argv[1])
db_path = Path(sys.argv[2])
binding_artifact_path = Path(sys.argv[3])
handoff_path = Path(sys.argv[4])
resume_manifest_path = Path(sys.argv[5])
trace_stream_path = Path(sys.argv[6])

conn = sqlite3.connect(db_path)
conn.execute("CREATE TABLE IF NOT EXISTS runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)")

def write_payload(target: Path, payload: str) -> None:
    relative_key = target.relative_to(root).as_posix()
    absolute_key = str(target)
    conn.execute(
        "INSERT OR REPLACE INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        (relative_key, payload),
    )
    conn.execute(
        "INSERT OR REPLACE INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        (absolute_key, payload),
    )

trace_payload = "\\n".join([
    json.dumps({
        "sink_schema_version": "runtime-trace-sink-v2",
        "event": {
            "ts": "2026-04-23T00:00:00.000Z",
            "event_id": "evt-sqlite-1",
            "kind": "job.started",
            "session_id": "session-sqlite",
            "job_id": "job-sqlite",
            "seq": 1,
            "generation": 0,
            "cursor": "g0:s1:evt-sqlite-1",
        },
    }),
    json.dumps({
        "ts": "2026-04-23T00:00:01.000Z",
        "event_id": "evt-sqlite-2",
        "kind": "job.completed",
        "session_id": "session-sqlite",
        "job_id": "job-sqlite",
        "seq": 2,
        "generation": 0,
        "cursor": "g0:s2:evt-sqlite-2",
    }),
]) + "\\n"

binding_payload = json.dumps({
    "schema_version": "runtime-event-transport-v1",
    "session_id": "session-sqlite",
    "job_id": "job-sqlite",
    "handoff_method": "describe_runtime_event_handoff",
    "replay_supported": True,
    "cleanup_preserves_replay": True,
    "binding_backend_family": "sqlite",
    "binding_artifact_path": str(binding_artifact_path),
}, indent=2) + "\\n"

resume_payload = json.dumps({
    "schema_version": "runtime-resume-manifest-v1",
    "session_id": "session-sqlite",
    "job_id": "job-sqlite",
    "status": "completed",
    "trace_stream_path": str(trace_stream_path),
    "event_transport_path": str(binding_artifact_path),
    "artifact_paths": [str(binding_artifact_path), str(trace_stream_path)],
    "updated_at": "2026-04-23T00:00:01.000Z",
}, indent=2) + "\\n"

handoff_payload = json.dumps({
    "schema_version": "runtime-event-handoff-v1",
    "session_id": "session-sqlite",
    "job_id": "job-sqlite",
    "checkpoint_backend_family": "sqlite",
    "trace_stream_path": str(trace_stream_path),
    "resume_manifest_path": str(resume_manifest_path),
    "cleanup_preserves_replay": True,
    "attach_target": {
        "handoff_method": "describe_runtime_event_handoff",
    },
    "transport": {
        "binding_backend_family": "sqlite",
        "binding_artifact_path": str(binding_artifact_path),
    },
}, indent=2) + "\\n"

write_payload(trace_stream_path, trace_payload)
write_payload(binding_artifact_path, binding_payload)
write_payload(resume_manifest_path, resume_payload)
write_payload(handoff_path, handoff_payload)
conn.commit()
conn.close()
      `,
      runtimeRoot,
      dbPath,
      bindingArtifactPath,
      handoffPath,
      resumeManifestPath,
      traceStreamPath,
    ],
  );

  await writeFile(
    descriptorPath,
    JSON.stringify(
      {
        schema_version: 'runtime-event-attach-descriptor-v1',
        attach_mode: 'process_external_artifact_replay',
        artifact_backend_family: 'sqlite',
        attach_capabilities: {
          artifact_replay: true,
          live_remote_stream: false,
          cleanup_preserves_replay: true,
        },
        recommended_entrypoint: 'describe_runtime_event_handoff',
        resolved_artifacts: {
          binding_artifact_path: bindingArtifactPath,
          handoff_path: handoffPath,
          resume_manifest_path: resumeManifestPath,
          trace_stream_path: traceStreamPath,
        },
      },
      null,
      2,
    ),
    'utf8',
  );

  return {
    tempRoot,
    traceStreamPath,
    descriptorPath,
    bindingArtifactPath,
    handoffPath,
    resumeManifestPath,
  };
}

/**
 * Creates the demo HTML page used by tests.
 * @returns HTML payload.
 */
function createLoginPageHtml(): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="UTF-8" />
    <title>Login - Demo</title>
  </head>
  <body>
    <main>
      <h1>Welcome back</h1>
      <form id="login-form">
        <label>Email <input data-testid="email-input" name="email" /></label>
        <label>Password <input data-testid="password-input" name="password" type="password" /></label>
        <button data-testid="sign-in-btn" type="submit">Sign in</button>
      </form>
      <div id="message"></div>
    </main>
    <script>
      const form = document.getElementById('login-form');
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const payload = {
          email: form.elements.email.value,
          password: form.elements.password.value,
        };
        const response = await fetch('/api/login', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const data = await response.json();
        history.pushState({}, '', '/dashboard');
        document.title = 'Dashboard';
        document.querySelector('main').innerHTML = '<h1>Recent activity</h1><p>Verification code sent</p>';
        document.getElementById('message')?.remove();
        document.body.dataset.user = data.email;
      });
    </script>
  </body>
</html>`;
}

/**
 * Creates a simple page whose contents are unique per label.
 * @param label Human-readable revision label.
 * @returns HTML payload.
 */
function createVariantPageHtml(label: string): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="UTF-8" />
    <title>${label}</title>
  </head>
  <body>
    <main>
      <h1>${label}</h1>
      <p>${label} body</p>
    </main>
  </body>
</html>`;
}

/**
 * Creates a page that fires two same-URL fetches with different delays.
 * @returns HTML payload.
 */
function createDuplicateTimingPageHtml(): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="UTF-8" />
    <title>Duplicate timing</title>
  </head>
  <body>
    <main>
      <h1>Duplicate timing</h1>
      <button data-testid="fire-dupe-btn" type="button">Fire duplicate requests</button>
    </main>
    <script>
      const fireDuplicateRequests = () => {
        void (async () => {
          const first = fetch('/api/duplicate-timing', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ slot: 1 }),
          });
          await new Promise((resolve) => setTimeout(resolve, 150));
          const second = fetch('/api/duplicate-timing', {
            method: 'POST',
            headers: { 'content-type': 'application/json' },
            body: JSON.stringify({ slot: 2 }),
          });
          await Promise.all([first, second]);
        })();
      };
      document.querySelector('[data-testid="fire-dupe-btn"]')?.addEventListener('click', fireDuplicateRequests);
      fireDuplicateRequests();
    </script>
  </body>
</html>`;
}

/**
 * Creates a page that continuously mutates its title and visible text.
 * @returns HTML payload.
 */
function createChurnPageHtml(): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="UTF-8" />
    <title>Churn 0</title>
  </head>
  <body>
    <main>
      <h1>Churn 0</h1>
      <p>Step 0</p>
    </main>
    <script>
      let step = 0;
      const render = () => {
        document.title = 'Churn ' + step;
        document.querySelector('main').innerHTML = '<h1>Churn ' + step + '</h1><p>Step ' + step + '</p>';
      };
      render();
      setInterval(() => {
        step += 1;
        render();
      }, 40);
    </script>
  </body>
</html>`;
}

/**
 * Starts the local HTTP server used by browser tests.
 * @returns Promise that resolves once the server listens.
 */
function startHttpServer(): Promise<void> {
  return new Promise((resolve) => {
    server = createServer(async (req, res) => {
      const requestUrl = new URL(req.url ?? '/', 'http://127.0.0.1');

      if (requestUrl.pathname === '/api/login' && req.method === 'POST') {
        const chunks: Buffer[] = [];
        for await (const chunk of req) {
          chunks.push(Buffer.from(chunk));
        }
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8')) as { email: string };
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ ok: true, email: body.email }));
        return;
      }

      if (requestUrl.pathname === '/api/duplicate-timing' && req.method === 'POST') {
        const chunks: Buffer[] = [];
        for await (const chunk of req) {
          chunks.push(Buffer.from(chunk));
        }
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8')) as { slot?: number };
        const delayMs = body.slot === 1 ? 300 : 50;
        await new Promise((resolve) => setTimeout(resolve, delayMs));
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ ok: true, slot: body.slot ?? null }));
        return;
      }

      if (requestUrl.pathname === '/duplicate-fetch') {
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
        res.end(createDuplicateTimingPageHtml());
        return;
      }

      if (requestUrl.pathname === '/churn') {
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
        res.end(createChurnPageHtml());
        return;
      }

      if (requestUrl.pathname.startsWith('/variant/')) {
        const label = requestUrl.pathname.slice('/variant/'.length) || 'variant';
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
        res.end(createVariantPageHtml(label));
        return;
      }

      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      res.end(createLoginPageHtml());
    });

    server.listen(0, '127.0.0.1', () => {
      const address = server.address() as AddressInfo;
      baseUrl = `http://127.0.0.1:${address.port}`;
      resolve();
    });
  });
}

/**
 * Stops the local HTTP server.
 * @returns Promise that resolves once the server closes.
 */
function stopHttpServer(): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}

beforeAll(async () => {
  await startHttpServer();
  runtime = new BrowserRuntime({
    headless: true,
  });
});

afterAll(async () => {
  await runtime.shutdown();
  await stopHttpServer();
});

describe('BrowserRuntime', () => {
  it('opens a page and returns compressed state', async () => {
    await runtime.open({ url: `${baseUrl}/login`, newTab: true });

    const state = await runtime.getState({ include: ['summary', 'interactive_elements', 'diff'] });
    const elements = state.interactiveElements as Array<{ name: string }>;
    const summary = state.summary as { mainGoalArea: string };

    expect(summary.mainGoalArea).toContain('Welcome back');
    expect(elements.some((element) => element.name.includes('Email'))).toBe(true);
    expect(elements.some((element) => element.name.includes('Sign in'))).toBe(true);
  });

  it('fills the form, clicks submit, waits for navigation, and records network activity', async () => {
    await runtime.open({ url: `${baseUrl}/login`, newTab: false });
    const state = await runtime.getState({ include: ['interactive_elements'] });
    const elements = state.interactiveElements as Array<{ ref: string; name: string }>;
    const emailRef = elements.find((element) => element.name.includes('Email'))?.ref;
    const passwordRef = elements.find((element) => element.name.includes('Password'))?.ref;
    const submitRef = elements.find((element) => element.name.includes('Sign in'))?.ref;

    expect(emailRef).toBeTruthy();
    expect(passwordRef).toBeTruthy();
    expect(submitRef).toBeTruthy();

    await runtime.fill({ ref: emailRef!, value: 'agent@example.com' });
    await runtime.fill({ ref: passwordRef!, value: 'secret' });
    await runtime.click({ ref: submitRef! });
    await runtime.waitFor({ condition: { type: 'url_contains', value: '/dashboard' } });

    const nextState = await runtime.getState({ include: ['summary', 'diff'] });
    const network = await runtime.getNetwork({ resourceTypes: ['fetch', 'xhr'], limit: 10, sinceSeconds: 60 });
    const summary = nextState.summary as { mainGoalArea: string };

    expect(summary.mainGoalArea).toContain('Recent activity');
    expect(network.requests.some((request) => request.url.includes('/api/login') && request.status === 200)).toBe(
      true,
    );
  });

  it('returns scoped text and creates a screenshot artifact', async () => {
    await runtime.open({ url: `${baseUrl}/login`, newTab: false });
    const state = await runtime.getState({ include: ['interactive_elements'] });
    const elements = state.interactiveElements as Array<{ ref: string; name: string }>;
    const submitRef = elements.find((element) => element.name.includes('Sign in'))?.ref;

    expect(submitRef).toBeTruthy();

    const text = await runtime.getText({ maxChars: 200 });
    const screenshot = await runtime.screenshot({ scopeRef: submitRef! });

    expect(text.text).toContain('Welcome back');
    expect(screenshot.path.endsWith('.png')).toBe(true);
  });

  it('raises a stale ref error after the page revision changes', async () => {
    await runtime.open({ url: `${baseUrl}/login`, newTab: false });
    const state = await runtime.getState({ include: ['interactive_elements'] });
    const elements = state.interactiveElements as Array<{ ref: string; name: string }>;
    const emailRef = elements.find((element) => element.name.includes('Email'))?.ref;
    const passwordRef = elements.find((element) => element.name.includes('Password'))?.ref;
    const submitRef = elements.find((element) => element.name.includes('Sign in'))?.ref;

    expect(emailRef).toBeTruthy();
    expect(passwordRef).toBeTruthy();
    expect(submitRef).toBeTruthy();

    await runtime.fill({ ref: emailRef!, value: 'agent@example.com' });
    await runtime.fill({ ref: passwordRef!, value: 'secret' });
    await runtime.click({ ref: submitRef! });
    await runtime.waitFor({ condition: { type: 'url_contains', value: '/dashboard' } });

    await expect(runtime.click({ ref: submitRef! })).rejects.toMatchObject({
      code: 'STALE_ELEMENT_REF',
    });
  });

  it('returns a real diff when sinceRevision is provided', async () => {
    await runtime.open({ url: `${baseUrl}/login`, newTab: false });
    const initial = await runtime.getState({ include: ['interactive_elements', 'diff'] });
    const initialRevision = (initial.tab as { pageRevision: number }).pageRevision;
    const elements = initial.interactiveElements as Array<{ ref: string; name: string }>;
    const emailRef = elements.find((element) => element.name.includes('Email'))?.ref;
    const passwordRef = elements.find((element) => element.name.includes('Password'))?.ref;
    const submitRef = elements.find((element) => element.name.includes('Sign in'))?.ref;

    expect(emailRef).toBeTruthy();
    expect(passwordRef).toBeTruthy();
    expect(submitRef).toBeTruthy();

    await runtime.fill({ ref: emailRef!, value: 'agent@example.com' });
    await runtime.fill({ ref: passwordRef!, value: 'secret' });
    await runtime.click({ ref: submitRef! });
    await runtime.waitFor({ condition: { type: 'url_contains', value: '/dashboard' } });

    const finalState = await runtime.getState({ include: ['diff'], sinceRevision: initialRevision });
    const diff = finalState.diff as { fromRevision: number; toRevision: number; newText: string[] };

    expect(diff.fromRevision).toBe(initialRevision);
    expect(diff.toRevision).toBeGreaterThan(initialRevision);
    expect(diff.newText.some((line) => line.includes('Recent activity'))).toBe(true);
  });

  it('records distinct timing for concurrent same-url requests', async () => {
    await runtime.open({ url: `${baseUrl}/duplicate-fetch`, newTab: false });

    const network = await runtime.getNetwork({ limit: 10, sinceSeconds: 60 });
    const duplicateRequests = network.requests.filter((request) =>
      request.url.endsWith('/api/duplicate-timing'),
    );
    const durations = duplicateRequests.map((request) => request.durationMs ?? 0);

    expect(duplicateRequests).toHaveLength(2);
    expect(Math.max(...durations)).toBeGreaterThan(250);
    expect(Math.min(...durations)).toBeLessThan(150);
  });

  it('fails closed when sinceRevision has been evicted from snapshot history', async () => {
    await runtime.open({ url: `${baseUrl}/churn`, newTab: false });
    const initial = await runtime.getState({ include: ['summary'] });
    const initialRevision = (initial.tab as { pageRevision: number }).pageRevision;

    for (let index = 0; index < 11; index += 1) {
      await new Promise((resolve) => setTimeout(resolve, 60));
      await runtime.getState({ include: ['summary'] });
    }

    await expect(runtime.getState({ include: ['diff'], sinceRevision: initialRevision })).rejects.toMatchObject({
      code: 'STALE_STATE_REVISION',
    });
  });

  it('surfaces attached runtime diagnostics from a persisted attach descriptor', async () => {
    const { tempRoot, traceStreamPath, descriptorPath } = await createAttachedRuntimeFixture();

    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptorPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('descriptor_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('attach_descriptor');
      expect(diagnostics.attachedRuntime.recommendedEntrypoint).toBe('describe_runtime_event_handoff');
      expect(diagnostics.attachedRuntime.eventCount).toBe(2);
      expect(diagnostics.attachedRuntime.latestEventId).toBe('evt-2');
      expect(diagnostics.attachedRuntime.latestEventKind).toBe('job.completed');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('accepts the canonical attach artifact path for a persisted descriptor', async () => {
    const { tempRoot, traceStreamPath, descriptorPath } = await createAttachedRuntimeFixture();

    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachArtifactPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('attach_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('attach_descriptor');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('replays attached runtime events through the configured descriptor', async () => {
    const { tempRoot, traceStreamPath, descriptorPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptorPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 1 });
      const firstEvent = replay.events[0]!;
      expect(replay.attachedRuntime.status).toBe('ready');
      expect(replay.replayContext.descriptorSource).toBe('descriptor_path');
      expect(replay.replayContext.inputArtifactKind).toBe('attach_descriptor');
      expect(replay.replayContext.recommendedEntrypoint).toBe('describe_runtime_event_handoff');
      expect(replay.events).toHaveLength(1);
      expect(firstEvent.event_id).toBe('evt-1');
      expect(replay.hasMore).toBe(true);
      expect(replay.nextCursor?.eventId).toBe('evt-1');

      const resumed = await attachedRuntime.getAttachedRuntimeEvents({
        afterEventId: 'evt-1',
        limit: 5,
      });
      const resumedEvent = resumed.events[0]!;
      expect(resumed.events).toHaveLength(1);
      expect(resumedEvent.event_id).toBe('evt-2');
      expect(resumed.hasMore).toBe(false);

      const idle = await attachedRuntime.getAttachedRuntimeEvents({
        afterEventId: 'evt-2',
        heartbeat: true,
      });
      expect(idle.events).toHaveLength(0);
      expect(idle.heartbeat?.status).toBe('idle');
      expect(idle.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('hydrates replay from a transport binding artifact path', async () => {
    const { tempRoot, traceStreamPath, bindingArtifactPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeBindingArtifactPath: bindingArtifactPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('binding_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('binding_artifact');
      expect(diagnostics.attachedRuntime.sourceTransportMethod).toBe('describe_runtime_event_transport');
      expect(diagnostics.attachedRuntime.sourceHandoffMethod).toBe('describe_runtime_event_handoff');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
      expect(diagnostics.attachedRuntime.bindingArtifactSource).toBe('explicit_request');
      expect(diagnostics.attachedRuntime.traceStreamSource).toBe('resume_manifest');

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.replayContext.descriptorSource).toBe('binding_artifact_path');
      expect(replay.replayContext.inputArtifactKind).toBe('binding_artifact');
      expect(replay.replayContext.bindingArtifactSource).toBe('explicit_request');
      expect(replay.replayContext.traceStreamSource).toBe('resume_manifest');
      expect(replay.events).toHaveLength(2);
      expect(replay.events[1]!.event_id).toBe('evt-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('hydrates replay from a handoff artifact path', async () => {
    const { tempRoot, traceStreamPath, handoffPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeHandoffPath: handoffPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('handoff_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('handoff');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
      expect(diagnostics.attachedRuntime.handoffSource).toBe('explicit_request');

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ afterEventId: 'evt-1', limit: 5 });
      expect(replay.replayContext.descriptorSource).toBe('handoff_path');
      expect(replay.replayContext.inputArtifactKind).toBe('handoff');
      expect(replay.replayContext.handoffSource).toBe('explicit_request');
      expect(replay.events).toHaveLength(1);
      expect(replay.events[0]!.event_id).toBe('evt-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('hydrates replay from a resume manifest path', async () => {
    const { tempRoot, traceStreamPath, resumeManifestPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeResumeManifestPath: resumeManifestPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('resume_manifest_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('resume_manifest');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
      expect(diagnostics.attachedRuntime.resumeManifestSource).toBe('explicit_request');

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ afterEventId: 'evt-1', limit: 5 });
      expect(replay.replayContext.descriptorSource).toBe('resume_manifest_path');
      expect(replay.replayContext.inputArtifactKind).toBe('resume_manifest');
      expect(replay.replayContext.resumeManifestSource).toBe('explicit_request');
      expect(replay.events).toHaveLength(1);
      expect(replay.events[0]!.event_id).toBe('evt-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('keeps replay and resume semantics consistent after handoff cleanup falls back to the resume manifest', async () => {
    const { tempRoot, traceStreamPath, handoffPath, resumeManifestPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeHandoffPath: handoffPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const loaded = await (
        attachedRuntime as unknown as {
          loadRuntimeAttachDescriptor(): Promise<{
            descriptor: Record<string, unknown>;
            inputArtifactKind: string | null;
          }>;
        }
      ).loadRuntimeAttachDescriptor();
      expect(loaded.inputArtifactKind).toBe('handoff');
      expect(loaded.descriptor.cleanup_method).toBe('cleanup_attached_runtime_event_transport');
      expect(loaded.descriptor.resume_mode).toBe('after_event_id');
      expect(
        ((loaded.descriptor.attach_capabilities as Record<string, unknown> | undefined)
          ?.cleanup_preserves_replay as boolean | undefined) ?? false,
      ).toBe(true);

      const firstWindow = await attachedRuntime.getAttachedRuntimeEvents({ limit: 1 });
      const afterEventId = firstWindow.events[0]?.event_id;
      expect(firstWindow.events).toHaveLength(1);
      expect(afterEventId).toBe('evt-1');
      expect(firstWindow.nextCursor?.eventId).toBe('evt-1');

      await writeFile(
        handoffPath,
        JSON.stringify(
          {
            schema_version: 'runtime-event-handoff-v1',
            session_id: 'session-1',
            job_id: 'job-1',
            checkpoint_backend_family: 'filesystem',
            trace_stream_path: null,
            resume_manifest_path: resumeManifestPath,
            cleanup_preserves_replay: true,
            attach_target: {
              handoff_method: 'describe_runtime_event_handoff',
            },
            transport: {
              binding_backend_family: 'filesystem',
            },
          },
          null,
          2,
        ),
        'utf8',
      );

      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
      expect(diagnostics.attachedRuntime.resumeManifestSource).toBe('handoff_manifest');
      expect(diagnostics.attachedRuntime.traceStreamSource).toBe('resume_manifest');

      const resumed = await attachedRuntime.getAttachedRuntimeEvents({
        afterEventId: afterEventId as string,
        limit: 5,
      });
      expect(resumed.afterEventId).toBe('evt-1');
      expect(resumed.replayContext.descriptorSource).toBe('handoff_path');
      expect(resumed.replayContext.inputArtifactKind).toBe('handoff');
      expect(resumed.replayContext.resumeManifestSource).toBe('handoff_manifest');
      expect(resumed.replayContext.traceStreamSource).toBe('resume_manifest');
      expect(resumed.events).toHaveLength(1);
      expect(resumed.events[0]!.event_id).toBe('evt-2');
      expect(resumed.hasMore).toBe(false);
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('auto-detects handoff artifacts through the canonical attach artifact path', async () => {
    const { tempRoot, traceStreamPath, handoffPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachArtifactPath: handoffPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('attach_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('handoff');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.events).toHaveLength(2);
      expect(replay.events[1]!.event_id).toBe('evt-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('auto-detects resume manifests through the canonical attach artifact path', async () => {
    const { tempRoot, traceStreamPath, resumeManifestPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachArtifactPath: resumeManifestPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('attach_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('resume_manifest');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.events).toHaveLength(2);
      expect(replay.events[1]!.event_id).toBe('evt-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('fails closed when descriptor and resume manifest disagree on the trace stream path', async () => {
    const {
      tempRoot,
      descriptorPath,
      traceStreamPath,
      alternateTraceStreamPath,
      resumeManifestPath,
    } = await createAttachedRuntimeFixture();
    await writeFile(
      resumeManifestPath,
      JSON.stringify(
        {
          schema_version: 'runtime-resume-manifest-v1',
          session_id: 'session-1',
          job_id: 'job-1',
          status: 'completed',
          trace_stream_path: alternateTraceStreamPath,
          event_transport_path: path.join(tempRoot, 'data', 'runtime_event_transports', 'session-1__job-1.json'),
          artifact_paths: [alternateTraceStreamPath],
          updated_at: '2026-04-22T10:00:03.000Z',
        },
        null,
        2,
      ),
      'utf8',
    );
    await writeFile(
      descriptorPath,
      JSON.stringify(
        {
          schema_version: 'runtime-event-attach-descriptor-v1',
          attach_mode: 'process_external_artifact_replay',
          artifact_backend_family: 'filesystem',
          attach_capabilities: {
            artifact_replay: true,
            live_remote_stream: false,
            cleanup_preserves_replay: true,
          },
          recommended_entrypoint: 'describe_runtime_event_handoff',
          resolved_artifacts: {
            trace_stream_path: traceStreamPath,
            resume_manifest_path: resumeManifestPath,
          },
        },
        null,
        2,
      ),
      'utf8',
    );

    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptorPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('invalid_descriptor');
      expect(diagnostics.attachedRuntime.warning).toContain(
        'mismatched binding/resume trace stream paths',
      );
      await expect(attachedRuntime.getAttachedRuntimeEvents({ limit: 5 })).rejects.toMatchObject({
        code: 'ATTACHED_RUNTIME_INVALID_DESCRIPTOR',
      });
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  });

  it('fails closed in diagnostics when the attach descriptor backend is unsupported', async () => {
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptor: {
        schema_version: 'runtime-event-attach-descriptor-v1',
        attach_mode: 'process_external_artifact_replay',
        artifact_backend_family: 'memory',
        attach_capabilities: {
          artifact_replay: true,
          live_remote_stream: false,
          cleanup_preserves_replay: true,
        },
        recommended_entrypoint: 'describe_runtime_event_handoff',
        resolved_artifacts: {
          trace_stream_path: '/logical/memory/TRACE_EVENTS.jsonl',
        },
      },
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('unsupported_backend');
      expect(diagnostics.attachedRuntime.replaySupported).toBe(true);
      expect(diagnostics.attachedRuntime.warning).toContain('filesystem/sqlite replay only');
    } finally {
      await attachedRuntime.shutdown();
    }
  });

  it('fails closed when a direct attach descriptor drifts on cleanup or resume vocabulary', async () => {
    const { tempRoot, traceStreamPath } = await createAttachedRuntimeFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptor: {
        schema_version: 'runtime-event-attach-descriptor-v1',
        attach_mode: 'process_external_artifact_replay',
        artifact_backend_family: 'filesystem',
        attach_capabilities: {
          artifact_replay: true,
          live_remote_stream: false,
          cleanup_preserves_replay: false,
        },
        cleanup_method: 'cleanup_runtime_events',
        resume_mode: 'cursor_index',
        resolved_artifacts: {
          trace_stream_path: traceStreamPath,
        },
      },
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('invalid_descriptor');
      expect(diagnostics.attachedRuntime.warning).toContain(
        'cleanup_method=cleanup_attached_runtime_event_transport',
      );
      await expect(attachedRuntime.getAttachedRuntimeEvents({ limit: 5 })).rejects.toMatchObject({
        code: 'ATTACHED_RUNTIME_INVALID_DESCRIPTOR',
      });
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  });

  it('supports attached runtime diagnostics and replay from a sqlite attach descriptor', async () => {
    const { tempRoot, descriptorPath, traceStreamPath } = await createAttachedRuntimeSqliteFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptorPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.artifactBackendFamily).toBe('sqlite');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('attach_descriptor');
      expect(diagnostics.attachedRuntime.sourceTransportMethod).toBe('describe_runtime_event_transport');
      expect(diagnostics.attachedRuntime.sourceHandoffMethod).toBe('describe_runtime_event_handoff');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
      expect(diagnostics.attachedRuntime.bindingArtifactSource).toBeNull();
      expect(diagnostics.attachedRuntime.traceStreamSource).toBe('handoff_manifest');
      expect(diagnostics.attachedRuntime.eventCount).toBe(2);
      expect(diagnostics.attachedRuntime.latestEventId).toBe('evt-sqlite-2');

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.replayContext.descriptorSource).toBe('descriptor_path');
      expect(replay.replayContext.inputArtifactKind).toBe('attach_descriptor');
      expect(replay.replayContext.artifactBackendFamily).toBe('sqlite');
      expect(replay.replayContext.sourceTransportMethod).toBe('describe_runtime_event_transport');
      expect(replay.replayContext.traceStreamSource).toBe('handoff_manifest');
      expect(replay.events).toHaveLength(2);
      expect(replay.events[0]!.event_id).toBe('evt-sqlite-1');
      expect(replay.events[1]!.event_id).toBe('evt-sqlite-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('hydrates replay from a sqlite binding artifact path', async () => {
    const { tempRoot, bindingArtifactPath, traceStreamPath } = await createAttachedRuntimeSqliteFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeBindingArtifactPath: bindingArtifactPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('binding_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('binding_artifact');
      expect(diagnostics.attachedRuntime.artifactBackendFamily).toBe('sqlite');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.events).toHaveLength(2);
      expect(replay.events[1]!.event_id).toBe('evt-sqlite-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('hydrates a canonical descriptor from a sqlite binding artifact path', async () => {
    const { tempRoot, bindingArtifactPath, traceStreamPath } = await createAttachedRuntimeSqliteFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeBindingArtifactPath: bindingArtifactPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('binding_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('binding_artifact');
      expect(diagnostics.attachedRuntime.artifactBackendFamily).toBe('sqlite');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);

      const loaded = await (
        attachedRuntime as unknown as {
          loadRuntimeAttachDescriptor(): Promise<{
            descriptor: Record<string, unknown>;
            inputArtifactKind: string | null;
          }>;
        }
      ).loadRuntimeAttachDescriptor();
      expect(loaded.inputArtifactKind).toBe('binding_artifact');
      const descriptor = loaded.descriptor;
      expect(descriptor.schema_version).toBe('runtime-event-attach-descriptor-v1');
      expect(descriptor.source_handoff_method).toBe('describe_runtime_event_handoff');
      expect(descriptor.source_transport_method).toBe('describe_runtime_event_transport');
      expect(descriptor.attach_method).toBe('attach_runtime_event_transport');
      expect(descriptor.subscribe_method).toBe('subscribe_attached_runtime_events');
      expect(descriptor.cleanup_method).toBe('cleanup_attached_runtime_event_transport');
      expect(descriptor.resume_mode).toBe('after_event_id');
      expect((descriptor.resolution as Record<string, unknown>).binding_artifact_path).toBe(
        'explicit_request',
      );
      expect(
        ((descriptor.resolved_artifacts as Record<string, unknown>).binding_artifact_path as string),
      ).toBe(bindingArtifactPath);
      expect(
        ((descriptor.resolved_artifacts as Record<string, unknown>).trace_stream_path as string),
      ).toBe(traceStreamPath);

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.events).toHaveLength(2);
      expect(replay.events[0]!.event_id).toBe('evt-sqlite-1');
      expect(replay.events[1]!.event_id).toBe('evt-sqlite-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);

  it('auto-detects sqlite resume manifests through the canonical attach artifact path', async () => {
    const { tempRoot, resumeManifestPath, traceStreamPath } = await createAttachedRuntimeSqliteFixture();
    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachArtifactPath: resumeManifestPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('attach_artifact_path');
      expect(diagnostics.attachedRuntime.inputArtifactKind).toBe('resume_manifest');
      expect(diagnostics.attachedRuntime.artifactBackendFamily).toBe('sqlite');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);

      const replay = await attachedRuntime.getAttachedRuntimeEvents({ limit: 5 });
      expect(replay.replayContext.descriptorSource).toBe('attach_artifact_path');
      expect(replay.replayContext.inputArtifactKind).toBe('resume_manifest');
      expect(replay.replayContext.artifactBackendFamily).toBe('sqlite');
      expect(replay.events).toHaveLength(2);
      expect(replay.events[0]!.event_id).toBe('evt-sqlite-1');
      expect(replay.events[1]!.event_id).toBe('evt-sqlite-2');
    } finally {
      await attachedRuntime.shutdown();
      await rm(tempRoot, { recursive: true, force: true });
    }
  }, 15000);
});
