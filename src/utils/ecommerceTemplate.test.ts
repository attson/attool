import { describe, expect, it } from 'vitest';
import type { TemplateLayer } from '../types/ecommerceTemplate';
import {
  collectBindingKeys,
  extractBindingKey,
  flattenLayers,
  makeExportFileName,
  textLayerPreviewStyle,
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

  it('generates safe PNG filenames', () => {
    expect(makeExportFileName({ title: '双人黑色/北欧风' }, 0)).toBe('双人黑色_北欧风.png');
    expect(makeExportFileName({ name: 'sku 88' }, 4)).toBe('sku_88.png');
    expect(makeExportFileName({}, 8)).toBe('009.png');
  });
});
