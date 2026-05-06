import type { TemplateProject } from '../../types/ecommerceTemplate';

export function createEmptyTemplateProject(): TemplateProject {
  const timestamp = new Date().toLocaleString();
  return {
    id: `tpl-${crypto.randomUUID()}`,
    name: '未命名主图模板',
    canvasWidth: 1000,
    canvasHeight: 1000,
    layers: [],
    assets: [],
    createdAt: timestamp,
    updatedAt: timestamp
  };
}
