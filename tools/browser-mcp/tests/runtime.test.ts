import { createServer, type Server } from 'node:http';
import { AddressInfo } from 'node:net';
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
 * Starts the local HTTP server used by browser tests.
 * @returns Promise that resolves once the server listens.
 */
function startHttpServer(): Promise<void> {
  return new Promise((resolve) => {
    server = createServer(async (req, res) => {
      if (req.url === '/api/login' && req.method === 'POST') {
        const chunks: Buffer[] = [];
        for await (const chunk of req) {
          chunks.push(Buffer.from(chunk));
        }
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8')) as { email: string };
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ ok: true, email: body.email }));
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
});
