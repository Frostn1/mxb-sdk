export function readFile(path: string, encoding: "utf8"): Promise<string>;
export function readdir(path: string, options: { withFileTypes: true }): Promise<Array<{ name: string; isFile(): boolean }>>;
export function stat(path: string): Promise<{ size: number }>;
export function writeFile(path: string, data: string, encoding?: "utf8"): Promise<void>;
