import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useClipboardStore } from './clipboardStore';

interface HotkeyModeState {
  isHotkeyMode: boolean;

  // Actions
  enterHotkeyMode: () => void;
  exitHotkeyMode: () => void;
  cycleNext: () => void;
  setupListeners: () => Promise<() => void>;
}

// Dedup timestamp shared by all cycle sources (global shortcut, backend polling, frontend keydown)
let lastCycleTime = 0;

export const useHotkeyModeStore = create<HotkeyModeState>((set, get) => ({
  isHotkeyMode: false,

  enterHotkeyMode: () => {
    set({ isHotkeyMode: true });
    // Sync current selected item to backend for modifier-release paste
    const { items, selectedIndex } = useClipboardStore.getState();
    const selectedItem = items[selectedIndex];
    if (selectedItem) {
      invoke('set_selected_item', { id: selectedItem.id });
    }
  },

  exitHotkeyMode: () => {
    set({ isHotkeyMode: false });
  },

  // Unified cycle handler with dedup - called from hotkey-cycle events AND frontend keydown
  cycleNext: () => {
    if (!get().isHotkeyMode) {
      // If a backend hotkey-cycle event arrives before we entered hotkey mode,
      // sync state and continue instead of dropping the cycle.
      set({ isHotkeyMode: true });
    }
    const now = Date.now();
    if (now - lastCycleTime < 100) return;
    lastCycleTime = now;
    const { selectNext } = useClipboardStore.getState();
    selectNext();
    // Sync new selection to backend
    const { items, selectedIndex } = useClipboardStore.getState();
    const newItem = items[selectedIndex];
    if (newItem) {
      invoke('set_selected_item', { id: newItem.id });
    }
  },

  setupListeners: async () => {
    const webview = getCurrentWebviewWindow();

    const unlistenHotkeyMode = await webview.listen('hotkey-mode-started', () => {
      get().enterHotkeyMode();
    });

    const unlistenPanelHidden = await webview.listen('panel-hidden', () => {
      get().exitHotkeyMode();
    });

    const unlistenCycle = await webview.listen('hotkey-cycle', () => {
      get().cycleNext();
    });

    // If the frontend loaded after hotkey mode started, sync once on setup.
    try {
      const active = await invoke<boolean>('is_hotkey_mode_active');
      if (active) {
        get().enterHotkeyMode();
      }
    } catch {
      // Ignore sync failures; normal events will still handle mode changes.
    }

    return () => {
      unlistenHotkeyMode();
      unlistenPanelHidden();
      unlistenCycle();
    };
  },
}));
