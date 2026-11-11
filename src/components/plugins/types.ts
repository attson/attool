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
  updateAvailable: boolean;
  addedPermissions: string[];
  removedPermissions: string[];
}

export interface PluginCommandError {
  code: string;
  message: string;
  correlationId: string;
}

export interface PluginHistoryRecord {
  id: number;
  appId: string;
  action: 'install' | 'update' | 'rollback' | 'enable' | 'disable' | 'uninstall';
  version: string;
  occurredAt: string;
  result: 'success' | 'failed' | 'rolled-back';
  errorCode: string | null;
  operationId: string | null;
  correlationId: string | null;
  steps: string[];
  finalAvailableVersion: string | null;
}

export type PluginOperationStage =
  | 'download'
  | 'verify'
  | 'staging'
  | 'health-check'
  | 'activate'
  | 'complete'
  | 'rolled-back'
  | 'error';

export interface PluginOperationSnapshot {
  operationId: string;
  correlationId: string;
  appId: string;
  name: string;
  version: string;
  publisherId: string;
  publisherName: string;
  publisherKeyId: string;
  entry: string | null;
  permissions: string[];
  stage: PluginOperationStage;
  steps: PluginOperationStage[];
  downloadedBytes: number;
  totalBytes: number;
  errorCode: string | null;
  message: string | null;
  finalAvailableVersion: string | null;
}

export interface CatalogInstallResult {
  operation: PluginOperationSnapshot;
  installed: InstalledPlugin;
}

export interface PluginApi {
  inspectPackage(path: string): Promise<InspectedPluginPackage>;
  installPackage(path: string): Promise<InstalledPlugin>;
  listInstalled(): Promise<InstalledPlugin[]>;
  listHistory(): Promise<PluginHistoryRecord[]>;
  listCatalog(): Promise<CatalogSnapshot | null>;
  refreshCatalog(): Promise<CatalogSnapshot>;
  installCatalogApp(appId: string, acceptedPermissions: string[]): Promise<PluginOperationSnapshot>;
  updateCatalogApp(appId: string, acceptedAddedPermissions: string[]): Promise<PluginOperationSnapshot>;
  completeCatalogInstall(operationId: string): Promise<CatalogInstallResult>;
  failCatalogInstall(operationId: string): Promise<PluginOperationSnapshot>;
  getOperation(operationId: string): Promise<PluginOperationSnapshot>;
  getLatestOperation(): Promise<PluginOperationSnapshot | null>;
  listenOperations(listener: (operation: PluginOperationSnapshot) => void): Promise<() => void>;
  confirmPluginLaunch(appId: string, version: string): Promise<boolean>;
  rollbackPluginLaunch(appId: string, version: string): Promise<InstalledPlugin | null>;
  setEnabled(appId: string, enabled: boolean): Promise<void>;
  uninstall(appId: string, deleteData: boolean): Promise<void>;
}
