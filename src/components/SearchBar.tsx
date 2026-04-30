import { useEffect, useRef } from 'react';
import { useClipboardStore } from '@/stores/clipboardStore';
import clsx from 'clsx';

export function SearchBar() {
  const inputRef = useRef<HTMLInputElement>(null);
  const { search, setSearch } = useClipboardStore();

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Focus on window show
  useEffect(() => {
    const handleFocus = () => {
      inputRef.current?.focus();
    };

    window.addEventListener('focus', handleFocus);
    return () => {
      window.removeEventListener('focus', handleFocus);
    };
  }, []);

  return (
    <div className="relative flex-1">
      <div className="relative">
        <svg
          className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[var(--text-tertiary)]"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          />
        </svg>
        <input
          ref={inputRef}
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search clipboard history..."
          className={clsx(
            'w-full pl-10 pr-10 py-1.5 rounded-md text-[13px]',
            'bg-[rgba(255,255,255,0.06)] text-[var(--text-primary)]',
            'border border-[rgba(255,255,255,0.08)]',
            'placeholder-[var(--text-tertiary)]',
            'focus:outline-none focus:border-[rgba(120,180,255,0.6)]',
            'transition-all duration-150'
          )}
        />
        {search && (
          <button
            onClick={() => setSearch('')}
            className={clsx(
              'absolute right-3 top-1/2 -translate-y-1/2',
              'w-5 h-5 rounded-full',
              'bg-[var(--text-tertiary)] text-[var(--bg-primary)]',
              'flex items-center justify-center',
              'hover:bg-[var(--text-secondary)] transition-colors'
            )}
          >
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        )}
      </div>
    </div>
  );
}
