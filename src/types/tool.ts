export type ToolStatus = 'ready' | 'soon';

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
  | 'dice';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
}
