import { createServer, type Server } from 'node:http';
import { AddressInfo } from 'node:net';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { BrowserRuntime } from '../src/runtime.js';

let server: Server;
let baseUrl: string;
let runtime: BrowserRuntime;

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
    const tempRoot = await mkdtemp(path.join(tmpdir(), 'browser-mcp-attach-'));
    const traceStreamPath = path.join(tempRoot, 'TRACE_EVENTS.jsonl');
    const descriptorPath = path.join(tempRoot, 'runtime-attach-descriptor.json');

    await writeFile(
      traceStreamPath,
      [
        JSON.stringify({
          ts: '2026-04-22T10:00:00.000Z',
          event_id: 'evt-1',
          kind: 'job.started',
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

    const attachedRuntime = new BrowserRuntime({
      headless: true,
      runtimeAttachDescriptorPath: descriptorPath,
      screenshotDir: path.join(tempRoot, 'screenshots'),
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('ready');
      expect(diagnostics.attachedRuntime.descriptorSource).toBe('path');
      expect(diagnostics.attachedRuntime.recommendedEntrypoint).toBe('describe_runtime_event_handoff');
      expect(diagnostics.attachedRuntime.eventCount).toBe(2);
      expect(diagnostics.attachedRuntime.latestEventId).toBe('evt-2');
      expect(diagnostics.attachedRuntime.latestEventKind).toBe('job.completed');
      expect(diagnostics.attachedRuntime.traceStreamPath).toBe(traceStreamPath);
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
        artifact_backend_family: 'sqlite',
        attach_capabilities: {
          artifact_replay: true,
          live_remote_stream: false,
          cleanup_preserves_replay: true,
        },
        recommended_entrypoint: 'describe_runtime_event_handoff',
        resolved_artifacts: {
          trace_stream_path: '/logical/sqlite/TRACE_EVENTS.jsonl',
        },
      },
    });

    try {
      const diagnostics = await attachedRuntime.getDiagnostics();
      expect(diagnostics.attachedRuntime.status).toBe('unsupported_backend');
      expect(diagnostics.attachedRuntime.replaySupported).toBe(true);
      expect(diagnostics.attachedRuntime.warning).toContain('filesystem replay only');
    } finally {
      await attachedRuntime.shutdown();
    }
  });
});
