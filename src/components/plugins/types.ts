export interface InspectedPluginPackage {
  appId: string;
  name: string;
  version: string;
  publisherId: string;
  publisherName: string;
  publisherKeyId: string;
  targetOs: string;
  targetArch: string;
  hostApiMin: string;
  hostApiMax: string;
  entry: string;
  permissions: string[];
  files: Record<string, string>;
}

export interface InstalledPlugin {
  appId: string;
  name: string;
  version: string;
  publisherId: string;
  publisherName: string;
  publisherKeyId: string;
  entry: string;
  permissions: string[];
  enabled: boolean;
  installedAt: string;
}

export type CatalogTargetOs = 'macos' | 'windows' | 'linux';
export type CatalogTargetArch = 'aarch64' | 'x86_64';

export interface CatalogTarget {
  os: CatalogTargetOs;
  arch: CatalogTargetArch;
  url: string;
  fileName: string;
  size: number;
  sha256: string;
}

export interface CatalogApp {
  appId: string;
  name: string;
  description: string;
  publisherId: string;
  publisherName: string;
  icon: string;
  latestVersion: string;
  publishedAt: string;
  notes: string;
  publisherKeyId: string;
  hostApiMin: string;
  hostApiMax: string;
  permissions: string[];
  target: CatalogTarget | null;
}

export interface CatalogSnapshot {
  rootSequence: number;
  catalogSequence: number;
  generatedAt: string;
  expiresAt: string;
  expired: boolean;
  apps: CatalogApp[];
}

export interface ManagedCatalogApp extends CatalogApp {
  installedVersion: string | null;
  enabled: boolean | null;
  installedAt: string | null;
  remoteActionsAllowed: boolean;
}

export interface PluginCommandError {
  code: string;
  message: string;
  correlationId: string;
}

export interface PluginHistoryRecord {
  id: number;
  appId: string;
  action: 'install' | 'enable' | 'disable' | 'uninstall';
  version: string;
  occurredAt: string;
  result: 'success' | 'failed';
  errorCode: string | null;
  steps: string[];
}

export interface PluginApi {
  inspectPackage(path: string): Promise<InspectedPluginPackage>;
  installPackage(path: string): Promise<InstalledPlugin>;
  listInstalled(): Promise<InstalledPlugin[]>;
  listHistory(): Promise<PluginHistoryRecord[]>;
  listCatalog(): Promise<CatalogSnapshot | null>;
  refreshCatalog(): Promise<CatalogSnapshot>;
  setEnabled(appId: string, enabled: boolean): Promise<void>;
  uninstall(appId: string, deleteData: boolean): Promise<void>;
}
