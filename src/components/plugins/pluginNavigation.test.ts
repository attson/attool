import { describe, expect, it } from 'vitest';

import type { Tool } from '../../types/tool';
import { composeToolNavigation } from './pluginNavigation';

const builtinCodec: Tool = {
  id: 'codec',
  name: '编码转换',
  description: 'Built in',
  status: 'ready',
  icon: 'hash',
  group: 'network'
};
const pluginCodec: Tool = {
  id: 'plugin:com.attson.codec',
  name: 'AT Codec',
  description: 'Plugin',
  status: 'ready',
  icon: 'apps',
  group: 'utility',
  source: 'plugin',
  pluginAppId: 'com.attson.codec'
};

describe('plugin migration navigation', () => {
  it('shows exactly one Codec entry in either migration mode', () => {
    expect(composeToolNavigation([builtinCodec], [pluginCodec], false)).toEqual([builtinCodec]);
    expect(composeToolNavigation([builtinCodec], [pluginCodec], true)).toEqual([pluginCodec]);
  });
});
