export function execFile(
  file: string,
  args: string[],
  options: { windowsHide: boolean },
  callback: (error: Error | null, stdout: string, stderr: string) => void,
): void;
