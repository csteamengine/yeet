import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { useSettingsStore } from './settingsStore';

interface GitHubUser {
  login: string;
  avatar_url: string;
  name: string | null;
}

interface DeviceFlowResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
}

type AuthStatus = 'idle' | 'awaiting_code' | 'polling' | 'authenticated' | 'error';

interface AuthState {
  status: AuthStatus;
  user: GitHubUser | null;
  userCode: string | null;
  verificationUri: string | null;
  error: string | null;

  checkAuth: () => Promise<void>;
  startDeviceFlow: () => Promise<void>;
  cancelFlow: () => void;
  logout: () => Promise<void>;
}

export const useAuthStore = create<AuthState>((set, get) => {
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  const stopPolling = () => {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    invoke('github_cancel_polling').catch(() => {});
  };

  return {
    status: 'idle',
    user: null,
    userCode: null,
    verificationUri: null,
    error: null,

    checkAuth: async () => {
      try {
        const user = await invoke<GitHubUser | null>('github_get_user');
        if (user) {
          set({ status: 'authenticated', user, error: null });
        } else {
          set({ status: 'idle', user: null });
        }
      } catch (e) {
        console.error('[auth] checkAuth failed:', e);
        set({ status: 'idle', user: null });
      }
    },

    startDeviceFlow: async () => {
      set({ status: 'awaiting_code', error: null });
      // Force the panel to stay open during auth
      useSettingsStore.getState().openSettings();
      try {
        const flow = await invoke<DeviceFlowResponse>('github_start_device_flow');

        set({
          status: 'polling',
          userCode: flow.user_code,
          verificationUri: flow.verification_uri,
        });

        const interval = Math.max(flow.interval, 5) * 1000;

        pollTimer = setInterval(async () => {
          try {
            const result = await invoke<{ status: string; token?: string; message?: string }>(
              'github_poll_token',
              { deviceCode: flow.device_code }
            );
            if (result.status === 'success') {
              stopPolling();
              await get().checkAuth();
            } else if (result.status === 'expired') {
              stopPolling();
              set({ status: 'error', error: 'Code expired. Try again.', userCode: null });
            } else if (result.status === 'error') {
              stopPolling();
              set({ status: 'error', error: result.message || 'Unknown error', userCode: null });
            }
          } catch (e) {
            stopPolling();
            set({ status: 'error', error: String(e), userCode: null });
          }
        }, interval);
      } catch (e) {
        set({ status: 'error', error: String(e) });
      }
    },

    cancelFlow: () => {
      stopPolling();
      set({ status: 'idle', userCode: null, verificationUri: null, error: null });
    },

    logout: async () => {
      stopPolling();
      await invoke('github_logout');
      set({ status: 'idle', user: null, userCode: null, verificationUri: null, error: null });
    },
  };
});
