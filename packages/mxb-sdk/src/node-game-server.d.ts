type Buffer = any;
declare const Buffer: any;

declare module "node:crypto" {
  export function createHash(name: string): any;
  export function randomBytes(size: number): any;
}

declare module "node:fs" {
  export function readFileSync(path: string, encoding: "utf8"): string;
}

declare module "node:stream" {
  export interface Duplex {
    writable: boolean;
    destroyed: boolean;
    write(data: unknown): void;
    end(data?: unknown): void;
    destroy(): void;
    on(event: string, listener: (...args: any[]) => void): this;
  }
}

declare module "node:http" {
  import type { Duplex } from "node:stream";
  export interface IncomingMessage extends AsyncIterable<unknown> {
    url?: string;
    method?: string;
    headers: Record<string, string | string[] | undefined>;
  }
  export interface ServerResponse {
    writeHead(status: number, headers?: Record<string, unknown>): void;
    end(data?: unknown): void;
  }
  interface Server {
    on(event: "upgrade", listener: (request: IncomingMessage, socket: Duplex) => void): this;
    once(event: string, listener: (...args: any[]) => void): this;
    listen(port: number, host: string, listener: () => void): void;
    address(): { port: number } | null;
    close(listener: (error?: Error) => void): void;
  }
  export function createServer(listener: (request: IncomingMessage, response: ServerResponse) => void): Server;
}
