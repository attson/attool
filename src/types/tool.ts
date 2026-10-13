export type ToolStatus = 'ready' | 'soon';

export type ToolGroup = 'download' | 'edit' | 'network' | 'utility';

export type ToolIconId =
  | 'download'
  | 'layout'
  | 'clipboard'
  | 'type'
  | 'wifi'
  | 'hash'
  | 'code'
  | 'video'
  | 'image'
  | 'record'
  | 'dice'
  | 'clock'
  | 'send'
  | 'chat';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
  group: ToolGroup;
}
