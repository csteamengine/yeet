import { useEffect } from 'react';
import { SearchBar } from './SearchBar';
import { ClipboardList } from './ClipboardList';
import { PreviewPane } from './PreviewPane';
import { SettingsPanel } from './SettingsPanel';
import { useClipboardMonitor } from '@/hooks/useClipboardMonitor';
import { useKeyboardNav } from '@/hooks/useKeyboardNav';
import { useGlobalHotkey } from '@/hooks/useGlobalHotkey';
import { useSettingsStore } from '@/stores/settingsStore';
import { useHotkeyModeStore } from '@/stores/hotkeyModeStore';
import { useClipboardStore } from '@/stores/clipboardStore';
import { invoke } from '@tauri-apps/api/core';
import { ChevronLeft, Settings } from 'lucide-react';
import clsx from 'clsx';

export default function App() {
  useClipboardMonitor();
  useKeyboardNav();
  useGlobalHotkey();

  const { settings, loadSettings, applyTheme, openSettings, closeSettings, isSettingsOpen } = useSettingsStore();
  const { isHotkeyMode, setupListeners: setupHotkeyModeListeners } = useHotkeyModeStore();
  const { items, selectedIndex } = useClipboardStore();
  const showSearch = settings.sticky_mode;

  useEffect(() => {
    loadSettings();

    // Expose functions for backend webview.eval() calls.
    (window as unknown as { __openSettings?: () => void }).__openSettings = openSettings;
    (window as unknown as { __cycleNext?: () => void }).__cycleNext = () => {
      const { selectNext } = useClipboardStore.getState();
      selectNext();
      const { items, selectedIndex } = useClipboardStore.getState();
      const item = items[selectedIndex];
      if (item) {
        invoke('set_selected_item', { id: item.id });
      }
    };
    (window as unknown as { __cyclePrev?: () => void }).__cyclePrev = () => {
      const { selectPrevious } = useClipboardStore.getState();
      selectPrevious();
      const { items, selectedIndex } = useClipboardStore.getState();
      const item = items[selectedIndex];
      if (item) {
        invoke('set_selected_item', { id: item.id });
      }
    };

    let cleanupHotkeyMode: (() => void) | undefined;
    setupHotkeyModeListeners().then((unsub) => {
      cleanupHotkeyMode = unsub;
    });

    applyTheme();

    return () => {
      cleanupHotkeyMode?.();
    };
  }, [loadSettings, setupHotkeyModeListeners, applyTheme, openSettings]);

  return (
    <div
      className={clsx(
        'h-full flex flex-col',
        'glass rounded-[14px] overflow-hidden',
        'border border-[rgba(255,255,255,0.08)]',
        isHotkeyMode && 'ring-2 ring-[var(--accent-color)] ring-inset'
      )}
    >
      <header className="flex items-center gap-2 px-2.5 py-1.5 border-b border-[var(--border-color)] flex-shrink-0">
        {isSettingsOpen ? (
          <button
            onClick={closeSettings}
            className="flex items-center gap-1 p-1.5 rounded-md hover:bg-[rgba(255,255,255,0.08)] text-[var(--text-secondary)]"
            aria-label="Back"
          >
            <ChevronLeft className="w-4 h-4" />
            <span className="text-sm font-medium text-[var(--text-primary)]">Settings</span>
          </button>
        ) : (
          <>
            {showSearch && <SearchBar />}
            <span className="text-xs tabular-nums text-[var(--text-secondary)]">
              {items.length > 0 ? `${selectedIndex + 1}/${items.length}` : '0/0'}
            </span>
            <button
              onClick={openSettings}
              className="ml-auto p-1.5 rounded-md hover:bg-[rgba(255,255,255,0.08)] text-[var(--text-secondary)]"
              aria-label="Open settings"
              title="Settings"
            >
              <Settings className="w-5 h-5" />
            </button>
          </>
        )}
      </header>

      <div className="flex-1 flex min-h-0">
        {isSettingsOpen ? (
          <SettingsPanel />
        ) : (
          <>
            <div className="w-1/2 flex flex-col min-h-0 border-r border-[var(--border-color)]">
              <ClipboardList />
            </div>
            <div className="w-1/2 flex flex-col min-h-0">
              <PreviewPane />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
