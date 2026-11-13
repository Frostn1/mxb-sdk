import { createHash, createPrivateKey, createPublicKey, generateKeyPairSync, sign, verify as verifySignature } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { deflateRawSync, inflateRawSync } from "node:zlib";

export const ALLOWED_EXTENSIONS = new Set([
  "json", "html", "htm", "js", "mjs", "css", "png", "jpg", "jpeg", "webp", "gif", "svg",
  "woff", "woff2", "ttf", "otf", "txt", "md",
]);
export const MAX_PACKAGE_BYTES = 25 * 1024 * 1024;

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".") || entry.name === "node_modules") continue;
    const path = join(dir, entry.name);
    if (entry.isDirectory()) walk(path, out);
    else if (entry.isFile()) out.push(path);
  }
  return out;
}

function sortedNames(files) {
  return [...files.keys()].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
}

function digestInput(files) {
  let input = "mxbext-v1\n";
  for (const name of sortedNames(files)) {
    if (name === "signature.json") continue;
    input += `${name}\n${createHash("sha256").update(files.get(name)).digest("hex")}\n`;
  }
  return Buffer.from(input, "utf8");
}

const crcTable = new Uint32Array(256).map((_, n) => {
  let value = n;
  for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  return value >>> 0;
});

function crc32(data) {
  let value = 0xffffffff;
  for (const byte of data) value = crcTable[(value ^ byte) & 0xff] ^ (value >>> 8);
  return (value ^ 0xffffffff) >>> 0;
}

function makeZip(files) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  const names = sortedNames(files);
  for (const rel of names) {
    const data = files.get(rel);
    const name = Buffer.from(rel, "utf8");
    const compressed = deflateRawSync(data);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0x0800, 6);
    local.writeUInt16LE(8, 8);
    local.writeUInt32LE(0, 10);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(compressed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(name.length, 26);
    locals.push(local, name, compressed);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0x0800, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt32LE(0, 12);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(compressed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, name);
    offset += local.length + name.length + compressed.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(names.length, 8);
  end.writeUInt16LE(names.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

function readZip(path) {
  const zip = readFileSync(path);
  const files = new Map();
  let offset = 0;
  while (offset + 4 <= zip.length && zip.readUInt32LE(offset) === 0x04034b50) {
    const method = zip.readUInt16LE(offset + 8);
    const crc = zip.readUInt32LE(offset + 14);
    const compressedSize = zip.readUInt32LE(offset + 18);
    const size = zip.readUInt32LE(offset + 22);
    const nameSize = zip.readUInt16LE(offset + 26);
    const extraSize = zip.readUInt16LE(offset + 28);
    const name = zip.subarray(offset + 30, offset + 30 + nameSize).toString("utf8");
    if (!name || name.startsWith("/") || name.includes("\\") || name.split("/").includes("..")) throw new Error(`unsafe ZIP path '${name}'`);
    const start = offset + 30 + nameSize + extraSize;
    const compressed = zip.subarray(start, start + compressedSize);
    const data = method === 8 ? inflateRawSync(compressed) : method === 0 ? compressed : undefined;
    if (!data || data.length !== size || crc32(data) !== crc) throw new Error(`invalid ZIP entry '${name}'`);
    if (files.has(name)) throw new Error(`duplicate ZIP entry '${name}'`);
    files.set(name, data);
    offset = start + compressedSize;
  }
  if (!files.size) throw new Error("package is not a readable ZIP");
  return files;
}

function readManifest(files) {
  const bytes = files.get("extension.json");
  if (!bytes) throw new Error("package has no extension.json");
  const manifest = JSON.parse(Buffer.from(bytes).toString("utf8"));
  if (!manifest || typeof manifest !== "object" || typeof manifest.id !== "string" ||
    typeof manifest.version !== "string" || manifest.manifestVersion !== 1) {
    throw new Error("extension.json is not a version 1 extension manifest");
  }
  return manifest;
}

export function keygen(path) {
  if (existsSync(path)) throw new Error(`${path} exists; refusing to overwrite a key`);
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  writeFileSync(path, privateKey.export({ type: "pkcs8", format: "pem" }), { mode: 0o600 });
  const raw = publicKey.export({ type: "spki", format: "der" }).subarray(-32);
  return { path, publicKey: raw.toString("base64") };
}

export function pack(folder, options = {}) {
  if (!existsSync(folder) || !statSync(folder).isDirectory()) throw new Error(`${folder} is not a folder`);
  const files = new Map();
  for (const path of walk(folder)) {
    const rel = relative(folder, path).split(sep).join("/");
    const extension = rel.split(".").pop()?.toLowerCase() ?? "";
    if (rel === "signature.json") continue;
    if (!ALLOWED_EXTENSIONS.has(extension)) {
      options.onWarning?.(`skipped ${rel} (extensions cannot carry .${extension} files)`);
      continue;
    }
    files.set(rel, readFileSync(path));
  }
  const manifest = readManifest(files);
  let publicKey;
  if (options.key) {
    const key = createPrivateKey(readFileSync(options.key));
    const raw = createPublicKey(key).export({ type: "spki", format: "der" }).subarray(-32);
    publicKey = raw.toString("base64");
    const signature = sign(null, digestInput(files), key);
    files.set("signature.json", Buffer.from(JSON.stringify({ publicKey, signature: signature.toString("base64") }, null, 2)));
  } else {
    options.onWarning?.("unsigned: this installs only with developer mode on");
  }
  const zip = makeZip(files);
  if (zip.length > MAX_PACKAGE_BYTES) throw new Error("the package is over 25 MB");
  const out = options.out ?? `${manifest.id}-${manifest.version}.mxbext`;
  writeFileSync(out, zip);
  return { path: out, files: files.size, bytes: zip.length, signed: Boolean(options.key), publicKey };
}

export function verify(path) {
  const files = readZip(path);
  const manifest = readManifest(files);
  const rawSignature = files.get("signature.json");
  if (!rawSignature) return { valid: true, signed: false, manifest, files: sortedNames(files) };
  const record = JSON.parse(Buffer.from(rawSignature).toString("utf8"));
  if (typeof record.publicKey !== "string" || typeof record.signature !== "string") throw new Error("signature.json is not readable");
  const rawKey = Buffer.from(record.publicKey, "base64");
  const signature = Buffer.from(record.signature, "base64");
  if (rawKey.length !== 32 || signature.length !== 64) throw new Error("signature.json has invalid Ed25519 data");
  const prefix = Buffer.from("302a300506032b6570032100", "hex");
  const key = createPublicKey({ key: Buffer.concat([prefix, rawKey]), type: "spki", format: "der" });
  const valid = verifySignature(null, digestInput(files), key, signature);
  return { valid, signed: true, manifest, files: sortedNames(files), publicKey: record.publicKey };
}
