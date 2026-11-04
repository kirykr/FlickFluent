import type { CheckItem } from "./types";

/** Items shown in the blocked table: the missing ones, in the order the core reported them. */
export function rowsToShow(missing: CheckItem[]): CheckItem[] {
  return missing.filter((i) => i.status === "belowMinimum");
}

/** Message key for a hardware item, so labels stay in the text catalogue. */
export function itemLabelKey(item: CheckItem): string {
  return `compat.item.${item.key}`;
}
