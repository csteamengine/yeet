import { useEffect } from 'react';
import { useClipboardStore } from '@/stores/clipboardStore';

export function useClipboardMonitor() {
  useEffect(() => {
    useClipboardStore.getState().loadItems();

    let cleanup: (() => void) | undefined;
    useClipboardStore.getState().setupListeners().then((unsub) => {
      cleanup = unsub;
    });

    let stopped = false;
    async function poll() {
      while (!stopped) {
        await useClipboardStore.getState().loadItems();
        await new Promise((r) => setTimeout(r, 300));
      }
    }
    poll();

    return () => {
      stopped = true;
      cleanup?.();
    };
  }, []);
}
