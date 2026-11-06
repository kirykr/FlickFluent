import { createContext, useContext, useState, type ReactNode } from "react";
import { useEditor } from "./useEditor";

interface Value {
  handle: string;
  ed: ReturnType<typeof useEditor>;
  readOnly: boolean;
  /** Ids of the selected lines, shared by the table and the timeline. */
  selected: Set<string>;
  setSelected: (ids: Set<string>) => void;
}

const Context = createContext<Value | null>(null);

/** One editing session for the table and the timeline, with the selection they share. */
export function EditorProvider({
  handle,
  readOnly,
  children,
}: {
  handle: string;
  readOnly: boolean;
  children: ReactNode;
}) {
  const ed = useEditor(handle);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  return (
    <Context.Provider value={{ handle, ed, readOnly, selected, setSelected }}>
      {children}
    </Context.Provider>
  );
}

/** `null` outside a project: the editor then shows its empty state. */
export function useEditorContext(): Value | null {
  return useContext(Context);
}
