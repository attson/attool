import type { HttpMethod, HttpRequestSpec } from './types';
import { makeEmptySpec } from './types';

const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'] as const;

interface OpenApiParameter {
  name?: string;
  in?: string;
  description?: string;
  schema?: JsonSchema;
  example?: unknown;
}

interface JsonSchema {
  type?: string;
  properties?: Record<string, JsonSchema>;
  items?: JsonSchema;
  required?: string[];
  example?: unknown;
  default?: unknown;
  enum?: unknown[];
}

interface OpenApiOperation {
  operationId?: string;
  summary?: string;
  parameters?: OpenApiParameter[];
  requestBody?: {
    content?: Record<string, { schema?: JsonSchema; example?: unknown }>;
  };
}

interface OpenApiDocument {
  openapi?: string;
  info?: { title?: string };
  servers?: Array<{ url?: string }>;
  paths?: Record<string, Record<string, OpenApiOperation | unknown>>;
}

export interface ImportedCollection {
  id: string;
  name: string;
  orderIndex: number;
}

export interface ImportedFolder {
  id: string;
  collectionId: string;
  parentId: string | null;
  name: string;
  orderIndex: number;
}

export interface ImportedRequest {
  id: string;
  collectionId: string;
  folderId: string | null;
  name: string;
  method: HttpMethod;
  spec: HttpRequestSpec;
  orderIndex: number;
}

export interface ImportedOpenApiCollection {
  collection: ImportedCollection;
  folders: ImportedFolder[];
  requests: ImportedRequest[];
  baseUrl: string;
}

export interface OpenApiImportOptions {
  baseUrl?: string;
  collectionName?: string;
}

let nextId = 0;

function makeId(prefix: string): string {
  nextId += 1;
  return `${prefix}-${Date.now().toString(36)}-${nextId.toString(36)}`;
}

export function parseOpenApiToCollection(input: string, options: OpenApiImportOptions = {}): ImportedOpenApiCollection {
  let doc: OpenApiDocument;
  try {
    doc = JSON.parse(input) as OpenApiDocument;
  } catch {
    throw new Error('OpenAPI JSON 解析失败');
  }

  if (!doc.openapi?.startsWith('3.')) {
    throw new Error('只支持 OpenAPI 3.x JSON');
  }

  const collectionId = makeId('col');
  const baseUrl = normalizeBaseUrl(options.baseUrl ?? doc.servers?.find((s) => s.url)?.url ?? '');
  const collection: ImportedCollection = {
    id: collectionId,
    name: (options.collectionName || doc.info?.title || 'OpenAPI Collection').trim(),
    orderIndex: 0
  };
  const folders: ImportedFolder[] = [];
  const requests: ImportedRequest[] = [];
  const folderByName = new Map<string, ImportedFolder>();

  for (const [path, pathItem] of Object.entries(doc.paths ?? {})) {
    if (!pathItem || typeof pathItem !== 'object') continue;
    const folderName = firstPathSegment(path);
    let folder: ImportedFolder | null = null;
    if (folderName) {
      folder = folderByName.get(folderName) ?? null;
      if (!folder) {
        folder = {
          id: makeId('fld'),
          collectionId,
          parentId: null,
          name: folderName,
          orderIndex: folders.length
        };
        folderByName.set(folderName, folder);
        folders.push(folder);
      }
    }

    for (const [methodKey, operationRaw] of Object.entries(pathItem as Record<string, unknown>)) {
      const method = methodKey.toUpperCase();
      if (!isHttpMethod(method) || !operationRaw || typeof operationRaw !== 'object') continue;
      const operation = operationRaw as OpenApiOperation;
      const spec = operationToSpec(method, baseUrl, path, operation);
      requests.push({
        id: makeId('req'),
        collectionId,
        folderId: folder?.id ?? null,
        name: `${method} ${operation.summary || operation.operationId || path}`.slice(0, 80),
        method,
        spec,
        orderIndex: requests.length
      });
    }
  }

  if (requests.length === 0) {
    throw new Error('没有可导入的 HTTP operation');
  }

  return { collection, folders, requests, baseUrl };
}

function operationToSpec(method: HttpMethod, baseUrl: string, path: string, operation: OpenApiOperation): HttpRequestSpec {
  const spec = makeEmptySpec();
  spec.method = method;
  spec.url = joinUrl(baseUrl, replacePathParams(path));
  spec.queryParams = [];
  spec.headers = [];

  for (const parameter of operation.parameters ?? []) {
    if (!parameter.name) continue;
    const value = exampleValue(parameter.example ?? parameter.schema?.example ?? parameter.schema?.default);
    const row = {
      key: parameter.name,
      value,
      enabled: true,
      description: parameter.description ?? ''
    };
    if (parameter.in === 'query') spec.queryParams.push(row);
    else if (parameter.in === 'header') spec.headers.push(row);
  }

  const jsonBody = operation.requestBody?.content?.['application/json'];
  if (jsonBody) {
    spec.bodyType = 'json';
    spec.body = JSON.stringify(
      jsonBody.example ?? schemaExample(jsonBody.schema),
      null,
      2
    );
  }

  return spec;
}

function isHttpMethod(value: string): value is HttpMethod {
  return (METHODS as readonly string[]).includes(value);
}

function firstPathSegment(path: string): string {
  return path.split('/').filter(Boolean)[0]?.replace(/[{}]/g, '') ?? '';
}

function replacePathParams(path: string): string {
  return path.replace(/\{([^}]+)\}/g, '{{$1}}');
}

function normalizeBaseUrl(url: string): string {
  return url.trim().replace(/\/+$/, '');
}

function joinUrl(baseUrl: string, path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return baseUrl ? `${baseUrl}${normalizedPath}` : normalizedPath;
}

function exampleValue(value: unknown): string {
  if (value === undefined || value === null) return '';
  return String(value);
}

function schemaExample(schema?: JsonSchema): unknown {
  if (!schema) return {};
  if (schema.example !== undefined) return schema.example;
  if (schema.default !== undefined) return schema.default;
  if (schema.enum?.length) return schema.enum[0];
  if (schema.type === 'array') return [schemaExample(schema.items)];
  if (schema.type === 'integer' || schema.type === 'number') return 0;
  if (schema.type === 'boolean') return false;
  if (schema.type === 'object' || schema.properties) {
    const out: Record<string, unknown> = {};
    for (const [key, child] of Object.entries(schema.properties ?? {})) {
      out[key] = schemaExample(child);
    }
    return out;
  }
  return 'string';
}
