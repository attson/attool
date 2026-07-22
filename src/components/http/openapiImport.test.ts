import { describe, expect, it } from 'vitest';
import { parseOpenApiToCollection } from './openapiImport';

const spec = {
  openapi: '3.0.3',
  info: { title: 'Petstore Admin', version: '1.0.0' },
  servers: [{ url: 'https://api.example.com/v1' }],
  paths: {
    '/pets': {
      get: {
        summary: 'List pets',
        parameters: [
          { name: 'limit', in: 'query', schema: { type: 'integer' } },
          { name: 'x-trace-id', in: 'header', required: false }
        ],
        responses: { '200': { description: 'ok' } }
      },
      post: {
        operationId: 'createPet',
        requestBody: {
          content: {
            'application/json': {
              schema: {
                type: 'object',
                properties: {
                  name: { type: 'string' },
                  age: { type: 'integer' },
                  vaccinated: { type: 'boolean' }
                },
                required: ['name']
              }
            }
          }
        },
        responses: { '201': { description: 'created' } }
      }
    },
    '/stores/{storeId}/orders': {
      get: {
        summary: 'List store orders',
        parameters: [
          { name: 'storeId', in: 'path', required: true, schema: { type: 'string' } },
          { name: 'status', in: 'query', schema: { type: 'string', example: 'open' } }
        ]
      }
    }
  }
};

describe('parseOpenApiToCollection', () => {
  it('imports OAS 3 JSON into a collection tree using the first server URL', () => {
    const result = parseOpenApiToCollection(JSON.stringify(spec));

    expect(result.collection.name).toBe('Petstore Admin');
    expect(result.baseUrl).toBe('https://api.example.com/v1');
    expect(result.folders.map((f) => f.name)).toEqual(['pets', 'stores']);
    expect(result.requests.map((r) => r.name)).toEqual([
      'GET List pets',
      'POST createPet',
      'GET List store orders'
    ]);
    expect(result.requests[0].spec.url).toBe('https://api.example.com/v1/pets');
    expect(result.requests[2].spec.url).toBe('https://api.example.com/v1/stores/{{storeId}}/orders');
  });

  it('maps query/header/path parameters and JSON request body examples', () => {
    const result = parseOpenApiToCollection(JSON.stringify(spec), { baseUrl: '{{baseUrl}}' });
    const listPets = result.requests.find((r) => r.name === 'GET List pets')!;
    const createPet = result.requests.find((r) => r.name === 'POST createPet')!;
    const listOrders = result.requests.find((r) => r.name === 'GET List store orders')!;

    expect(listPets.spec.url).toBe('{{baseUrl}}/pets');
    expect(listPets.spec.queryParams).toEqual([
      { key: 'limit', value: '', enabled: true, description: '' }
    ]);
    expect(listPets.spec.headers).toEqual([
      { key: 'x-trace-id', value: '', enabled: true, description: '' }
    ]);
    expect(listOrders.spec.url).toContain('{{storeId}}');
    expect(listOrders.spec.queryParams[0]).toMatchObject({ key: 'status', value: 'open' });
    expect(createPet.spec.bodyType).toBe('json');
    expect(JSON.parse(createPet.spec.body)).toEqual({ name: 'string', age: 0, vaccinated: false });
  });

  it('rejects unsupported or empty documents', () => {
    expect(() => parseOpenApiToCollection('{}')).toThrow('只支持 OpenAPI 3.x JSON');
    expect(() => parseOpenApiToCollection(JSON.stringify({ openapi: '3.0.0', info: {}, paths: {} })))
      .toThrow('没有可导入的 HTTP operation');
  });
});
