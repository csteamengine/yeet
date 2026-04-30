import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';

export interface ClipboardItem {
  id: string;
  content_type: string; // "text" | "url" | "code" | "file" | "image"
  content: string;
  preview: string;
  hash: string;
  created_at: string;
}

interface ClipboardState {
  items: ClipboardItem[];
  selectedIndex: number;
  search: string;
  isLoading: boolean;
  error: string | null;

  loadItems: () => Promise<void>;
  setSearch: (search: string) => void;
  setSelectedIndex: (index: number) => void;
  selectNext: () => void;
  selectPrevious: () => void;
  pasteSelected: () => Promise<void>;
  pasteItem: (id: string) => Promise<void>;
  deleteItem: (id: string) => Promise<void>;
  deleteSelected: () => Promise<void>;
  clearHistory: () => Promise<void>;
  setupListeners: () => Promise<() => void>;
}

export const useClipboardStore = create<ClipboardState>((set, get) => ({
  items: [],
  selectedIndex: 0,
  search: '',
  isLoading: false,
  error: null,

  loadItems: async () => {
    try {
      const { search } = get();
      const items = await invoke<ClipboardItem[]>('get_clipboard_items', {
        limit: 100,
        offset: 0,
        search: search || null,
      });
      set({ items, isLoading: false, error: null });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  setSearch: (search: string) => {
    set({ search, selectedIndex: 0 });
    get().loadItems();
  },

  setSelectedIndex: (index: number) => {
    const { items } = get();
    if (index >= 0 && index < items.length) {
      set({ selectedIndex: index });
    }
  },

  selectNext: () => {
    const { items, selectedIndex } = get();
    if (items.length === 0) return;
    set({ selectedIndex: (selectedIndex + 1) % items.length });
  },

  selectPrevious: () => {
    const { items, selectedIndex } = get();
    if (items.length === 0) return;
    set({ selectedIndex: selectedIndex === 0 ? items.length - 1 : selectedIndex - 1 });
  },

  pasteSelected: async () => {
    const { items, selectedIndex } = get();
    if (items[selectedIndex]) {
      await get().pasteItem(items[selectedIndex].id);
    }
  },

  pasteItem: async (id: string) => {
    try {
      await invoke('paste_and_simulate', { id });
    } catch (error) {
      set({ error: String(error) });
    }
  },

  deleteItem: async (id: string) => {
    try {
      await invoke('delete_clipboard_item', { id });
      await get().loadItems();
    } catch (error) {
      set({ error: String(error) });
    }
  },

  deleteSelected: async () => {
    const { items, selectedIndex } = get();
    if (items[selectedIndex]) {
      await get().deleteItem(items[selectedIndex].id);
    }
  },

  clearHistory: async () => {
    try {
      await invoke('clear_history');
      await get().loadItems();
    } catch (error) {
      set({ error: String(error) });
    }
  },

  setupListeners: async () => {
    const webview = getCurrentWebviewWindow();
    const unlistenChanged = await webview.listen<ClipboardItem>('clipboard-changed', (event) => {
      console.log('[clipboard] received clipboard-changed event', event.payload?.content_type);
      get().loadItems();
    });
    const unlistenShown = await webview.listen('panel-shown', () => {
      get().loadItems();
    });
    return () => {
      unlistenChanged();
      unlistenShown();
    };
  },
}));
