export type ConvertFormat = 'json' | 'yaml' | 'toml' | 'csv';

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface JsonParseError {
  message: string;
  line?: number;
  column?: number;
}

export interface JsonParseResult {
  ok: boolean;
  value?: JsonValue;
  error?: JsonParseError;
}
