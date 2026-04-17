import { readFile } from 'node:fs/promises';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import type { CallToolResult } from '@modelcontextprotocol/sdk/types.js';
import * as z from 'zod';
import { normalizeError } from './errors.js';
import { BrowserRuntime } from './runtime.js';

// ---------------------------------------------------------------------------
// Result builders
// ---------------------------------------------------------------------------

/**
 * Builds a success tool result with both human-readable and machine-readable content.
 * @param data Structured payload.
 * @returns MCP call result.
 */
function createSuccessResult(data: unknown): CallToolResult {
  return {
    content: [{ type: 'text', text: JSON.stringify(data, null, 2) }],
    structuredContent: data as Record<string, unknown>,
  };
}

/**
 * Builds an error tool result.
 * @param error Unknown thrown value.
 * @returns MCP error result.
 */
function createErrorResult(error: unknown): CallToolResult {
  const normalized = normalizeError(error);
  const payload = { ok: false, error: normalized.toPayload() };
  return {
    isError: true,
    content: [{ type: 'text', text: JSON.stringify(payload, null, 2) }],
    structuredContent: payload,
  };
}

/**
 * Builds a tool result that includes an inline PNG image plus JSON metadata.
 * Falls back to text-only if the buffer is missing.
 * @param imageId Unique image identifier.
 * @param filePath Disk path of the screenshot.
 * @param buffer Raw PNG bytes.
 * @returns MCP call result with image content.
 */
function createScreenshotResult(
  imageId: string,
  filePath: string,
  buffer: Buffer,
): CallToolResult {
  const meta = { imageId, path: filePath };
  return {
    content: [
      {
        type: 'image',
        data: buffer.toString('base64'),
        mimeType: 'image/png',
      },
      { type: 'text', text: JSON.stringify(meta, null, 2) },
    ],
    structuredContent: meta,
  };
}

// ---------------------------------------------------------------------------
// Server factory
// ---------------------------------------------------------------------------

/**
 * Registers all browser tools on one MCP server.
 * @param runtime Browser runtime instance.
 * @returns Configured MCP server.
 */
export function createBrowserMcpServer(runtime = new BrowserRuntime()): McpServer {
  const server = new McpServer(
    { name: 'browser-mcp', version: '0.2.0' },
    { capabilities: { logging: {} } },
  );

  // Wire runtime log emitter → MCP server notifications
  runtime.setLogEmitter((level, message) => {
    void server.server.sendLoggingMessage({ level: level as 'info' | 'warning' | 'error', data: message });
  });

  // -------------------------------------------------------------------------
  // Navigation
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_open',
    {
      title: 'Open Browser Page',
      description: 'Open a page in the current browser session and return the active tab.',
      inputSchema: z.object({
        url: z.string().url(),
        newTab: z.boolean().optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.open(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_tabs',
    {
      title: 'List Or Select Tabs',
      description: 'List current tabs or switch the active tab.',
      inputSchema: z.object({
        action: z.enum(['list', 'select']),
        tabId: z.string().optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.tabs(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_close',
    {
      title: 'Close Tab Or Session',
      description: 'Close a single tab or the entire session.',
      inputSchema: z.object({
        target: z.enum(['tab', 'session']),
        tabId: z.string().optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.close(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  // -------------------------------------------------------------------------
  // Page inspection
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_get_state',
    {
      title: 'Get Compressed Page State',
      description:
        'Return a compressed page summary, interactive elements, and an optional diff. ' +
        'Pass sinceRevision to get only what changed since that snapshot.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        include: z.array(z.enum(['summary', 'interactive_elements', 'diff'])).optional(),
        sinceRevision: z.number().int().nonnegative().optional(),
        maxElements: z.number().int().positive().max(100).optional(),
        textBudget: z.number().int().positive().max(4000).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.getState(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_get_elements',
    {
      title: 'Get Interactive Elements',
      description: 'Return filtered interactive elements using role and text query.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        role: z.string().optional(),
        query: z.string().optional(),
        scopeRef: z.string().optional(),
        limit: z.number().int().positive().max(100).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.getElements(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_get_text',
    {
      title: 'Get Visible Text',
      description: 'Return visible text for the page or a specific element scope.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        scopeRef: z.string().optional(),
        maxChars: z.number().int().positive().max(8000).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.getText(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_get_network',
    {
      title: 'Get Recent Network Requests',
      description:
        'Return recent network requests including status, timing, and optional bodies. ' +
        'Failed requests (no response) are included with errorText set.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        sinceSeconds: z.number().int().positive().max(3600).optional(),
        resourceTypes: z.array(z.string()).optional(),
        limit: z.number().int().positive().max(100).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.getNetwork(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  // -------------------------------------------------------------------------
  // Screenshot — P1: returns inline base64 image content
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_screenshot',
    {
      title: 'Take Screenshot',
      description:
        'Take a screenshot and return it as an inline image. ' +
        'The image is delivered directly in the tool result so the agent can inspect it without a separate file read.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        scopeRef: z.string().optional(),
        fullPage: z.boolean().optional(),
      }),
    },
    async (input) => {
      try {
        const result = await runtime.screenshot(input);
        return createScreenshotResult(result.imageId, result.path, result.buffer);
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  // -------------------------------------------------------------------------
  // Interaction tools
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_click',
    {
      title: 'Click Element',
      description: 'Click an indexed element and return an incremental page delta.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        ref: z.string(),
        timeoutMs: z.number().int().positive().max(60000).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.click(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_fill',
    {
      title: 'Fill Element',
      description: 'Fill an indexed input-like element and optionally submit it.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        ref: z.string(),
        value: z.string(),
        submit: z.boolean().optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.fill(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_press',
    {
      title: 'Press Key',
      description: 'Press a keyboard key on the active page.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        key: z.string(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.press(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_wait_for',
    {
      title: 'Wait For Condition',
      description: 'Wait for one explicit page condition without re-reading the whole page.',
      inputSchema: z.object({
        tabId: z.string().optional(),
        condition: z.object({
          type: z.enum([
            'text_appears',
            'text_disappears',
            'element_appears',
            'element_disappears',
            'url_contains',
            'network_idle',
          ]),
          value: z.string().optional(),
        }),
        timeoutMs: z.number().int().positive().max(60000).optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.waitFor(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  // -------------------------------------------------------------------------
  // Session persistence (P6)
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_save_session',
    {
      title: 'Save Session State',
      description:
        'Save the current browser session (cookies, localStorage) to a JSON file on disk. ' +
        'Use browser_restore_session to reload it later and skip re-login.',
      inputSchema: z.object({
        sessionPath: z.string().optional(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.saveSession(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  server.registerTool(
    'browser_restore_session',
    {
      title: 'Restore Session State',
      description:
        'Restore a previously saved browser session from disk. ' +
        'The current session is disposed and a new browser context is created with the saved cookies / localStorage.',
      inputSchema: z.object({
        sessionPath: z.string(),
      }),
    },
    async (input) => {
      try {
        return createSuccessResult(await runtime.restoreSession(input));
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  // -------------------------------------------------------------------------
  // Diagnostics (P7)
  // -------------------------------------------------------------------------

  server.registerTool(
    'browser_diagnostics',
    {
      title: 'Runtime Diagnostics',
      description:
        'Return runtime health information: active sessions, tabs, network buffer size, ' +
        'screenshot count, and version. Useful for self-inspection and debugging.',
      inputSchema: z.object({}),
    },
    async () => {
      try {
        return createSuccessResult(await runtime.getDiagnostics());
      } catch (error) {
        return createErrorResult(error);
      }
    },
  );

  return server;
}
