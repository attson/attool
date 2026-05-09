export type TemplateLayerType = 'text' | 'image' | 'shape' | 'group';
export type ImageFit = 'cover' | 'contain' | 'stretch';
export type ShapeKind = 'rect' | 'roundRect' | 'ellipse' | 'line';
export type TextAlign = 'left' | 'center' | 'right';
export type TextFontStyle = 'normal' | 'italic';
export type TextDecoration = 'none' | 'underline' | 'line-through';

export type TemplateProject = {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  layers: TemplateLayer[];
  assets: TemplateAsset[];
  sourcePsdPath?: string;
  previewPath?: string;
  createdAt: string;
  updatedAt: string;
};

export type TemplateLayer = {
  id: string;
  name: string;
  type: TemplateLayerType;
  x: number;
  y: number;
  width: number;
  height: number;
  visible: boolean;
  opacity: number;
  rotation: number;
  bindingKey?: string;
  locked?: boolean;
  children?: TemplateLayer[];
  text?: TextLayerData;
  image?: ImageLayerData;
  shape?: ShapeLayerData;
};

export type TextLayerData = {
  text: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: number | string;
  color: string;
  strokeColor?: string;
  strokeWidth?: number;
  letterSpacing?: number;
  lineHeight?: number;
  align?: TextAlign;
  fontStyle?: TextFontStyle;
  textDecoration?: TextDecoration;
  backgroundColor?: string;
  backgroundRadius?: number;
  shadowColor?: string;
  shadowBlur?: number;
  shadowOffsetX?: number;
  shadowOffsetY?: number;
};

export type ImageLayerData = {
  assetId: string;
  fit: ImageFit;
  replaceable: boolean;
};

export type ShapeLayerData = {
  shape: ShapeKind;
  fill?: string;
  stroke?: string;
  strokeWidth?: number;
  radius?: number;
};

export type TemplateAsset = {
  id: string;
  name: string;
  dataUrl: string;
  sourceLayerId?: string;
  mimeType: string;
  width: number;
  height: number;
  createdAt: string;
};

export type TemplateSummary = {
  id: string;
  name: string;
  canvasWidth: number;
  canvasHeight: number;
  previewPath?: string;
  updatedAt: string;
};

export type BatchVariantInput =
  | { kind: 'image'; sourcePath: string }
  | { kind: 'text'; value: string };

export type BatchTaskInput = {
  layerId: string;
  variants: BatchVariantInput[];
};

export type BatchOutputItem = {
  id: string;
  filePath: string;
  fileName: string;
};
