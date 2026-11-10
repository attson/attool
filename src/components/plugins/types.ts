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
  setEnabled(appId: string, enabled: boolean): Promise<void>;
  uninstall(appId: string, deleteData: boolean): Promise<void>;
}
