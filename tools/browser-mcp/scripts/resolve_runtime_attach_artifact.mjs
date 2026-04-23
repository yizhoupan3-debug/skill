#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const TRACE_RESUME_MANIFEST_SCHEMA_VERSION = 'runtime-resume-manifest-v1';
const RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION = 'runtime-event-transport-v1';
const DEFAULT_SEARCH_ROOTS = [
  path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    '..',
    '..',
    '..',
    'framework_runtime',
    'artifacts',
    'scratch',
  ),
];

function parseArgs(argv) {
  let searchRoot = null;
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === '--search-root' && index + 1 < argv.length) {
      searchRoot = path.resolve(argv[index + 1]);
      index += 1;
    }
  }
  return { searchRoot };
}

function parseIsoEpoch(value) {
  if (typeof value !== 'string' || value.trim().length === 0) {
    return 0;
  }
  const parsed = Date.parse(value.trim().replace('Z', '+00:00'));
  return Number.isNaN(parsed) ? 0 : parsed / 1000;
}

function parseJsonObject(raw) {
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function buildManifestCandidate(payload, { sourcePath, attachPath, recencyHint }) {
  if (payload.schema_version !== TRACE_RESUME_MANIFEST_SCHEMA_VERSION) {
    return null;
  }
  const eventTransportPath =
    typeof payload.event_transport_path === 'string' ? payload.event_transport_path.trim() : '';
  if (!eventTransportPath) {
    return null;
  }
  return {
    attachPath: attachPath || sourcePath,
    sourceKind: 'resume_manifest',
    sourcePath,
    updatedAtEpoch: parseIsoEpoch(payload.updated_at),
    recencyHint,
  };
}

function buildBindingCandidate(payload, { sourcePath, fallbackAttachPath, recencyHint }) {
  if (payload.schema_version !== RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION) {
    return null;
  }
  if (payload.binding_backend_family !== 'sqlite') {
    return null;
  }
  const explicitAttachPath =
    typeof payload.binding_artifact_path === 'string' ? payload.binding_artifact_path.trim() : '';
  const attachPath = explicitAttachPath || (typeof fallbackAttachPath === 'string' ? fallbackAttachPath.trim() : '');
  if (!attachPath) {
    return null;
  }
  return {
    attachPath,
    sourceKind: 'binding_artifact',
    sourcePath,
    updatedAtEpoch: 0,
    recencyHint,
  };
}

function* walkFiles(root) {
  let entries;
  try {
    entries = readdirSync(root, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      yield* walkFiles(fullPath);
      continue;
    }
    if (entry.isFile()) {
      yield fullPath;
    }
  }
}

function* iterFilesystemCandidates(searchRoot) {
  for (const filePath of walkFiles(searchRoot)) {
    const fileName = path.basename(filePath);
    const inBindingDir = path.basename(path.dirname(filePath)) === 'runtime_event_transports';
    if (fileName !== 'TRACE_RESUME_MANIFEST.json' && !inBindingDir) {
      continue;
    }
    let stats;
    let payload;
    try {
      stats = statSync(filePath);
      payload = parseJsonObject(readFileSync(filePath, 'utf8'));
    } catch {
      continue;
    }
    if (!payload) {
      continue;
    }
    const recencyHint = stats.mtimeMs;
    const resolvedPath = path.resolve(filePath);
    const candidate =
      fileName === 'TRACE_RESUME_MANIFEST.json'
        ? buildManifestCandidate(payload, {
            sourcePath: resolvedPath,
            attachPath: resolvedPath,
            recencyHint,
          })
        : buildBindingCandidate(payload, {
            sourcePath: resolvedPath,
            fallbackAttachPath: resolvedPath,
            recencyHint,
          });
    if (candidate) {
      yield candidate;
    }
  }
}

function readSqliteRows(dbPath) {
  const query = `
SELECT rowid, payload_key, payload_text
FROM runtime_storage_payloads
WHERE payload_key LIKE '%TRACE_RESUME_MANIFEST.json'
   OR payload_key LIKE '%runtime_event_transports/%.json'
`;
  const result = spawnSync('sqlite3', ['-readonly', '-json', dbPath, query], {
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    return [];
  }
  try {
    const parsed = JSON.parse(result.stdout);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function* iterSqliteCandidates(searchRoot) {
  for (const filePath of walkFiles(searchRoot)) {
    if (path.basename(filePath) !== 'runtime_checkpoint_store.sqlite3') {
      continue;
    }
    let stats;
    try {
      stats = statSync(filePath);
    } catch {
      continue;
    }
    for (const row of readSqliteRows(filePath)) {
      if (!row || typeof row !== 'object') {
        continue;
      }
      const payload = parseJsonObject(typeof row.payload_text === 'string' ? row.payload_text : '');
      if (!payload) {
        continue;
      }
      const payloadKey = typeof row.payload_key === 'string' ? row.payload_key : '';
      const rowId = Number.isFinite(row.rowid) ? Number(row.rowid) : 0;
      const sourcePath = `${path.resolve(filePath)}::${payloadKey}`;
      const recencyHint = stats.mtimeMs + rowId / 1_000_000;
      const manifest = buildManifestCandidate(payload, {
        sourcePath,
        attachPath: payloadKey || sourcePath,
        recencyHint,
      });
      if (manifest) {
        yield manifest;
        continue;
      }
      const binding = buildBindingCandidate(payload, {
        sourcePath,
        fallbackAttachPath: payloadKey || null,
        recencyHint,
      });
      if (binding) {
        yield binding;
      }
    }
  }
}

function compareCandidates(left, right) {
  const leftRank = [
    left.updatedAtEpoch,
    left.recencyHint,
    left.sourceKind === 'resume_manifest' ? 1 : 0,
    left.attachPath,
  ];
  const rightRank = [
    right.updatedAtEpoch,
    right.recencyHint,
    right.sourceKind === 'resume_manifest' ? 1 : 0,
    right.attachPath,
  ];
  for (let index = 0; index < leftRank.length; index += 1) {
    if (leftRank[index] > rightRank[index]) {
      return 1;
    }
    if (leftRank[index] < rightRank[index]) {
      return -1;
    }
  }
  return 0;
}

function resolveRuntimeAttachArtifact(searchRoot) {
  const searchRoots = Array.isArray(searchRoot) ? searchRoot : [searchRoot];
  const deduped = new Map();
  for (const root of searchRoots) {
    for (const candidate of [
      ...iterFilesystemCandidates(root),
      ...iterSqliteCandidates(root),
    ]) {
      const current = deduped.get(candidate.attachPath);
      if (!current || compareCandidates(candidate, current) > 0) {
        deduped.set(candidate.attachPath, candidate);
      }
    }
  }
  let selected = null;
  for (const candidate of deduped.values()) {
    if (!selected || compareCandidates(candidate, selected) > 0) {
      selected = candidate;
    }
  }
  return selected ? selected.attachPath : null;
}

const { searchRoot } = parseArgs(process.argv.slice(2));
const attachPath = resolveRuntimeAttachArtifact(searchRoot || DEFAULT_SEARCH_ROOTS);
if (!attachPath) {
  process.exit(1);
}
process.stdout.write(`${attachPath}\n`);
