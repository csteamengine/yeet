import { useRef, useEffect } from 'react';
import { useClipboardStore } from '@/stores/clipboardStore';
import { ClipboardItem } from './ClipboardItem';

export function ClipboardList() {
  const listRef = useRef<HTMLDivElement>(null);
  const { items, selectedIndex, setSelectedIndex, pasteItem, isLoading } =
    useClipboardStore();

  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined;
    el?.scrollIntoView({ block: 'nearest' });
  }, [selectedIndex]);

  if (isLoading && items.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--text-tertiary)] text-[13px]">
        Loading...
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-[var(--text-tertiary)] p-8">
        <p className="text-[13px]">No clipboard items yet</p>
        <p className="text-[11px] mt-1">Copy something to get started</p>
      </div>
    );
  }

  return (
    <div ref={listRef} className="flex-1 overflow-auto py-1">
      {items.map((item, index) => (
        <ClipboardItem
          key={item.id}
          item={item}
          index={index}
          isSelected={selectedIndex === index}
          onSelect={() => setSelectedIndex(index)}
          onPaste={() => pasteItem(item.id)}
        />
      ))}
    </div>
  );
}
