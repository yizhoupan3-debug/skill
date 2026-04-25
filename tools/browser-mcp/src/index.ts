import { parseArgs } from 'node:util';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { createBrowserMcpServer } from './server.js';
import { BrowserRuntime } from './runtime.js';

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

const { values: args } = parseArgs({
  options: {
    headless: { type: 'string', default: 'true' },
    engine: { type: 'string', default: 'chromium' },
    'capture-body': { type: 'boolean', default: false },
    'runtime-attach-artifact-path': { type: 'string' },
    'runtime-attach-descriptor-path': { type: 'string' },
  },
  strict: false,
});

const headless = String(args['headless'] ?? 'true') !== 'false';
const engine = String(args['engine'] ?? 'chromium') as 'chromium' | 'firefox' | 'webkit';
const captureBody = Boolean(args['capture-body']);
const runtimeAttachArtifactPath =
  typeof args['runtime-attach-artifact-path'] === 'string'
    ? String(args['runtime-attach-artifact-path'])
    : process.env.BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH ?? null;
const runtimeAttachDescriptorPath =
  typeof args['runtime-attach-descriptor-path'] === 'string'
    ? String(args['runtime-attach-descriptor-path'])
    : process.env.BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH ?? null;
const runtimeAttachSource =
  runtimeAttachDescriptorPath ??
  runtimeAttachArtifactPath ??
  'off';

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

/**
 * Starts the browser-mcp server over stdio.
 */
async function main(): Promise<void> {
  const runtime = new BrowserRuntime({
    headless,
    browserEngine: engine,
    captureBody,
    runtimeAttachArtifactPath,
    runtimeAttachDescriptorPath,
  });

  const server = createBrowserMcpServer(runtime);

  const shutdown = async (): Promise<void> => {
    await runtime.shutdown();
  };

  process.on('SIGINT', () => void shutdown().finally(() => process.exit(0)));
  process.on('SIGTERM', () => void shutdown().finally(() => process.exit(0)));

  const stdioTransport = new StdioServerTransport();
  await server.connect(stdioTransport);
  console.error(
    `browser-mcp stdio server running [engine=${engine} headless=${headless} captureBody=${captureBody} runtimeAttach=${runtimeAttachSource}]`,
  );
}

void main().catch((error) => {
  console.error('Fatal error in main():', error);
  process.exit(1);
});
