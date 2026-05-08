import { describe, expect, it } from 'vitest';
import type { TemplateAsset, TemplateLayer, TemplateProject } from '../types/ecommerceTemplate';
import {
  collectBindingKeys,
  createImageLayer,
  createShapeLayer,
  createTemplateAsset,
  createTextLayer,
  deleteLayerById,
  duplicateLayer,
  extractBindingKey,
  flattenLayers,
  insertLayer,
  moveLayer,
  removeSelectedLayer,
  makeExportFileName,
  textLayerPreviewStyle,
  updateLayerById,
  validateBatchFields
} from './ecommerceTemplate';

const layers: TemplateLayer[] = [
  {
    id: 'group-1',
    name: '右下角标',
    type: 'group',
    x: 10,
    y: 20,
    width: 300,
    height: 80,
    visible: true,
    opacity: 1,
    rotation: 0,
    children: [
      {
        id: 'text-1',
        name: '{{title}} 大标题',
        type: 'text',
        x: 12,
        y: 24,
        width: 250,
        height: 40,
        visible: true,
        opacity: 1,
        rotation: 0,
        bindingKey: 'title',
        text: {
          text: '便携小沙发',
          fontFamily: 'STHupo',
          fontSize: 48,
          fontWeight: 700,
          color: '#ffffff'
        }
      }
    ]
  },
  {
    id: 'image-1',
    name: '{{product_image}} 商品图',
    type: 'image',
    x: 0,
    y: 0,
    width: 1000,
    height: 1000,
    visible: true,
    opacity: 1,
    rotation: 0,
    bindingKey: 'product_image',
    image: { assetId: 'asset-1', fit: 'cover', replaceable: true }
  }
];

function makeProject(): TemplateProject {
  return {
    id: 'tpl-test',
    name: '测试模板',
    canvasWidth: 1000,
    canvasHeight: 1000,
    layers: [...layers],
    assets: [],
    createdAt: '2026-05-08 00:00:00',
    updatedAt: '2026-05-08 00:00:00'
  };
}

