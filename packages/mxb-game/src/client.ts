import { GAME_EVENT_TYPES, SCHEMA_VERSION } from "./types.js";
import type { GameEvent, GameEventType, Hello, LapRecord, Snapshot } from "./types.js";

const gameEventTypes = new Set<string>(GAME_EVENT_TYPES);

function gameEvent(value: unknown): value is GameEvent {
  return typeof value === "object" && value !== null &&
    gameEventTypes.has(String((value as { type?: unknown }).type));
}

export interface ClientOptions {
  /** From MXB App → Settings → Game SDK. */
  token: string;
  /** Default `http://127.0.0.1:47820`. */
  url?: string;
}

export interface StreamOptions {
  /** Frames a second, 1 to 60. Default 20. */
  rate?: number;
  /** What to receive. Default both. */
  state?: boolean;
  events?: boolean;
  /** Reconnect after the app restarts or the game closes. Default true. */
  reconnect?: boolean;
}

export interface Stream {
  /** Called with every frame. */
  onState(fn: (s: Snapshot) => void): () => void;
  /** Called with every event, or only those of `type`. */
  onEvent<T extends GameEventType>(type: T, fn: (e: Extract<GameEvent, { type: T }>) => void): () => void;
  onEvent(fn: (e: GameEvent) => void): () => void;
  /** Connected and disconnected. */
  onStatus(fn: (connected: boolean, error?: GameSdkError) => void): () => void;
  close(): void;
}

export class GameSdkError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

/**
 * A client for MXB App's Game SDK endpoint. Works in browsers (an OBS browser source, a web
 * dashboard) and in Node 22+, which have `fetch` and `WebSocket` built in.
 *
 * ```ts
 * const mxb = new GameSdk({ token });
 * const stream = mxb.stream({ rate: 30 });
 * stream.onState((s) => console.log(s.local?.speedKmh));
 * stream.onEvent("lap", (e) => console.log(e.timeMs));
 * ```
 */
export class GameSdk {
  readonly url: string;
  private readonly token: string;

  constructor(opts: ClientOptions) {
    if (!opts?.token) throw new Error("a token is required (MXB App → Settings → Game SDK)");
    this.token = opts.token;
    this.url = (opts.url ?? "http://127.0.0.1:47820").replace(/\/+$/, "");
  }

  private async get<T>(path: string): Promise<T> {
    const r = await fetch(this.url + path, { headers: { Authorization: `Bearer ${this.token}` } });
    if (!r.ok) {
      const body = (await r.json().catch(() => ({}))) as { error?: string };
      throw new GameSdkError(body.error ?? `HTTP ${r.status}`, r.status);
    }
    return (await r.json()) as T;
  }

  private compatible(hello: Hello): Hello {
    if (hello.schema !== SCHEMA_VERSION) {
      throw new GameSdkError(
        `incompatible Game SDK schema ${hello.schema}; this client supports schema ${SCHEMA_VERSION}`,
        426,
      );
    }
    return hello;
  }

  /** Server versions, enabled capabilities, and game integration state. */
  async hello(): Promise<Hello> {
    return this.compatible(await this.get<Hello>("/v1/hello"));
  }

  /** Check that the server and this client use the same schema major. */
  connect(): Promise<Hello> {
    return this.hello();
  }

  /** The current state. */
  state(): Promise<Snapshot> {
    return this.get<Snapshot>("/v1/state");
  }

  /** Recent events after `since` (the last `seq` you saw), oldest first. */
  async events(opts: { since?: number } = {}): Promise<{ events: GameEvent[]; last: number }> {
    const page = await this.get<{ events: unknown[]; last: number }>(`/v1/events?since=${opts.since ?? 0}`);
    return { ...page, events: page.events.filter(gameEvent) };
  }

  /** The player's completed laps this session, oldest first. */
  async laps(): Promise<LapRecord[]> {
    return (await this.get<{ laps: LapRecord[] }>("/v1/laps")).laps;
  }

  /** Whether the app's endpoint is up. Needs no token. */
  async health(): Promise<boolean> {
    try {
      return (await fetch(this.url + "/v1/health")).ok;
    } catch {
      return false;
    }
  }

  /** Live frames and events over a WebSocket. */
  stream(opts: StreamOptions = {}): Stream {
    const topics = [opts.state !== false && "state", opts.events !== false && "events"].filter(Boolean).join(",");
    const rate = Math.min(60, Math.max(1, Math.round(opts.rate ?? 20)));
    const wsUrl =
      this.url.replace(/^http/, "ws") +
      `/v1/stream?token=${encodeURIComponent(this.token)}&rate=${rate}&topics=${topics}`;
    const stateFns = new Set<(s: Snapshot) => void>();
    const eventFns = new Set<{ type?: string; fn: (e: GameEvent) => void }>();
    const statusFns = new Set<(c: boolean, error?: GameSdkError) => void>();
    let ws: WebSocket | null = null;
    let closed = false;
    let retry = 500;

    const connect = () => {
      if (closed) return;
      ws = new WebSocket(wsUrl);
      ws.onopen = () => {
        retry = 500;
      };
      ws.onmessage = (m) => {
        let msg: { type: string; data: unknown };
        try {
          msg = JSON.parse(String(m.data));
        } catch {
          return;
        }
        if (msg.type === "hello") {
          try {
            this.compatible(msg as unknown as Hello);
            statusFns.forEach((fn) => fn(true));
          } catch (error) {
            closed = true;
            const sdkError = error as GameSdkError;
            statusFns.forEach((fn) => fn(false, sdkError));
            ws?.close();
          }
        } else if (msg.type === "state") stateFns.forEach((fn) => fn(msg.data as Snapshot));
        else if (msg.type === "event" && gameEvent(msg.data)) {
          const e = msg.data;
          eventFns.forEach((h) => (!h.type || h.type === e.type) && h.fn(e));
        }
      };
      ws.onclose = () => {
        statusFns.forEach((fn) => fn(false));
        if (!closed && opts.reconnect !== false) {
          setTimeout(connect, retry);
          retry = Math.min(retry * 2, 10_000);
        }
      };
      ws.onerror = () => ws?.close();
    };
    connect();

    const add = <T>(set: Set<T>, v: T) => {
      set.add(v);
      return () => void set.delete(v);
    };
    return {
      onState: (fn) => add(stateFns, fn),
      onEvent: ((a: unknown, b?: unknown) =>
        typeof a === "function"
          ? add(eventFns, { fn: a as (e: GameEvent) => void })
          : add(eventFns, { type: a as string, fn: b as (e: GameEvent) => void })) as Stream["onEvent"],
      onStatus: (fn) => add(statusFns, fn),
      close() {
        closed = true;
        ws?.close();
      },
    };
  }
}

/** `95123` → `"1:35.123"`. */
export function formatLap(ms: number | null | undefined): string {
  if (!ms || ms <= 0) return "-:--.---";
  const m = Math.floor(ms / 60_000);
  const s = Math.floor(ms / 1000) % 60;
  return `${m}:${String(s).padStart(2, "0")}.${String(ms % 1000).padStart(3, "0")}`;
}
