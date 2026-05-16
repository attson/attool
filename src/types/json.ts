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

export type JsonParseResult =
  | { ok: true; value: JsonValue }
  | { ok: false; error: JsonParseError };
