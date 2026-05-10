export type ToolStatus = 'ready' | 'soon';

export type ToolIconId =
  | 'download'
  | 'layout'
  | 'clipboard'
  | 'type'
  | 'wifi'
  | 'hash';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
  icon: ToolIconId;
}
