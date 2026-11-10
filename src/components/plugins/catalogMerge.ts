import type {
  CatalogSnapshot,
  InstalledPlugin,
  ManagedCatalogApp
} from './types';

export function mergeCatalogApps(
  snapshot: CatalogSnapshot,
  installed: InstalledPlugin[]
): ManagedCatalogApp[] {
  const installedById = new Map(installed.map((app) => [app.appId, app]));
  return snapshot.apps.map((app) => {
    const local = installedById.get(app.appId);
    return {
      ...app,
      installedVersion: local?.version ?? null,
      enabled: local?.enabled ?? null,
      installedAt: local?.installedAt ?? null,
      remoteActionsAllowed: !snapshot.expired && app.target !== null
    };
  });
}
