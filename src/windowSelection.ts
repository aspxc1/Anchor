export interface OpenWindow {
  hwnd: number;
  title: string;
  process_name: string;
  pid: number;
}

export function filterWindows(windows: OpenWindow[], search: string): OpenWindow[] {
  const query = search.trim().toLowerCase();
  return windows.filter(
    ({ title, process_name }) =>
      title.toLowerCase().includes(query) || process_name.toLowerCase().includes(query),
  );
}

export function retainSelection(
  selected: ReadonlyMap<number, number>,
  windows: OpenWindow[],
): Map<number, number> {
  return new Map(
    windows
      .filter(({ hwnd, pid }) => selected.get(hwnd) === pid)
      .map(({ hwnd, pid }) => [hwnd, pid]),
  );
}
