import { mkdir, readdir, readFile, stat, unlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import {
  chromium,
  firefox,
  webkit,
  type Browser,
  type BrowserType,
  type Locator,
  type Page,
  type Request,
  type Response,
} from 'playwright';
import { BrowserToolError } from './errors.js';
import type {
  ActionResult,
  BrowserRuntimeOptions,
  BrowserSessionView,
  BrowserTabView,
  ClickInput,
  CloseInput,
  DiagnosticsResult,
  ElementDescriptor,
  FillInput,
  GetElementsInput,
  GetNetworkInput,
  GetStateInput,
  GetTextInput,
  InteractiveElement,
  LoadingState,
  NetworkEvent,
  OpenPageInput,
  PageDelta,
  PageSnapshot,
  PageSummary,
  PressInput,
  RestoreSessionInput,
  RestoreSessionResult,
  SaveSessionInput,
  SaveSessionResult,
  ScreenshotInput,
  ScreenshotResult,
  SessionRecord,
  TabRecord,
  TabsInput,
  WaitCondition,
  WaitForInput,
} from './types.js';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_MAX_ELEMENTS = 20;
const DEFAULT_TEXT_BUDGET = 1200;
const DEFAULT_NETWORK_LIMIT = 20;
const DEFAULT_WAIT_MS = 8_000;
const MAX_NETWORK_EVENTS = 200;
const SNAPSHOT_HISTORY_LIMIT = 10;
const BODY_CAPTURE_LIMIT = 4096; // bytes
const RUNTIME_VERSION = '0.2.0';

const INTERACTIVE_SELECTOR = [
  'button',
  'input',
  'select',
  'textarea',
  'a[href]',
  '[role="button"]',
  '[role="link"]',
  '[role="textbox"]',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/** Returns ISO-8601 timestamp. */
function nowIso(): string {
  return new Date().toISOString();
}

/**
 * Builds deduplicated short lines from visible text.
 * Preserves order but removes exact-duplicate lines.
 */
function toTextLines(text: string): string[] {
  const seen = new Set<string>();
  const lines: string[] = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (line.length > 0 && !seen.has(line)) {
      seen.add(line);
      lines.push(line);
      if (lines.length >= 20) break;
    }
  }
  return lines;
}

/**
 * Truncates text to a maximum number of characters.
 */
function truncateText(text: string, maxChars: number): string {
  return text.length <= maxChars ? text : `${text.slice(0, Math.max(0, maxChars - 1))}…`;
}

/**
 * Creates a stable fingerprint for one interactive element.
 *
 * Strategy (most-to-least stable):
 *  1. If testId exists → use it directly (globally unique in well-tested apps).
 *  2. Otherwise → role::name::tag. If that string is seen >1 time in the same
 *     snapshot, a numeric suffix is appended to preserve uniqueness without
 *     depending on DOM ordinal across re-renders.
 *
 * The caller must pass already-accumulated fingerprints (fingerprintCounts) so
 * collision suffixes are computed correctly in one forward pass.
 */
function createFingerprint(
  descriptor: ElementDescriptor,
  fingerprintCounts: Map<string, number>,
): string {
  if (descriptor.locatorHint.testId) {
    return `tid::${descriptor.locatorHint.testId}`;
  }

  const base = `${descriptor.role}::${descriptor.name}::${descriptor.locatorHint.tag}`;
  const count = (fingerprintCounts.get(base) ?? 0) + 1;
  fingerprintCounts.set(base, count);
  return count === 1 ? base : `${base}::${count}`;
}

/**
 * Picks the Playwright browser type for a given engine name.
 */
function getBrowserType(browserEngine: BrowserRuntimeOptions['browserEngine']): BrowserType {
  switch (browserEngine) {
    case 'firefox':
      return firefox;
    case 'webkit':
      return webkit;
    case 'chromium':
    default:
      return chromium;
  }
}

// ---------------------------------------------------------------------------
// DOM evaluation snippet — shared by full-page and scoped collectors
// ---------------------------------------------------------------------------

/**
 * Evaluates interactive-element descriptors inside a Playwright locator.
 * This function runs inside the browser context via evaluateAll.
 */
function domDescriptorMapper(nodes: Element[]): ElementDescriptor[] {
  return nodes.map((node, index) => {
    const element = node as HTMLElement;
    const role =
      element.getAttribute('role') ||
      (element instanceof HTMLAnchorElement
        ? 'link'
        : element instanceof HTMLButtonElement
          ? 'button'
          : element instanceof HTMLInputElement ||
              element instanceof HTMLTextAreaElement ||
              element instanceof HTMLSelectElement
            ? 'textbox'
            : 'generic');
    const id = element.getAttribute('id');
    const wrappingLabel = element.closest('label');
    const explicitLabel = id ? document.querySelector(`label[for="${id}"]`) : null;
    const labelledBy = element.getAttribute('aria-labelledby');
    const labelledByText = labelledBy
      ? labelledBy
          .split(/\s+/)
          .map((token) => document.getElementById(token)?.textContent || '')
          .join(' ')
      : '';
    const rawLabel =
      element.getAttribute('aria-label') ||
      wrappingLabel?.textContent ||
      explicitLabel?.textContent ||
      labelledByText ||
      element.getAttribute('placeholder') ||
      element.getAttribute('name') ||
      element.textContent ||
      '';
    const text = (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
    const rect = element.getBoundingClientRect();
    const visible = rect.width > 0 && rect.height > 0;
    const testId =
      element.getAttribute('data-testid') || element.getAttribute('data-test') || undefined;
    return {
      role,
      name: rawLabel.replace(/\s+/g, ' ').trim(),
      text,
      visible,
      enabled: !(element as HTMLButtonElement).disabled,
      locatorHint: { tag: element.tagName.toLowerCase(), testId },
      ordinal: index,
    } satisfies ElementDescriptor;
  });
}

// ---------------------------------------------------------------------------
// BrowserRuntime
// ---------------------------------------------------------------------------

/**
 * Lean browser runtime for MCP tools.
 * Manages sessions, tabs, snapshots, network events, and storage persistence.
 */
export class BrowserRuntime {
  private readonly options: BrowserRuntimeOptions;
  private readonly sessions = new Map<string, SessionRecord>();
  private readonly browsers = new Set<Browser>();
  private sessionCounter = 0;
  private tabCounter = 0;
  private refCounter = 0;
  private requestCounter = 0;
  private screenshotCounter = 0;

  /** Optional MCP server reference for emitting log notifications. */
  private logEmitter: ((level: string, msg: string) => void) | null = null;

  public constructor(options?: Partial<BrowserRuntimeOptions>) {
    this.options = {
      browserEngine: options?.browserEngine ?? 'chromium',
      headless: options?.headless ?? true,
      viewport: options?.viewport ?? { width: 1440, height: 900 },
      screenshotDir:
        options?.screenshotDir ?? path.resolve(process.cwd(), 'output', 'browser-mcp-screenshots'),
      captureBody: options?.captureBody ?? false,
      maxScreenshots: options?.maxScreenshots ?? 100,
    };
  }

  /**
   * Attaches a log emitter so the runtime can relay log events to the MCP client.
   * @param emitter Callback accepting (level, message).
   */
  public setLogEmitter(emitter: (level: string, msg: string) => void): void {
    this.logEmitter = emitter;
  }

  // -------------------------------------------------------------------------
  // Public tool surface
  // -------------------------------------------------------------------------

  /**
   * Opens a page in the current session or creates a new session.
   */
  public async open(input: OpenPageInput): Promise<{ session: BrowserSessionView; tab: BrowserTabView }> {
    this.log('info', `open: ${input.url} newTab=${input.newTab ?? false}`);
    const session = await this.getOrCreateSession();
    const tab = input.newTab || session.currentTabId === null
      ? await this.createTab(session)
      : this.getRequiredTab(session, session.currentTabId);

    await tab.page.goto(input.url, { waitUntil: 'domcontentloaded' });
    await this.settle(tab.page);
    await this.refreshSnapshot(tab);
    session.currentTabId = tab.id;

    return {
      session: this.toSessionView(session),
      tab: this.toTabView(tab),
    };
  }

  /**
   * Lists tabs or switches the active tab.
   */
  public async tabs(input: TabsInput): Promise<{ currentTabId: string | null; tabs: BrowserTabView[] }> {
    const session = this.getRequiredSession();

    if (input.action === 'select') {
      if (!input.tabId) {
        throw new BrowserToolError('INVALID_INPUT', 'tabId is required for select.', true, [
          'provide tabId',
          'call browser_tabs with action=list',
        ]);
      }
      const tab = this.getRequiredTab(session, input.tabId);
      session.currentTabId = tab.id;
    }

    return {
      currentTabId: session.currentTabId,
      tabs: Array.from(session.tabs.values()).map((tab) => this.toTabView(tab)),
    };
  }

  /**
   * Closes a tab or the entire session.
   */
  public async close(input: CloseInput): Promise<{ ok: true; closed: string; remainingTabs: number }> {
    const session = this.getRequiredSession();

    if (input.target === 'session') {
      const remainingTabs = session.tabs.size;
      await this.disposeSession(session);
      return { ok: true, closed: 'session', remainingTabs };
    }

    const tabId = input.tabId ?? session.currentTabId;
    if (!tabId) {
      throw new BrowserToolError('TAB_NOT_FOUND', 'No active tab is available.', true, [
        'call browser_open',
      ]);
    }

    const tab = this.getRequiredTab(session, tabId);
    await tab.page.close();
    tab.disposeNetworkObserver?.();
    session.tabs.delete(tab.id);
    session.currentTabId =
      session.tabs.size > 0 ? (Array.from(session.tabs.keys())[0] ?? null) : null;

    if (session.tabs.size === 0) {
      await this.disposeSession(session);
    }

    return { ok: true, closed: 'tab', remainingTabs: session.tabs.size };
  }

  /**
   * Returns compressed page state with optional diff.
   */
  public async getState(input: GetStateInput): Promise<Record<string, unknown>> {
    const tab = await this.resolveTab(input.tabId);
    const previousSnapshot = tab.lastSnapshot;
    const snapshot = await this.refreshSnapshot(tab);
    const include = input.include ?? ['summary', 'interactive_elements', 'diff'];
    const baseSnapshot =
      input.sinceRevision !== undefined
        ? tab.snapshotHistory.find((s) => s.revision === input.sinceRevision)
        : previousSnapshot;

    if (input.sinceRevision !== undefined && !baseSnapshot) {
      const oldestRetainedRevision = tab.snapshotHistory[0]?.revision ?? snapshot.revision;
      throw new BrowserToolError(
        'STALE_STATE_REVISION',
        `Requested sinceRevision ${input.sinceRevision} is no longer retained (oldest retained is ${oldestRetainedRevision}, current is ${snapshot.revision}).`,
        true,
        [
          'call browser_get_state without sinceRevision',
          'request a fresher revision before diffing',
        ],
      );
    }

    const limitedElements = snapshot.interactiveElements.slice(
      0,
      input.maxElements ?? DEFAULT_MAX_ELEMENTS,
    );
    const state: Record<string, unknown> = { tab: this.toTabView(tab) };

    if (include.includes('summary')) {
      state.summary = {
        ...snapshot.summary,
        mainGoalArea: truncateText(
          snapshot.summary.mainGoalArea,
          input.textBudget ?? DEFAULT_TEXT_BUDGET,
        ),
        visibleMessages: snapshot.summary.visibleMessages.map((line) =>
          truncateText(line, Math.min(200, input.textBudget ?? DEFAULT_TEXT_BUDGET)),
        ),
      };
    }

    if (include.includes('interactive_elements')) {
      state.interactiveElements = limitedElements;
    }

    if (include.includes('diff')) {
      state.diff = baseSnapshot
        ? this.computeDelta(baseSnapshot, snapshot)
        : {
            fromRevision: snapshot.revision,
            toRevision: snapshot.revision,
            urlChanged: false,
            titleChanged: false,
            newElements: [],
            removedRefs: [],
            newText: [],
            alerts: [],
          };
    }

    return state;
  }

  /**
   * Returns filtered interactive elements.
   */
  public async getElements(input: GetElementsInput): Promise<{ matches: InteractiveElement[] }> {
    const tab = await this.resolveTab(input.tabId);
    const snapshot = await this.refreshSnapshot(tab);
    const query = input.query?.toLowerCase().trim();
    const role = input.role?.toLowerCase().trim();
    const candidates = input.scopeRef
      ? await this.collectElementsWithinScope(tab, input.scopeRef)
      : snapshot.interactiveElements;

    const matches = candidates.filter((el) => {
      const matchesRole = role ? el.role.toLowerCase() === role : true;
      const haystack = `${el.name} ${el.text}`.toLowerCase();
      const matchesQuery = query ? haystack.includes(query) : true;
      return matchesRole && matchesQuery;
    });

    return { matches: matches.slice(0, input.limit ?? DEFAULT_MAX_ELEMENTS) };
  }

  /**
   * Returns visible text for a page or element scope.
   */
  public async getText(input: GetTextInput): Promise<{ text: string; tab: BrowserTabView }> {
    const tab = await this.resolveTab(input.tabId);
    const maxChars = input.maxChars ?? DEFAULT_TEXT_BUDGET;

    if (!input.scopeRef) {
      const text = await this.getPageText(tab.page);
      return { text: truncateText(text, maxChars), tab: this.toTabView(tab) };
    }

    const locator = await this.getLocatorForRef(tab, input.scopeRef);
    const text = await locator.evaluate(
      (node) => (node as HTMLElement).innerText || node.textContent || '',
    );
    return { text: truncateText(text.trim(), maxChars), tab: this.toTabView(tab) };
  }

  /**
   * Returns recent network events with optional filtering.
   */
  public async getNetwork(input: GetNetworkInput): Promise<{ requests: NetworkEvent[] }> {
    const tab = await this.resolveTab(input.tabId);
    const cutoff = Date.now() - (input.sinceSeconds ?? 20) * 1000;
    const resourceTypes = input.resourceTypes?.map((v) => v.toLowerCase()) ?? [];

    const requests = tab.networkEvents.filter((ev) => {
      const matchesTime = ev.timestamp >= cutoff;
      const matchesType =
        resourceTypes.length === 0 || resourceTypes.includes(ev.resourceType.toLowerCase());
      return matchesTime && matchesType;
    });

    return { requests: requests.slice(-(input.limit ?? DEFAULT_NETWORK_LIMIT)) };
  }

  /**
   * Takes a screenshot and returns the buffer for inline delivery.
   */
  public async screenshot(input: ScreenshotInput): Promise<ScreenshotResult> {
    const tab = await this.resolveTab(input.tabId);
    await mkdir(this.options.screenshotDir, { recursive: true });

    const imageId = `img_${Date.now()}_${++this.screenshotCounter}`;
    const filePath = path.join(this.options.screenshotDir, `${imageId}.png`);

    if (input.scopeRef) {
      const locator = await this.getLocatorForRef(tab, input.scopeRef);
      await locator.screenshot({ path: filePath });
    } else {
      await tab.page.screenshot({ path: filePath, fullPage: input.fullPage ?? false });
    }

    const buffer = await readFile(filePath);

    // Trim oldest screenshots if over the limit (non-blocking, best-effort)
    void this.trimScreenshots();

    return { imageId, path: filePath, buffer };
  }

  /**
   * Clicks an indexed element and returns an incremental page delta.
   */
  public async click(input: ClickInput): Promise<ActionResult> {
    const tab = await this.resolveTab(input.tabId);
    const before = await this.refreshSnapshot(tab);
    const locator = await this.getLocatorForRef(tab, input.ref);
    await locator.click({ timeout: input.timeoutMs ?? DEFAULT_WAIT_MS });
    await this.settle(tab.page);
    const after = await this.refreshSnapshot(tab);

    return {
      ok: true,
      action: 'click',
      ref: input.ref,
      tab: this.toTabView(tab),
      delta: this.computeDelta(before, after),
    };
  }

  /**
   * Fills an indexed input element and optionally submits the form.
   */
  public async fill(input: FillInput): Promise<ActionResult> {
    const tab = await this.resolveTab(input.tabId);
    const before = await this.refreshSnapshot(tab);
    const locator = await this.getLocatorForRef(tab, input.ref);
    await locator.fill(input.value);
    if (input.submit) {
      await locator.press('Enter');
    }
    await this.settle(tab.page);
    const after = await this.refreshSnapshot(tab);

    return {
      ok: true,
      action: 'fill',
      ref: input.ref,
      tab: this.toTabView(tab),
      delta: this.computeDelta(before, after),
    };
  }

  /**
   * Presses a keyboard key on the active page.
   */
  public async press(input: PressInput): Promise<ActionResult> {
    const tab = await this.resolveTab(input.tabId);
    const before = await this.refreshSnapshot(tab);
    await tab.page.keyboard.press(input.key);
    await this.settle(tab.page);
    const after = await this.refreshSnapshot(tab);

    return {
      ok: true,
      action: 'press',
      tab: this.toTabView(tab),
      delta: this.computeDelta(before, after),
    };
  }

  /**
   * Waits for one explicit page condition.
   */
  public async waitFor(
    input: WaitForInput,
  ): Promise<{ ok: true; tab: BrowserTabView; condition: WaitCondition }> {
    const tab = await this.resolveTab(input.tabId);
    const timeoutMs = input.timeoutMs ?? DEFAULT_WAIT_MS;
    await this.waitForCondition(tab, input.condition, timeoutMs);
    await this.refreshSnapshot(tab);
    return { ok: true, tab: this.toTabView(tab), condition: input.condition };
  }

  /**
   * Saves the current browser context storage state to disk.
   */
  public async saveSession(input: SaveSessionInput): Promise<SaveSessionResult> {
    const session = this.getRequiredSession();
    const sessionPath =
      input.sessionPath ??
      path.join(this.options.screenshotDir, '..', 'sessions', `${session.id}.json`);

    await mkdir(path.dirname(sessionPath), { recursive: true });
    await session.context.storageState({ path: sessionPath });

    this.log('info', `saveSession: written to ${sessionPath}`);
    return { ok: true, path: sessionPath, savedAt: nowIso() };
  }

  /**
   * Restores a previously saved browser context storage state.
   * Creates a new session with cookie / localStorage pre-populated.
   */
  public async restoreSession(input: RestoreSessionInput): Promise<RestoreSessionResult> {
    // Dispose existing session first so there's only ever one active session.
    const existing = Array.from(this.sessions.values())[0];
    if (existing) {
      await this.disposeSession(existing);
    }

    await stat(input.sessionPath).catch(() => {
      throw new BrowserToolError(
        'INVALID_INPUT',
        `Session snapshot not found: ${input.sessionPath}`,
        false,
        ['call browser_save_session first', 'verify the path'],
      );
    });

    const browserType = getBrowserType(this.options.browserEngine);
    const browser = await browserType.launch({ headless: this.options.headless });
    this.browsers.add(browser);

    const context = await browser.newContext({
      viewport: this.options.viewport,
      storageState: input.sessionPath,
    });

    const session: SessionRecord = {
      id: `sess_${String(++this.sessionCounter).padStart(3, '0')}`,
      browserType,
      context,
      tabs: new Map<string, TabRecord>(),
      currentTabId: null,
      viewport: this.options.viewport,
      createdAt: nowIso(),
    };
    this.sessions.set(session.id, session);

    this.log('info', `restoreSession: new session ${session.id} from ${input.sessionPath}`);
    return { ok: true, restoredFrom: input.sessionPath, sessionId: session.id };
  }

  /**
   * Returns runtime diagnostic information for self-inspection.
   */
  public async getDiagnostics(): Promise<DiagnosticsResult> {
    let networkEventBufferSize = 0;
    let tabs = 0;
    for (const session of this.sessions.values()) {
      tabs += session.tabs.size;
      for (const tab of session.tabs.values()) {
        networkEventBufferSize += tab.networkEvents.length;
      }
    }

    let screenshotCount = 0;
    try {
      const files = await readdir(this.options.screenshotDir);
      screenshotCount = files.filter((f) => f.endsWith('.png')).length;
    } catch {
      // Directory might not exist yet — that's fine.
    }

    return {
      sessions: this.sessions.size,
      tabs,
      networkEventBufferSize,
      screenshotCount,
      runtimeVersion: RUNTIME_VERSION,
    };
  }

  /**
   * Disposes all sessions and browser processes.
   */
  public async shutdown(): Promise<void> {
    for (const session of Array.from(this.sessions.values())) {
      await this.disposeSession(session);
    }
    this.sessions.clear();
    this.log('info', 'shutdown complete');
  }

  // -------------------------------------------------------------------------
  // Private helpers — session / tab management
  // -------------------------------------------------------------------------

  private toSessionView(session: SessionRecord): BrowserSessionView {
    return {
      sessionId: session.id,
      createdAt: session.createdAt,
      viewport: session.viewport,
      currentTabId: session.currentTabId,
    };
  }

  private toTabView(tab: TabRecord): BrowserTabView {
    return {
      tabId: tab.id,
      url: tab.page.url(),
      title: tab.lastSnapshot?.title ?? 'Untitled',
      pageRevision: tab.pageRevision,
      loadingState: tab.loadingState,
    };
  }

  private getRequiredSession(): SessionRecord {
    const session = Array.from(this.sessions.values())[0];
    if (!session) {
      throw new BrowserToolError('SESSION_NOT_FOUND', 'No active browser session exists.', true, [
        'call browser_open',
      ]);
    }
    return session;
  }

  private async resolveTab(tabId?: string): Promise<TabRecord> {
    const session = this.getRequiredSession();
    const resolvedTabId = tabId ?? session.currentTabId;
    if (!resolvedTabId) {
      throw new BrowserToolError('TAB_NOT_FOUND', 'No active tab exists.', true, [
        'call browser_open',
      ]);
    }
    const tab = this.getRequiredTab(session, resolvedTabId);
    await this.ensurePageReady(tab.page);
    return tab;
  }

  private getRequiredTab(session: SessionRecord, tabId: string): TabRecord {
    const tab = session.tabs.get(tabId);
    if (!tab) {
      throw new BrowserToolError('TAB_NOT_FOUND', `Tab ${tabId} was not found.`, true, [
        'call browser_tabs with action=list',
      ]);
    }
    return tab;
  }

  private async getOrCreateSession(): Promise<SessionRecord> {
    const existing = Array.from(this.sessions.values())[0];
    if (existing) return existing;

    const browserType = getBrowserType(this.options.browserEngine);
    const browser = await browserType.launch({ headless: this.options.headless });
    this.browsers.add(browser);

    const context = await browser.newContext({ viewport: this.options.viewport });

    const session: SessionRecord = {
      id: `sess_${String(++this.sessionCounter).padStart(3, '0')}`,
      browserType,
      context,
      tabs: new Map<string, TabRecord>(),
      currentTabId: null,
      viewport: this.options.viewport,
      createdAt: nowIso(),
    };
    this.sessions.set(session.id, session);
    return session;
  }

  private async createTab(session: SessionRecord): Promise<TabRecord> {
    const page = await session.context.newPage();
    const tab: TabRecord = {
      id: `tab_${String(++this.tabCounter).padStart(2, '0')}`,
      page,
      pageRevision: 0,
      loadingState: 'loading',
      indexedElements: new Map<string, InteractiveElement>(),
      fingerprintToRef: new Map<string, string>(),
      lastSnapshot: null,
      snapshotHistory: [],
      networkEvents: [],
      requestStartTimes: new Map<Request, number>(),
      disposeNetworkObserver: undefined,
    };
    tab.disposeNetworkObserver = this.attachNetworkObserver(tab);
    session.tabs.set(tab.id, tab);
    session.currentTabId = tab.id;
    return tab;
  }

  private async disposeSession(session: SessionRecord): Promise<void> {
    for (const tab of session.tabs.values()) {
      tab.disposeNetworkObserver?.();
    }
    await session.context.close();
    this.sessions.delete(session.id);
    const browser = session.context.browser();
    if (browser) {
      await browser.close();
      this.browsers.delete(browser);
    }
  }

  // -------------------------------------------------------------------------
  // Network observer — enhanced with request/failed events and body capture
  // -------------------------------------------------------------------------

  private attachNetworkObserver(tab: TabRecord): () => void {
    const captureBody = this.options.captureBody;
    const requestStartTimes = tab.requestStartTimes;

    // Track request start times and optionally capture POST body
    const requestListener = (request: Request): void => {
      const id = `req_${++this.requestCounter}`;
      requestStartTimes.set(request, Date.now());

      if (captureBody) {
        const postData = request.postData();
        if (postData) {
          // Pre-insert a placeholder so we can update it on response
          const event: NetworkEvent = {
            id,
            method: request.method(),
            url: request.url(),
            status: null,
            contentType: null,
            resourceType: request.resourceType(),
            timestamp: Date.now(),
            ok: false,
            postData: postData.slice(0, BODY_CAPTURE_LIMIT),
          };
          tab.networkEvents.push(event);
          this.trimNetworkBuffer(tab);
        }
      }
    };

    const responseListener = (response: Response): void => {
      void this.handleResponse(tab, response, captureBody);
    };

    const requestFailedListener = (request: Request): void => {
      const failure = request.failure();
      const startTs = requestStartTimes.get(request);
      const durationMs = startTs != null ? Date.now() - startTs : null;
      requestStartTimes.delete(request);

      const event: NetworkEvent = {
        id: `req_${++this.requestCounter}`,
        method: request.method(),
        url: request.url(),
        status: null,
        contentType: null,
        resourceType: request.resourceType(),
        timestamp: Date.now(),
        ok: false,
        errorText: failure?.errorText ?? 'Request failed',
        durationMs,
      };
      tab.networkEvents.push(event);
      this.trimNetworkBuffer(tab);
    };

    tab.page.on('request', requestListener);
    tab.page.on('response', responseListener);
    tab.page.on('requestfailed', requestFailedListener);

    return () => {
      tab.page.off('request', requestListener);
      tab.page.off('response', responseListener);
      tab.page.off('requestfailed', requestFailedListener);
    };
  }

  private async handleResponse(
    tab: TabRecord,
    response: Response,
    captureBody: boolean,
  ): Promise<void> {
    const request = response.request();
    const headers = await response.allHeaders().catch(() => ({} as Record<string, string>));
    const requestStartTimes = tab.requestStartTimes;
    const startTs = requestStartTimes.get(request);
    const durationMs = startTs != null ? Date.now() - startTs : null;
    requestStartTimes.delete(request);

    const contentType = headers['content-type'] ?? null;

    let responseBody: string | null = null;
    if (captureBody && contentType?.includes('application/json')) {
      responseBody = await response
        .text()
        .then((t) => t.slice(0, BODY_CAPTURE_LIMIT))
        .catch(() => null);
    }

    // Find last pre-existing placeholder from request listener (captureBody mode).
    // Manual reverse loop for ES2022 compatibility (findLastIndex is ES2023).
    let existingIdx = -1;
    if (captureBody) {
      for (let i = tab.networkEvents.length - 1; i >= 0; i--) {
        const ev = tab.networkEvents[i];
        if (ev && ev.url === response.url() && ev.status === null && !ev.errorText) {
          existingIdx = i;
          break;
        }
      }
    }

    const placeholder = existingIdx >= 0 ? tab.networkEvents[existingIdx] : undefined;

    const event: NetworkEvent = {
      id: placeholder?.id ?? `req_${++this.requestCounter}`,
      method: request.method(),
      url: response.url(),
      status: response.status(),
      contentType,
      resourceType: request.resourceType(),
      timestamp: Date.now(),
      ok: response.ok(),
      postData: placeholder?.postData ?? null,
      durationMs,
      ...(responseBody != null ? { responseBody } as unknown as NetworkEvent : {}),
    };

    if (existingIdx >= 0) {
      tab.networkEvents[existingIdx] = event;
    } else {
      tab.networkEvents.push(event);
      this.trimNetworkBuffer(tab);
    }
  }

  private trimNetworkBuffer(tab: TabRecord): void {
    if (tab.networkEvents.length > MAX_NETWORK_EVENTS) {
      tab.networkEvents.splice(0, tab.networkEvents.length - MAX_NETWORK_EVENTS);
    }
  }

  // -------------------------------------------------------------------------
  // Page settlement + snapshot machinery
  // -------------------------------------------------------------------------

  private async settle(page: Page): Promise<void> {
    await this.ensurePageReady(page);
    await page.waitForLoadState('domcontentloaded');
    await page.waitForLoadState('networkidle').catch(() => undefined);
  }

  private async ensurePageReady(page: Page): Promise<void> {
    await page.waitForLoadState('domcontentloaded').catch(() => undefined);
  }

  private async refreshSnapshot(tab: TabRecord): Promise<PageSnapshot> {
    const snapshot = await this.captureSnapshot(tab);
    const hasChanged = this.hasMeaningfulChange(tab.lastSnapshot, snapshot);

    if (hasChanged || !tab.lastSnapshot) {
      tab.pageRevision += 1;
      snapshot.revision = tab.pageRevision;
      for (const el of snapshot.interactiveElements) {
        el.pageRevision = tab.pageRevision;
      }
      tab.lastSnapshot = snapshot;
      tab.snapshotHistory.push(snapshot);
      if (tab.snapshotHistory.length > SNAPSHOT_HISTORY_LIMIT) {
        tab.snapshotHistory.splice(0, tab.snapshotHistory.length - SNAPSHOT_HISTORY_LIMIT);
      }
    }

    tab.loadingState = (tab.lastSnapshot ?? snapshot).loadingState;
    tab.indexedElements = new Map(
      (tab.lastSnapshot ?? snapshot).interactiveElements.map((el) => [el.ref, el]),
    );
    tab.fingerprintToRef = new Map(
      (tab.lastSnapshot ?? snapshot).interactiveElements.map((el) => [el.fingerprint, el.ref]),
    );
    return tab.lastSnapshot ?? snapshot;
  }

  /** Compares two snapshots to decide whether a new revision is warranted. */
  private hasMeaningfulChange(previous: PageSnapshot | null, next: PageSnapshot): boolean {
    if (!previous) return true;
    if (previous.url !== next.url || previous.title !== next.title) return true;
    if (previous.textContent !== next.textContent) return true;
    // Compare fingerprints as a set (order-insensitive) to avoid ordinal-shift false positives
    const prevFps = new Set(previous.interactiveElements.map((el) => el.fingerprint));
    const nextFps = new Set(next.interactiveElements.map((el) => el.fingerprint));
    if (prevFps.size !== nextFps.size) return true;
    for (const fp of nextFps) {
      if (!prevFps.has(fp)) return true;
    }
    return false;
  }

  private async captureSnapshot(tab: TabRecord): Promise<PageSnapshot> {
    const loadingState = await this.detectLoadingState(tab.page);
    const summary = await this.buildSummary(tab.page);
    const interactiveElements = await this.collectInteractiveElements(tab);
    const textContent = truncateText(await this.getPageText(tab.page), DEFAULT_TEXT_BUDGET);

    return {
      revision: tab.pageRevision,
      url: tab.page.url(),
      title: await tab.page.title(),
      loadingState,
      summary,
      interactiveElements,
      textContent,
      textLines: toTextLines(textContent),
      createdAt: Date.now(),
    };
  }

  private async detectLoadingState(page: Page): Promise<LoadingState> {
    const readyState = await page.evaluate(() => document.readyState).catch(() => 'complete');
    if (readyState === 'loading') return 'loading';
    if (readyState === 'interactive') return 'domcontentloaded';
    return 'idle';
  }

  private async buildSummary(page: Page): Promise<PageSummary> {
    const payload = await page.evaluate(() => {
      const main = document.querySelector('main') ?? document.body;
      const mainText = (main.textContent ?? '').replace(/\s+/g, ' ').trim();
      const visibleText = (document.body.innerText ?? '').trim();
      const seen = new Set<string>();
      const messages: string[] = [];
      for (const raw of visibleText.split('\n')) {
        const line = raw.trim();
        if (line && !seen.has(line)) {
          seen.add(line);
          messages.push(line);
          if (messages.length >= 8) break;
        }
      }
      return {
        mainGoalArea: mainText,
        visibleMessages: messages,
        forms: document.querySelectorAll('form').length,
        dialogs: document.querySelectorAll('dialog,[role="dialog"],[aria-modal="true"]').length,
      };
    });

    return {
      mainGoalArea: truncateText(payload.mainGoalArea, 240),
      visibleMessages: payload.visibleMessages.map((line) => truncateText(line, 160)),
      forms: payload.forms,
      dialogs: payload.dialogs,
    };
  }

  private async getPageText(page: Page): Promise<string> {
    return page.evaluate(
      () => (document.body.innerText ?? '').replace(/\s+$/g, '').trim(),
    );
  }

  // -------------------------------------------------------------------------
  // Element collection — shared implementation (P2 dedup + P3 fingerprint)
  // -------------------------------------------------------------------------

  /**
   * Converts raw DOM descriptors into indexed InteractiveElements using a
   * shared fingerprint table. Limits output to `limit` items.
   */
  private buildInteractiveElements(
    tab: TabRecord,
    descriptors: ElementDescriptor[],
    limit: number,
  ): InteractiveElement[] {
    const fingerprintCounts = new Map<string, number>();
    return descriptors.slice(0, limit).map((descriptor) => {
      const fingerprint = createFingerprint(descriptor, fingerprintCounts);
      const ref = tab.fingerprintToRef.get(fingerprint) ?? `el_${++this.refCounter}`;
      return {
        ref,
        pageRevision: tab.pageRevision,
        fingerprint,
        role: descriptor.role,
        name: descriptor.name,
        text: descriptor.text,
        visible: descriptor.visible,
        enabled: descriptor.enabled,
        locatorHint: descriptor.locatorHint,
      };
    });
  }

  /** Collects interactive elements for the entire page. */
  private async collectInteractiveElements(tab: TabRecord): Promise<InteractiveElement[]> {
    const descriptors = await tab.page
      .locator(INTERACTIVE_SELECTOR)
      .evaluateAll(domDescriptorMapper);
    return this.buildInteractiveElements(tab, descriptors, DEFAULT_MAX_ELEMENTS * 3);
  }

  /** Collects interactive elements within a scoped locator. */
  private async collectElementsWithinScope(
    tab: TabRecord,
    scopeRef: string,
  ): Promise<InteractiveElement[]> {
    const scopeLocator = await this.getLocatorForRef(tab, scopeRef);
    const descriptors = await scopeLocator
      .locator(INTERACTIVE_SELECTOR)
      .evaluateAll(domDescriptorMapper);
    return this.buildInteractiveElements(tab, descriptors, DEFAULT_MAX_ELEMENTS * 3);
  }

  // -------------------------------------------------------------------------
  // Locator resolution
  // -------------------------------------------------------------------------

  private getIndexedElement(tab: TabRecord, ref: string): InteractiveElement {
    const element = tab.indexedElements.get(ref);
    if (!element) {
      throw new BrowserToolError('STALE_ELEMENT_REF', `Element ref ${ref} is stale or unknown.`, true, [
        'call browser_get_state',
        'call browser_get_elements',
      ]);
    }
    if (element.pageRevision !== tab.pageRevision) {
      throw new BrowserToolError(
        'STALE_ELEMENT_REF',
        `Ref ${ref} belongs to revision ${element.pageRevision}; current is ${tab.pageRevision}.`,
        true,
        ['call browser_get_state', 'call browser_get_elements'],
      );
    }
    return element;
  }

  private async getLocatorForRef(tab: TabRecord, ref: string): Promise<Locator> {
    const element = this.getIndexedElement(tab, ref);

    // Priority: testId > label > role > text
    if (element.locatorHint.testId) {
      const loc = tab.page.getByTestId(element.locatorHint.testId);
      if ((await loc.count().catch(() => 0)) > 0) return loc.first();
    }

    if (element.role === 'textbox' && element.name) {
      const loc = tab.page.getByLabel(element.name, { exact: false });
      if ((await loc.count().catch(() => 0)) > 0) return loc.first();
    }

    if ((element.role === 'button' || element.role === 'link') && element.name) {
      const loc = tab.page.getByRole(element.role as 'button' | 'link', {
        name: element.name,
        exact: false,
      });
      if ((await loc.count().catch(() => 0)) > 0) return loc.first();
    }

    if (element.text) {
      const loc = tab.page.locator(element.locatorHint.tag).filter({ hasText: element.text });
      if ((await loc.count().catch(() => 0)) > 0) return loc.first();
    }

    throw new BrowserToolError(
      'ELEMENT_NOT_VISIBLE',
      `Unable to resolve locator for ${ref} (${element.fingerprint}).`,
      true,
      ['call browser_get_state', 'call browser_get_elements', 'use a fresher ref'],
    );
  }

  // -------------------------------------------------------------------------
  // Delta computation
  // -------------------------------------------------------------------------

  private computeDelta(previous: PageSnapshot, next: PageSnapshot): PageDelta {
    const previousRefs = new Set(previous.interactiveElements.map((el) => el.ref));
    const nextRefs = new Set(next.interactiveElements.map((el) => el.ref));
    const previousText = new Set(previous.textLines);
    const alerts = next.textLines
      .filter((line) => /error|failed|invalid|warning/i.test(line))
      .slice(0, 5);

    return {
      fromRevision: previous.revision,
      toRevision: next.revision,
      urlChanged: previous.url !== next.url,
      titleChanged: previous.title !== next.title,
      newElements: next.interactiveElements
        .filter((el) => !previousRefs.has(el.ref))
        .map((el) => ({ ref: el.ref, role: el.role, name: el.name }))
        .slice(0, 10),
      removedRefs: previous.interactiveElements
        .filter((el) => !nextRefs.has(el.ref))
        .map((el) => el.ref)
        .slice(0, 10),
      newText: next.textLines.filter((line) => !previousText.has(line)).slice(0, 10),
      alerts,
    };
  }

  // -------------------------------------------------------------------------
  // Wait conditions
  // -------------------------------------------------------------------------

  private async waitForCondition(
    tab: TabRecord,
    condition: WaitCondition,
    timeoutMs: number,
  ): Promise<void> {
    switch (condition.type) {
      case 'text_appears':
        if (!condition.value) {
          throw new BrowserToolError('INVALID_INPUT', 'value is required for text_appears.', true, [
            'provide condition.value',
          ]);
        }
        await tab.page.getByText(condition.value).first().waitFor({ state: 'visible', timeout: timeoutMs });
        return;

      case 'text_disappears':
        if (!condition.value) {
          throw new BrowserToolError('INVALID_INPUT', 'value is required for text_disappears.', true, [
            'provide condition.value',
          ]);
        }
        await tab.page.getByText(condition.value).first().waitFor({ state: 'hidden', timeout: timeoutMs });
        return;

      case 'element_appears':
        if (!condition.value) {
          throw new BrowserToolError('INVALID_INPUT', 'value is required for element_appears.', true, [
            'provide element ref',
          ]);
        }
        await (await this.getLocatorForRef(tab, condition.value)).waitFor({
          state: 'visible',
          timeout: timeoutMs,
        });
        return;

      case 'element_disappears':
        if (!condition.value) {
          throw new BrowserToolError('INVALID_INPUT', 'value is required for element_disappears.', true, [
            'provide element ref',
          ]);
        }
        await (await this.getLocatorForRef(tab, condition.value)).waitFor({
          state: 'hidden',
          timeout: timeoutMs,
        });
        return;

      case 'url_contains':
        if (!condition.value) {
          throw new BrowserToolError('INVALID_INPUT', 'value is required for url_contains.', true, [
            'provide condition.value',
          ]);
        }
        await tab.page.waitForURL(`**${condition.value}**`, { timeout: timeoutMs });
        return;

      case 'network_idle':
        await tab.page.waitForLoadState('networkidle', { timeout: timeoutMs });
        return;

      default:
        throw new BrowserToolError(
          'UNSUPPORTED_OPERATION',
          `Unsupported wait condition: ${String(condition.type)}.`,
          true,
          ['use a supported condition type'],
        );
    }
  }

  // -------------------------------------------------------------------------
  // Screenshot housekeeping
  // -------------------------------------------------------------------------

  /** Removes the oldest PNG files when the screenshot directory exceeds maxScreenshots. */
  private async trimScreenshots(): Promise<void> {
    try {
      const files = await readdir(this.options.screenshotDir);
      const pngs = files.filter((f) => f.endsWith('.png'));
      if (pngs.length <= this.options.maxScreenshots) return;

      // Sort ascending by name (timestamp prefix ensures chronological order)
      pngs.sort();
      const toRemove = pngs.slice(0, pngs.length - this.options.maxScreenshots);
      for (const f of toRemove) {
        await unlink(path.join(this.options.screenshotDir, f)).catch(() => undefined);
      }
    } catch {
      // Best-effort; never let cleanup failures bubble up to the agent.
    }
  }

  // -------------------------------------------------------------------------
  // Internal logging
  // -------------------------------------------------------------------------

  private log(level: string, message: string): void {
    if (this.logEmitter) {
      try {
        this.logEmitter(level, message);
      } catch {
        // Never let log failures break tool execution.
      }
    }
  }
}
