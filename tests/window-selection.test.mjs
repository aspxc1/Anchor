import test from "node:test";
import assert from "node:assert/strict";
import { filterWindows, retainSelection } from "../src/windowSelection.ts";

const windows = [
  { hwnd: 100, title: "Research Plan", process_name: "notepad.exe", pid: 10 },
  { hwnd: 200, title: "Reference PDF", process_name: "msedge.exe", pid: 20 },
];

test("search matches the window title or executable without regard to case or surrounding whitespace", () => {
  assert.deepEqual(filterWindows(windows, "  RESEARCH "), [windows[0]]);
  assert.deepEqual(filterWindows(windows, "EDGE.EXE"), [windows[1]]);
  assert.deepEqual(filterWindows(windows, " "), windows);
  assert.deepEqual(filterWindows(windows, "missing"), []);
});

test("refresh removes closed windows and handles reused by a different process", () => {
  const selected = new Map([[100, 10], [200, 99], [300, 30]]);
  assert.deepEqual(retainSelection(selected, windows), new Map([[100, 10]]));
});
