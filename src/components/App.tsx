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
import clsx from 'clsx';

export default function App() {
  useClipboardMonitor();
  useKeyboardNav();
  useGlobalHotkey();

  const { settings, loadSettings, applyTheme, openSettings } = useSettingsStore();
  const { isHotkeyMode, setupListeners: setupHotkeyModeListeners } = useHotkeyModeStore();
  const showSearch = settings.sticky_mode;

  useEffect(() => {
    loadSettings();

    // Tray menu calls window.__openSettings via webview eval; expose it.
    (window as unknown as { __openSettings?: () => void }).__openSettings = openSettings;

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
        {showSearch && <SearchBar />}
        <button
          onClick={openSettings}
          className="p-1.5 rounded-md hover:bg-[rgba(255,255,255,0.08)] text-[var(--text-secondary)]"
          aria-label="Open settings"
          title="Settings"
        >
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
              d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      </header>

      <div className="flex-1 flex min-h-0">
        <div className="w-1/2 flex flex-col min-h-0 border-r border-[var(--border-color)]">
          <ClipboardList />
        </div>
        <div className="w-1/2 flex flex-col min-h-0">
          <PreviewPane />
        </div>
      </div>
      <SettingsPanel />
    </div>
  );
}
