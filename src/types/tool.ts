export type ToolStatus = 'ready' | 'soon';

export type ToolGroup = 'platform' | 'download' | 'edit' | 'network' | 'utility';

export type ToolIconId =
  | 'download'
  | 'layout'
  | 'clipboard'
  | 'wifi'
  | 'code'
  | 'video'
  | 'image'
  | 'dice'
  | 'clock'
  | 'send'
  | 'chat'
  | 'apps';

export type ToolSource = 'platform' | 'builtin' | 'plugin';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
  group: ToolGroup;
  source?: ToolSource;
  pluginAppId?: string;
}
