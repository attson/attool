import type { Tool } from '../../types/tool';

const CODEC_APP_ID = 'com.attson.codec';

export function composeToolNavigation(
  builtinTools: Tool[],
  pluginTools: Tool[],
  pluginCodecPreference: boolean | null
): Tool[] {
  const usePluginCodec =
    pluginCodecPreference ?? pluginTools.some((tool) => tool.pluginAppId === CODEC_APP_ID);
  const builtins = usePluginCodec
    ? builtinTools.filter((tool) => tool.id !== 'codec')
    : builtinTools;
  const plugins = usePluginCodec
    ? pluginTools
    : pluginTools.filter((tool) => tool.pluginAppId !== CODEC_APP_ID);
  return [...builtins, ...plugins];
}
