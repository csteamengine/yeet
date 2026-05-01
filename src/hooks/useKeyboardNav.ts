import { useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '@/stores/clipboardStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useHotkeyModeStore } from '@/stores/hotkeyModeStore';

export function useKeyboardNav() {
  const {
    items,
    selectNext,
    selectPrevious,
    pasteSelected,
    deleteSelected,
    setSelectedIndex,
  } = useClipboardStore();

  const { isSettingsOpen, closeSettings, openSettings, settings } = useSettingsStore();
  const { exitHotkeyMode, cycleNext } = useHotkeyModeStore();

  const handleKeyDown = useCallback(
    async (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isInput =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable;

      if (e.key === 'Escape') {
        if (isSettingsOpen) {
          closeSettings();
          return;
        }
        if (useHotkeyModeStore.getState().isHotkeyMode) {
          exitHotkeyMode();
          invoke('exit_hotkey_mode');
        }
        await invoke('hide_window');
        return;
      }

      if (e.metaKey && e.key === ',') {
        e.preventDefault();
        openSettings();
        return;
      }

      if (e.code === 'KeyV' && useHotkeyModeStore.getState().isHotkeyMode) {
        e.preventDefault();
        cycleNext();
        return;
      }

      if (isSettingsOpen) return;

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        selectNext();
        syncSelectionToBackend();
        return;
      }

      if (e.key === 'ArrowUp') {
        e.preventDefault();
        selectPrevious();
        syncSelectionToBackend();
        return;
      }

      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        await pasteSelected();
        return;
      }

      if (isInput) return;

      if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        await deleteSelected();
        return;
      }

      if (/^[1-9]$/.test(e.key) && settings.sticky_mode && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
        e.preventDefault();
        const index = parseInt(e.key) - 1;
        if (index < items.length) setSelectedIndex(index);
        return;
      }
    },
    [
      items,
      selectNext,
      selectPrevious,
      pasteSelected,
      deleteSelected,
      setSelectedIndex,
      isSettingsOpen,
      closeSettings,
      openSettings,
      settings,
      exitHotkeyMode,
      cycleNext,
    ]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);
}

function syncSelectionToBackend() {
  const state = useClipboardStore.getState();
  const item = state.items[state.selectedIndex];
  if (item) {
    invoke('set_selected_item', { id: item.id });
  }
}
