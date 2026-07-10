export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';
export type BodyType = 'none' | 'json' | 'form' | 'text' | 'multipart';
export type AuthType = 'none' | 'bearer' | 'basic';

export interface KV {
  key: string;
  value: string;
  enabled: boolean;
  description?: string;
}

export interface MultipartField {
  key: string;
  kind: 'text' | 'file';
  value: string;
  enabled: boolean;
}

export interface HttpAuth {
  type: AuthType;
  bearerToken?: string;
  basicUser?: string;
  basicPass?: string;
}

export interface HttpRequestSpec {
  method: HttpMethod;
  url: string;
  headers: KV[];
  queryParams: KV[];
  auth: HttpAuth;
  bodyType: BodyType;
  body: string;
  multipartFields: MultipartField[];
  timeoutSeconds: number;
  followRedirects: boolean;
  verifySsl: boolean;
  saveToHistory: boolean;
}

export interface HttpResponseInfo {
  status: number;
  statusText: string;
  headers: Array<[string, string]>;
  body: string;
  bodyBytes: number;
  elapsedMs: number;
  finalUrl: string;
}

export interface HttpTab {
  id: string;
  title: string;
  orderIndex: number;
  isActive: boolean;
  spec: HttpRequestSpec;
  lastResponse: HttpResponseInfo | null;
  lastError: string | null;
  sending: boolean;
  cancelTokenId?: string;
}

export interface HttpHistoryItem {
  id: string;
  method: HttpMethod;
  url: string;
  status: number | null;
  elapsedMs: number | null;
  bodyBytes: number | null;
  spec: HttpRequestSpec;
  respSummary: string | null;
  createdAt: number;
}

export interface HttpEnv {
  id: string;
  name: string;
  isActive: boolean;
  orderIndex: number;
}

export interface HttpEnvVar {
  id: string;
  envId: string;
  key: string;
  value: string;
  enabled: boolean;
  orderIndex: number;
}

export function makeEmptySpec(): HttpRequestSpec {
  return {
    method: 'GET',
    url: '',
    headers: [],
    queryParams: [],
    auth: { type: 'none' },
    bodyType: 'none',
    body: '',
    multipartFields: [],
    timeoutSeconds: 30,
    followRedirects: true,
    verifySsl: true,
    saveToHistory: true
  };
}
