export type ToolStatus = 'ready' | 'soon';

export interface Tool {
  id: string;
  name: string;
  description: string;
  status: ToolStatus;
}
