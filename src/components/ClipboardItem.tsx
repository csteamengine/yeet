import clsx from 'clsx';
import type { ClipboardItem as ClipboardItemType } from '@/stores/clipboardStore';

interface ClipboardItemProps {
  item: ClipboardItemType;
  index: number;
  isSelected: boolean;
  showNumbers: boolean;
  onSelect: () => void;
  onPaste: () => void;
  onDelete: () => void;
}

export function ClipboardItem({ item, index, isSelected, showNumbers, onSelect, onPaste, onDelete }: ClipboardItemProps) {
  let preview: string;
  if (item.content_type === 'image') {
    const filename = item.content.split('/').pop() || 'image';
    preview = filename;
  } else {
    preview = item.preview.length > 120 ? item.preview.slice(0, 120) + '...' : item.preview;
  }

  return (
    <div
      className={clsx(
        'group flex items-center gap-2 px-3 py-1.5 mx-1.5 rounded-md cursor-pointer',
        'transition-colors duration-75',
        isSelected
          ? 'bg-[rgba(74,158,255,0.18)]'
          : 'hover:bg-[rgba(255,255,255,0.04)]'
      )}
      onClick={onSelect}
      onDoubleClick={onPaste}
    >
      <span className="flex-1 min-w-0 text-[13px] text-[var(--text-primary)] truncate leading-snug">
        {preview}
      </span>

      {item.content_type !== 'text' && (
        <span className="flex-shrink-0 text-[11px] text-[var(--text-tertiary)] capitalize">
          {item.content_type}
        </span>
      )}

      {showNumbers && index < 9 && (
        <span className="flex-shrink-0 w-4 h-4 rounded text-[10px] flex items-center justify-center text-[var(--text-tertiary)] font-mono">
          {index + 1}
        </span>
      )}

      <button
        onClick={(e) => { e.stopPropagation(); onDelete(); }}
        className="flex-shrink-0 w-4 h-4 flex items-center justify-center rounded text-[var(--text-tertiary)] hover:text-red-400 hover:bg-[rgba(255,255,255,0.08)] opacity-0 group-hover:opacity-100 transition-opacity"
        aria-label="Delete item"
      >
        <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
