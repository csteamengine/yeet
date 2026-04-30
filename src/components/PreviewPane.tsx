import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '@/stores/clipboardStore';

function isDataImageUrl(s: string): boolean {
  return s.trimStart().startsWith('data:image/');
}

export function PreviewPane() {
  const { items, selectedIndex } = useClipboardStore();
  const item = items[selectedIndex];
  const [imageSrc, setImageSrc] = useState<string | null>(null);
  const [imageError, setImageError] = useState<string | null>(null);

  useEffect(() => {
    setImageSrc(null);
    setImageError(null);
    if (!item) return;

    if (item.content_type === 'image') {
      invoke<string | null>('get_image_base64', { id: item.id })
        .then((src) => {
          if (src) {
            setImageSrc(src);
          } else {
            setImageError('Image file not found');
          }
        })
        .catch((err) => {
          console.error('[preview] get_image_base64 failed:', err);
          setImageError(String(err));
        });
    } else if (isDataImageUrl(item.content)) {
      setImageSrc(item.content.trim());
    }
  }, [item?.id, item?.content_type]);

  if (!item) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--text-tertiary)] text-[13px]">
        No item selected
      </div>
    );
  }

  if (item.content_type === 'image' || isDataImageUrl(item.content)) {
    return (
      <div className="flex-1 flex items-center justify-center p-4 overflow-hidden">
        {imageSrc ? (
          <img
            src={imageSrc}
            alt="Clipboard image"
            className="max-w-full max-h-full object-contain rounded"
          />
        ) : imageError ? (
          <span className="text-[var(--text-tertiary)] text-[13px]">{imageError}</span>
        ) : (
          <span className="text-[var(--text-tertiary)] text-[13px]">Loading image...</span>
        )}
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-3">
      <pre className="text-[12px] text-[var(--text-primary)] whitespace-pre-wrap break-words font-mono leading-relaxed">
        {item.content}
      </pre>
    </div>
  );
}
