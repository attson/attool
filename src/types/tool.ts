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
  | 'dice'
  | 'clock'
  | 'send';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
  group: ToolGroup;
}
