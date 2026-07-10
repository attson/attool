import { invoke } from '@tauri-apps/api/core';
import type {
  HttpEnv,
  HttpEnvVar,
  HttpHistoryItem,
  HttpRequestSpec,
  HttpResponseInfo,
  HttpTab
} from './types';

interface HttpTabRow {
  id: string;
  title: string;
  orderIndex: number;
  isActive: boolean;
  specJson: string;
  updatedAt: number;
}

interface HttpHistoryRow {
  id: string;
  method: string;
  url: string;
  status: number | null;
  elapsedMs: number | null;
  bodyBytes: number | null;
  specJson: string;
  respSummary: string | null;
  createdAt: number;
}

interface HttpEnvRow {
  id: string;
  name: string;
  isActive: boolean;
  orderIndex: number;
  updatedAt: number;
}

interface HttpEnvVarRow {
  id: string;
  envId: string;
  key: string;
  value: string;
  enabled: boolean;
  orderIndex: number;
  updatedAt: number;
}

function tabFromRow(row: HttpTabRow): HttpTab {
  return {
    id: row.id,
    title: row.title,
    orderIndex: row.orderIndex,
    isActive: row.isActive,
    spec: JSON.parse(row.specJson) as HttpRequestSpec,
    lastResponse: null,
    lastError: null,
    sending: false
  };
}

function tabToRow(tab: HttpTab): HttpTabRow {
  return {
    id: tab.id,
    title: tab.title,
    orderIndex: tab.orderIndex,
    isActive: tab.isActive,
    specJson: JSON.stringify(tab.spec),
    updatedAt: Date.now()
  };
}

function historyFromRow(row: HttpHistoryRow): HttpHistoryItem {
  return {
    id: row.id,
    method: row.method as HttpHistoryItem['method'],
    url: row.url,
    status: row.status,
    elapsedMs: row.elapsedMs,
    bodyBytes: row.bodyBytes,
    spec: JSON.parse(row.specJson) as HttpRequestSpec,
    respSummary: row.respSummary,
    createdAt: row.createdAt
  };
}

function historyToRow(item: HttpHistoryItem): HttpHistoryRow {
  return {
    id: item.id,
    method: item.method,
    url: item.url,
    status: item.status,
    elapsedMs: item.elapsedMs,
    bodyBytes: item.bodyBytes,
    specJson: JSON.stringify(item.spec),
    respSummary: item.respSummary,
    createdAt: item.createdAt
  };
}

function envFromRow(row: HttpEnvRow): HttpEnv {
  return { id: row.id, name: row.name, isActive: row.isActive, orderIndex: row.orderIndex };
}

function envVarFromRow(row: HttpEnvVarRow): HttpEnvVar {
  return {
    id: row.id,
    envId: row.envId,
    key: row.key,
    value: row.value,
    enabled: row.enabled,
    orderIndex: row.orderIndex
  };
}

export interface HttpApi {
  sendHttp(spec: HttpRequestSpec, cancelTokenId?: string): Promise<HttpResponseInfo>;
  cancelHttp(cancelTokenId: string): Promise<boolean>;

  listTabs(): Promise<HttpTab[]>;
  upsertTab(tab: HttpTab): Promise<void>;
  deleteTab(id: string): Promise<void>;
  setActiveTab(id: string): Promise<void>;

  listHistory(limit?: number): Promise<HttpHistoryItem[]>;
  insertHistory(item: HttpHistoryItem): Promise<void>;
  deleteHistory(id: string): Promise<void>;
  clearHistory(): Promise<void>;

  listEnvs(): Promise<HttpEnv[]>;
  upsertEnv(env: HttpEnv): Promise<void>;
  deleteEnv(id: string): Promise<void>;
  setActiveEnv(id: string): Promise<void>;

  listEnvVars(envId: string): Promise<HttpEnvVar[]>;
  upsertEnvVar(v: HttpEnvVar): Promise<void>;
  deleteEnvVar(id: string): Promise<void>;
}

export function createHttpApi(invoker = invoke): HttpApi {
  return {
    async sendHttp(spec, cancelTokenId) {
      return await invoker<HttpResponseInfo>('send_http', { request: spec, cancelTokenId });
    },
    async cancelHttp(cancelTokenId) {
      return await invoker<boolean>('cancel_http', { cancelTokenId });
    },

    async listTabs() {
      const rows = await invoker<HttpTabRow[]>('list_http_tabs');
      return rows.map(tabFromRow);
    },
    async upsertTab(tab) {
      await invoker('upsert_http_tab', { row: tabToRow(tab) });
    },
    async deleteTab(id) {
      await invoker('delete_http_tab', { id });
    },
    async setActiveTab(id) {
      await invoker('set_active_http_tab', { id });
    },

    async listHistory(limit) {
      const rows = await invoker<HttpHistoryRow[]>('list_http_history', { limit: limit ?? 500 });
      return rows.map(historyFromRow);
    },
    async insertHistory(item) {
      await invoker('insert_http_history', { row: historyToRow(item) });
    },
    async deleteHistory(id) {
      await invoker('delete_http_history', { id });
    },
    async clearHistory() {
      await invoker('clear_http_history');
    },

    async listEnvs() {
      const rows = await invoker<HttpEnvRow[]>('list_http_envs');
      return rows.map(envFromRow);
    },
    async upsertEnv(env) {
      await invoker('upsert_http_env', {
        row: {
          id: env.id,
          name: env.name,
          isActive: env.isActive,
          orderIndex: env.orderIndex,
          updatedAt: Date.now()
        }
      });
    },
    async deleteEnv(id) {
      await invoker('delete_http_env', { id });
    },
    async setActiveEnv(id) {
      await invoker('set_active_http_env', { id });
    },

    async listEnvVars(envId) {
      const rows = await invoker<HttpEnvVarRow[]>('list_http_env_vars', { envId });
      return rows.map(envVarFromRow);
    },
    async upsertEnvVar(v) {
      await invoker('upsert_http_env_var', {
        row: {
          id: v.id,
          envId: v.envId,
          key: v.key,
          value: v.value,
          enabled: v.enabled,
          orderIndex: v.orderIndex,
          updatedAt: Date.now()
        }
      });
    },
    async deleteEnvVar(id) {
      await invoker('delete_http_env_var', { id });
    }
  };
}
