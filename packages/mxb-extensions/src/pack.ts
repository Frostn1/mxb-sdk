export interface PackOptions {
  key?: string;
  out?: string;
  onWarning?: (message: string) => void;
}
export interface PackResult {
  path: string;
  files: number;
  bytes: number;
  signed: boolean;
  publicKey?: string;
}
export interface KeygenResult { path: string; publicKey: string }
export interface VerifyResult {
  valid: boolean;
  signed: boolean;
  manifest: import("./manifest.ts").ExtManifest;
  files: string[];
  publicKey?: string;
}

// The implementation is plain ESM so the standalone CLI still works on Node 18 and 20.
// @ts-expect-error The typed facade below is the declaration for this dependency-free module.
import * as runtime from "./pack.mjs";

const implementation = runtime as unknown as {
  ALLOWED_EXTENSIONS: Set<string>;
  MAX_PACKAGE_BYTES: number;
  keygen(path: string): KeygenResult;
  pack(folder: string, options?: PackOptions): PackResult;
  verify(path: string): VerifyResult;
};

export const ALLOWED_EXTENSIONS = implementation.ALLOWED_EXTENSIONS;
export const MAX_PACKAGE_BYTES = implementation.MAX_PACKAGE_BYTES;
export const keygen = implementation.keygen;
export const pack = implementation.pack;
export const verify = implementation.verify;
