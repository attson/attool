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
    const updateAvailable = Boolean(
      local && compareSemver(app.latestVersion, local.version) > 0
    );
    return {
      ...app,
      installedVersion: local?.version ?? null,
      enabled: local?.enabled ?? null,
      installedAt: local?.installedAt ?? null,
      remoteActionsAllowed: !snapshot.expired && app.target !== null,
      updateAvailable,
      addedPermissions: local
        ? app.permissions.filter((permission) => !local.permissions.includes(permission))
        : app.permissions,
      removedPermissions: local
        ? local.permissions.filter((permission) => !app.permissions.includes(permission))
        : []
    };
  });
}

export function permissionConfirmationComplete(expected: string[], accepted: string[]) {
  const expectedSet = new Set(expected);
  const acceptedSet = new Set(accepted);
  return expectedSet.size === expected.length &&
    acceptedSet.size === accepted.length &&
    expectedSet.size === acceptedSet.size &&
    [...expectedSet].every((permission) => acceptedSet.has(permission));
}

function compareSemver(left: string, right: string) {
  const [leftCore, leftPre] = splitSemver(left);
  const [rightCore, rightPre] = splitSemver(right);
  const leftParts = leftCore.split('.');
  const rightParts = rightCore.split('.');
  for (let index = 0; index < 3; index += 1) {
    const comparison = compareNumeric(leftParts[index] ?? '0', rightParts[index] ?? '0');
    if (comparison) return comparison;
  }
  if (leftPre === undefined || rightPre === undefined) {
    return leftPre === rightPre ? 0 : leftPre === undefined ? 1 : -1;
  }
  const leftIdentifiers = leftPre.split('.');
  const rightIdentifiers = rightPre.split('.');
  for (let index = 0; index < Math.max(leftIdentifiers.length, rightIdentifiers.length); index += 1) {
    const leftId = leftIdentifiers[index];
    const rightId = rightIdentifiers[index];
    if (leftId === undefined || rightId === undefined) return leftId === undefined ? -1 : 1;
    if (leftId === rightId) continue;
    const leftNumeric = /^\d+$/.test(leftId);
    const rightNumeric = /^\d+$/.test(rightId);
    if (leftNumeric && rightNumeric) return compareNumeric(leftId, rightId);
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1;
    return leftId < rightId ? -1 : 1;
  }
  return 0;
}

function splitSemver(version: string): [string, string | undefined] {
  const withoutBuild = version.split('+', 1)[0];
  const prereleaseIndex = withoutBuild.indexOf('-');
  if (prereleaseIndex === -1) return [withoutBuild, undefined];
  return [withoutBuild.slice(0, prereleaseIndex), withoutBuild.slice(prereleaseIndex + 1)];
}

function compareNumeric(left: string, right: string) {
  const normalizedLeft = left.replace(/^0+(?=\d)/, '');
  const normalizedRight = right.replace(/^0+(?=\d)/, '');
  if (normalizedLeft.length !== normalizedRight.length) {
    return normalizedLeft.length < normalizedRight.length ? -1 : 1;
  }
  return normalizedLeft === normalizedRight ? 0 : normalizedLeft < normalizedRight ? -1 : 1;
}