describe('ecommerceTemplate helpers', () => {
  it('extracts binding keys from PSD-style layer names', () => {
    expect(extractBindingKey('{{title}} 大标题')).toBe('title');
    expect(extractBindingKey('prefix {{selling_point_1}} 卖点')).toBe('selling_point_1');
    expect(extractBindingKey('商品图')).toBeUndefined();
  });

  it('flattens nested layers in paint order', () => {
    expect(flattenLayers(layers).map((layer) => layer.id)).toEqual(['group-1', 'text-1', 'image-1']);
  });

  it('collects unique binding keys from nested layers', () => {
    expect(collectBindingKeys(layers)).toEqual(['title', 'product_image']);
  });

  it('reports missing and unused batch fields', () => {
    expect(validateBatchFields(['title', 'product_image'], ['title', 'price'])).toEqual({
      missingFields: ['product_image'],
      unusedFields: ['price']
    });
  });

  it('scales text styles to the rendered canvas size', () => {
    const textLayer: TemplateLayer = {
      ...layers[0].children![0],
      text: {
        ...layers[0].children![0].text!,
        fontSize: 80,
        lineHeight: 96,
        letterSpacing: -4,
        strokeColor: '#000000',
        strokeWidth: 2
      }
    };

    expect(textLayerPreviewStyle(textLayer, 0.63)).toMatchObject({
      color: '#ffffff',
      fontFamily: 'STHupo',
      fontSize: '50.4px',
      lineHeight: '60.48px',
      letterSpacing: '-2.52px',
      WebkitTextStroke: '1.26px #000000'
    });
  });

  it('normalizes legacy PSD tracking values before scaling text styles', () => {
    const textLayer: TemplateLayer = {
      ...layers[0].children![0],
      text: {
        ...layers[0].children![0].text!,
        fontSize: 20,
        letterSpacing: -100
      }
    };

    expect(textLayerPreviewStyle(textLayer, 0.5)).toMatchObject({
      letterSpacing: '-1px'
    });
  });

  it('creates default text, shape, image assets, and image layers', () => {
    const textLayer = createTextLayer({ canvasWidth: 1000, canvasHeight: 1000, preset: 'title' });
    expect(textLayer).toMatchObject({
      type: 'text',
      visible: true,
      opacity: 1,
      rotation: 0,
      width: 420,
      height: 96,
      text: {
        text: '双击编辑标题',
        fontSize: 64,
        fontWeight: 800,
        fontStyle: 'normal',
        textDecoration: 'none',
        backgroundRadius: 0,
        shadowBlur: 0,
        shadowOffsetX: 0,
        shadowOffsetY: 0
      }
    });
    expect(textLayer.x).toBe(290);
    expect(textLayer.y).toBe(452);

    const shapeLayer = createShapeLayer({ canvasWidth: 1000, canvasHeight: 1000, shape: 'roundRect' });
    expect(shapeLayer).toMatchObject({
      type: 'shape',
      x: 350,
      y: 420,
      width: 300,
      height: 160,
      shape: { shape: 'roundRect', fill: '#f5d36b', stroke: '#17211b', strokeWidth: 0, radius: 24 }
    });

    const asset: TemplateAsset = createTemplateAsset({ path: '/tmp/chair.png', name: 'chair.png', width: 640, height: 480 });
    expect(asset).toMatchObject({ name: 'chair.png', path: '/tmp/chair.png', mimeType: 'image/png', width: 640, height: 480 });
    expect(asset.id).toMatch(/^asset-/);

    const imageLayer = createImageLayer({ canvasWidth: 1000, canvasHeight: 1000, asset });
    expect(imageLayer).toMatchObject({
      type: 'image',
      x: 250,
      y: 250,
      width: 500,
      height: 500,
      image: { assetId: asset.id, fit: 'contain', replaceable: true }
    });
  });

  it('inserts layers immutably at the top of paint order', () => {
    const project = makeProject();
    const layer = createTextLayer({ canvasWidth: project.canvasWidth, canvasHeight: project.canvasHeight, preset: 'body' });
    const next = insertLayer(project, layer);

    expect(next).not.toBe(project);
    expect(next.layers.at(-1)).toEqual(layer);
    expect(project.layers).toHaveLength(2);
    expect(next.layers).toHaveLength(3);
  });

  it('removes a selected layer and returns the next selection', () => {
    const project = makeProject();
    const result = removeSelectedLayer(project, 'image-1');

    expect(result.project.layers.map((layer) => layer.id)).toEqual(['group-1']);
    expect(result.selectedLayerId).toBeNull();
  });

  it('updates, deletes, duplicates, and reorders layers immutably', () => {
    const project = makeProject();
    const updated = updateLayerById(project.layers, 'text-1', (layer) => ({ ...layer, name: '新标题' }));
    expect(flattenLayers(updated).find((layer) => layer.id === 'text-1')?.name).toBe('新标题');
    expect(flattenLayers(project.layers).find((layer) => layer.id === 'text-1')?.name).toBe('{{title}} 大标题');

    const duplicated = duplicateLayer(project.layers, 'image-1');
    expect(duplicated).toHaveLength(3);
    expect(duplicated[2]).toMatchObject({ name: '{{product_image}} 商品图 副本', type: 'image' });
    expect(duplicated[2].id).not.toBe('image-1');

    const moved = moveLayer(duplicated, duplicated[2].id, 'backward');
    expect(moved.map((layer) => layer.id)).toEqual(['group-1', duplicated[2].id, 'image-1']);

    const deleted = deleteLayerById(moved, duplicated[2].id);
    expect(deleted.map((layer) => layer.id)).toEqual(['group-1', 'image-1']);
  });

  it('maps complete text styles to preview CSS', () => {
    const textLayer: TemplateLayer = {
      ...layers[0].children![0],
      text: {
        ...layers[0].children![0].text!,
        fontStyle: 'italic',
        textDecoration: 'underline',
        backgroundColor: '#fff1b8',
        backgroundRadius: 16,
        shadowColor: '#000000',
        shadowBlur: 8,
        shadowOffsetX: 3,
        shadowOffsetY: 4
      }
    };

    expect(textLayerPreviewStyle(textLayer, 0.5)).toMatchObject({
      fontStyle: 'italic',
      textDecoration: 'underline',
      backgroundColor: '#fff1b8',
      borderRadius: '8px',
      textShadow: '1.5px 2px 4px #000000'
    });
  });

  it('generates safe PNG filenames', () => {
    expect(makeExportFileName({ title: '双人黑色/北欧风' }, 0)).toBe('双人黑色_北欧风.png');
    expect(makeExportFileName({ name: 'sku 88' }, 4)).toBe('sku_88.png');
    expect(makeExportFileName({}, 8)).toBe('009.png');
  });
});
