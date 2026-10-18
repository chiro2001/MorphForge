import assert from "node:assert/strict";
import test from "node:test";
import {
  groupRows,
  isGroupProtected,
  normalizeProtectedGroups,
  protectionSavePayload,
  setProtectedGroup,
} from "./group-ui.js";

test("protected groups are normalized without losing names", () => {
  assert.deepEqual(
    normalizeProtectedGroups(["主楼", "", "屋顶", "主楼", null]),
    ["屋顶", "主楼"].sort((a, b) => a.localeCompare(b)),
  );
  assert.deepEqual(normalizeProtectedGroups(undefined), []);
});

test("protection toggling is independent and idempotent", () => {
  const protectedGroups = setProtectedGroup(["主楼"], "屋顶", true);
  assert.equal(isGroupProtected(protectedGroups, "主楼"), true);
  assert.equal(isGroupProtected(protectedGroups, "屋顶"), true);
  const unlocked = setProtectedGroup(protectedGroups, "主楼", false);
  assert.equal(isGroupProtected(unlocked, "主楼"), false);
  assert.equal(isGroupProtected(unlocked, "屋顶"), true);
  assert.deepEqual(setProtectedGroup(unlocked, "屋顶", true), ["屋顶"]);
});

test("protection save always carries the current baseRevision", () => {
  assert.deepEqual(protectionSavePayload(["主楼"], 17), {
    protectedGroups: ["主楼"],
    baseRevision: 17,
  });
  assert.throws(
    () => protectionSavePayload(["主楼"], undefined),
    /工程版本无效/,
  );
});

test("future protected empty groups remain visible and unlockable", () => {
  assert.deepEqual(
    groupRows({ 主楼: 8, emptyOld: 0 }, ["未来组", "emptyOld"]),
    [
      { name: "emptyOld", count: 0, empty: true, protected: true },
      { name: "主楼", count: 8, empty: false, protected: false },
      { name: "未来组", count: 0, empty: true, protected: true },
    ],
  );
});
