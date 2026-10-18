export function normalizeProtectedGroups(value) {
  if (!Array.isArray(value)) return [];
  const unique = new Set();
  for (const group of value) {
    const name = String(group ?? "");
    if (name) unique.add(name);
  }
  return [...unique].sort((left, right) => left.localeCompare(right));
}

export function setProtectedGroup(groups, name, protect) {
  const target = String(name ?? "");
  if (!target) throw new Error("分组名称不能为空");
  const next = new Set(normalizeProtectedGroups(groups));
  if (protect) next.add(target);
  else next.delete(target);
  return [...next].sort((left, right) => left.localeCompare(right));
}

export function protectionSavePayload(groups, baseRevision) {
  const revision = Number(baseRevision);
  if (!Number.isSafeInteger(revision) || revision < 0) {
    throw new Error("工程版本无效，无法保存分组保护");
  }
  return {
    protectedGroups: normalizeProtectedGroups(groups),
    baseRevision: revision,
  };
}

export function isGroupProtected(groups, name) {
  return normalizeProtectedGroups(groups).includes(String(name ?? ""));
}

export function groupRows(statsGroups, protectedGroups) {
  const counts = new Map();
  for (const [name, count] of Object.entries(statsGroups || {})) {
    counts.set(String(name), Number(count) || 0);
  }
  for (const name of normalizeProtectedGroups(protectedGroups)) {
    if (!counts.has(name)) counts.set(name, 0);
  }
  return [...counts.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, count]) => ({
      name,
      count,
      empty: count === 0,
      protected: isGroupProtected(protectedGroups, name),
    }));
}
