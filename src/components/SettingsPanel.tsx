import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import clsx from 'clsx';
import { useSettingsStore, type ContentType } from '@/stores/settingsStore';
import { useClipboardStore } from '@/stores/clipboardStore';
import { useAuthStore } from '@/stores/authStore';

const TABS = [
  { id: 'general', label: 'General' },
  { id: 'exclusions', label: 'Exclusions' },
  { id: 'appearance', label: 'Appearance' },
  { id: 'account', label: 'Account' },
] as const;

type TabId = (typeof TABS)[number]['id'];

export function SettingsPanel() {
  const [activeTab, setActiveTab] = useState<TabId>('general');

  return (
    <div className="flex w-full h-full">
        <div className="w-36 border-r border-[var(--border-color)] py-2">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={clsx(
                'w-full px-4 py-2 text-left text-sm transition-colors',
                activeTab === tab.id
                  ? 'bg-[rgba(99,102,241,0.15)] text-accent-400 font-medium'
                  : 'text-[var(--text-secondary)] hover:bg-[var(--bg-secondary)]'
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>

        <div className="flex-1 p-6 overflow-auto">
          {activeTab === 'general' && <GeneralTab />}
          {activeTab === 'exclusions' && <ExclusionsTab />}
          {activeTab === 'appearance' && <AppearanceTab />}
          {activeTab === 'account' && <AccountTab />}
        </div>
    </div>
  );
}

function keyEventToShortcut(e: KeyboardEvent): { display: string; shortcut: string } | null {
  const isMac = navigator.platform.toUpperCase().includes('MAC');

  const modifiers: string[] = [];
  if (e.metaKey) modifiers.push(isMac ? 'Command' : 'Super');
  if (e.ctrlKey) modifiers.push(isMac ? 'Control' : 'Ctrl');
  if (e.altKey) modifiers.push(isMac ? 'Option' : 'Alt');
  if (e.shiftKey) modifiers.push('Shift');

  if (modifiers.length === 0) return null;

  const modifierKeys = new Set(['Meta', 'Control', 'Alt', 'Shift']);
  if (modifierKeys.has(e.key)) return null;

  const keyMap: Record<string, string> = {
    ' ': 'Space', ArrowUp: 'Up', ArrowDown: 'Down',
    ArrowLeft: 'Left', ArrowRight: 'Right',
    Backspace: 'Backspace', Delete: 'Delete', Enter: 'Enter',
    Tab: 'Tab', Escape: 'Escape', Home: 'Home', End: 'End',
    PageUp: 'PageUp', PageDown: 'PageDown',
  };

  let key = keyMap[e.key] || e.key.toUpperCase();
  if (key.length === 1 && /[A-Z0-9]/.test(key)) { /* valid */ }
  else if (key.startsWith('F') && /^F\d{1,2}$/.test(key)) { /* function key */ }
  else if (keyMap[e.key]) { /* mapped key */ }
  else return null;

  const parts = [...modifiers, key];
  return { display: parts.join('+'), shortcut: parts.join('+') };
}

function HotkeyRecorder({ value, onRecord }: { value: string; onRecord: (shortcut: string) => void }) {
  const [recording, setRecording] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const ref = useRef<HTMLButtonElement>(null);

  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const result = keyEventToShortcut(e);
    if (result) {
      setPending(result.shortcut);
      setRecording(false);
    }
  }, []);

  const handleBlur = useCallback(() => {
    setRecording(false);
  }, []);

  useEffect(() => {
    if (!recording) return;
    const el = ref.current;
    if (!el) return;
    el.addEventListener('keydown', handleKeyDown);
    el.addEventListener('blur', handleBlur);
    return () => {
      el.removeEventListener('keydown', handleKeyDown);
      el.removeEventListener('blur', handleBlur);
    };
  }, [recording, handleKeyDown, handleBlur]);

  useEffect(() => {
    if (recording && ref.current) ref.current.focus();
  }, [recording]);

  const displayValue = pending ?? value;

  return (
    <div className="flex items-center gap-2">
      <div className="flex items-stretch rounded-lg border border-[var(--border-color)] overflow-hidden">
        {recording ? (
          <button
            ref={ref}
            className="px-3 py-1.5 w-44 text-left text-sm bg-accent-500/10 text-accent-400 animate-pulse"
          >
            Press a shortcut…
          </button>
        ) : (
          <span className="px-3 py-1.5 w-44 text-sm bg-[var(--bg-secondary)] text-[var(--text-primary)] flex items-center">
            {displayValue || 'No hotkey set'}
          </span>
        )}
        <button
          onClick={() => {
            if (recording) {
              setRecording(false);
            } else {
              setPending(null);
              setRecording(true);
            }
          }}
          className={clsx(
            'px-3 py-1.5 text-sm font-medium border-l border-[var(--border-color)] transition-colors',
            recording
              ? 'bg-red-500/10 text-red-400 hover:bg-red-500/20'
              : 'bg-[var(--bg-tertiary)] text-[var(--text-secondary)] hover:bg-accent-500/10 hover:text-accent-400'
          )}
        >
          {recording ? 'Cancel' : 'Record'}
        </button>
      </div>
      {pending && !recording && (
        <>
          <button
            onClick={() => { onRecord(pending); setPending(null); }}
            className="px-3 py-1.5 rounded-lg bg-accent-500 text-white text-sm hover:bg-accent-600"
          >
            Save
          </button>
          <button
            onClick={() => setPending(null)}
            className="px-2 py-1.5 text-sm text-[var(--text-tertiary)] hover:text-[var(--text-secondary)]"
          >
            Discard
          </button>
        </>
      )}
    </div>
  );
}

function GeneralTab() {
  const { settings, setHotkey, updateSettings } = useSettingsStore();
  const { clearHistory } = useClipboardStore();

  return (
    <div className="space-y-6">
      <Row label="Panel hotkey" description="Opens Yeet and lets you cycle with V">
        <HotkeyRecorder value={settings.hotkey} onRecord={setHotkey} />
      </Row>

      <Row
        label="Sticky mode"
        description="Panel stays open until Escape or Enter. When off, releasing modifier keys dismisses."
      >
        <Toggle
          checked={settings.sticky_mode}
          onChange={(checked) => updateSettings({ sticky_mode: checked })}
        />
      </Row>

      <Row label="History limit" description="Maximum items kept in history">
        <select
          value={settings.history_limit}
          onChange={(e) => updateSettings({ history_limit: parseInt(e.target.value) })}
          className="px-3 py-1.5 rounded-lg bg-[var(--bg-secondary)] text-[var(--text-primary)] border border-[var(--border-color)]"
        >
          <option value={50}>50</option>
          <option value={100}>100</option>
          <option value={200}>200</option>
          <option value={500}>500</option>
        </select>
      </Row>

      <Row label="Show timestamps" description="Show how long ago each item was copied">
        <Toggle
          checked={settings.show_timestamps}
          onChange={(checked) => updateSettings({ show_timestamps: checked })}
        />
      </Row>

      <Row label="Clear history" description="Delete all clipboard items">
        <button
          onClick={clearHistory}
          className="px-3 py-1.5 rounded-lg bg-red-500 text-white text-sm hover:bg-red-600"
        >
          Clear
        </button>
      </Row>
    </div>
  );
}

function ExclusionsTab() {
  const { settings, updateSettings, addExcludedApp, removeExcludedApp, toggleExcludedType } = useSettingsStore();
  const [newApp, setNewApp] = useState('');
  const [currentApp, setCurrentApp] = useState<string | null>(null);

  useEffect(() => {
    invoke<string | null>('get_current_app').then(setCurrentApp).catch(() => {});
  }, []);

  const types: { id: ContentType; label: string }[] = [
    { id: 'text', label: 'Plain text' },
    { id: 'url', label: 'URLs' },
    { id: 'code', label: 'Code' },
    { id: 'file', label: 'Files / paths' },
    { id: 'image', label: 'Images' },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-1">Excluded apps</h3>
        <p className="text-xs text-[var(--text-secondary)] mb-3">
          When the listed bundle ID (or any substring of it) is the frontmost app, clipboard writes from it are ignored. Useful for dictation tools.
        </p>

        <div className="flex items-center gap-2 mb-3">
          <input
            type="text"
            value={newApp}
            onChange={(e) => setNewApp(e.target.value)}
            placeholder="com.example.app"
            className="flex-1 px-3 py-1.5 rounded-lg bg-[var(--bg-secondary)] text-[var(--text-primary)] border border-[var(--border-color)]"
          />
          <button
            onClick={async () => {
              if (newApp.trim()) {
                await addExcludedApp(newApp.trim());
                setNewApp('');
              }
            }}
            className="px-3 py-1.5 rounded-lg bg-accent-500 text-white text-sm hover:bg-accent-600"
          >
            Add
          </button>
          {currentApp && (
            <button
              onClick={() => addExcludedApp(currentApp)}
              className="px-3 py-1.5 rounded-lg bg-[var(--bg-secondary)] text-[var(--text-primary)] text-sm border border-[var(--border-color)] hover:bg-[var(--bg-tertiary)]"
              title={`Add ${currentApp}`}
            >
              + Frontmost
            </button>
          )}
        </div>

        <ul className="space-y-1">
          {settings.excluded_apps.length === 0 && (
            <li className="text-xs text-[var(--text-tertiary)]">No apps excluded.</li>
          )}
          {settings.excluded_apps.map((id) => (
            <li
              key={id}
              className="flex items-center justify-between px-3 py-1.5 rounded bg-[var(--bg-secondary)]"
            >
              <code className="text-xs text-[var(--text-primary)]">{id}</code>
              <button
                onClick={() => removeExcludedApp(id)}
                className="text-xs text-red-500 hover:text-red-600"
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-1">Excluded types</h3>
        <p className="text-xs text-[var(--text-secondary)] mb-3">
          Content types to skip when capturing.
        </p>
        <div className="grid grid-cols-2 gap-2">
          {types.map((t) => {
            const checked = settings.excluded_types.includes(t.id);
            return (
              <label
                key={t.id}
                className="flex items-center gap-2 px-3 py-2 rounded bg-[var(--bg-secondary)] cursor-pointer"
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={() => toggleExcludedType(t.id)}
                />
                <span className="text-sm text-[var(--text-primary)]">{t.label}</span>
              </label>
            );
          })}
        </div>
      </div>

      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-1">Pasteboard markers</h3>
        <p className="text-xs text-[var(--text-secondary)] mb-3">
          Skip clipboard entries tagged with these NSPasteboard markers.
        </p>
        <div className="space-y-2">
          <label className="flex items-center gap-2 px-3 py-2 rounded bg-[var(--bg-secondary)] cursor-pointer">
            <input
              type="checkbox"
              checked={settings.ignore_transient}
              onChange={(e) => updateSettings({ ignore_transient: e.target.checked })}
            />
            <div>
              <span className="text-sm text-[var(--text-primary)]">Transient</span>
              <p className="text-xs text-[var(--text-secondary)]">Short-lived content (e.g. drag operations)</p>
            </div>
          </label>
          <label className="flex items-center gap-2 px-3 py-2 rounded bg-[var(--bg-secondary)] cursor-pointer">
            <input
              type="checkbox"
              checked={settings.ignore_autogenerated}
              onChange={(e) => updateSettings({ ignore_autogenerated: e.target.checked })}
            />
            <div>
              <span className="text-sm text-[var(--text-primary)]">Auto-generated</span>
              <p className="text-xs text-[var(--text-secondary)]">Machine-generated, not user-copied</p>
            </div>
          </label>
          <label className="flex items-center gap-2 px-3 py-2 rounded bg-[var(--bg-secondary)] cursor-pointer">
            <input
              type="checkbox"
              checked={settings.ignore_concealed}
              onChange={(e) => updateSettings({ ignore_concealed: e.target.checked })}
            />
            <div>
              <span className="text-sm text-[var(--text-primary)]">Concealed</span>
              <p className="text-xs text-[var(--text-secondary)]">Sensitive content (e.g. passwords)</p>
            </div>
          </label>
        </div>
      </div>
    </div>
  );
}

function AppearanceTab() {
  const { settings, setTheme, updateSettings } = useSettingsStore();

  return (
    <div className="space-y-6">
      <Row label="Theme" description="Light, dark, or follow system">
        <select
          value={settings.theme}
          onChange={(e) => setTheme(e.target.value as 'light' | 'dark' | 'system')}
          className="px-3 py-1.5 rounded-lg bg-[var(--bg-secondary)] text-[var(--text-primary)] border border-[var(--border-color)]"
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </Row>

      <Row label="Font size">
        <select
          value={settings.font_size}
          onChange={(e) => updateSettings({ font_size: parseInt(e.target.value) })}
          className="px-3 py-1.5 rounded-lg bg-[var(--bg-secondary)] text-[var(--text-primary)] border border-[var(--border-color)]"
        >
          <option value={12}>12px</option>
          <option value={14}>14px</option>
          <option value={16}>16px</option>
          <option value={18}>18px</option>
        </select>
      </Row>
    </div>
  );
}

interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version: string | null;
  release_notes: string | null;
}

function AccountTab() {
  const { status, user, userCode, verificationUri, error, checkAuth, startDeviceFlow, cancelFlow, logout } = useAuthStore();
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateStatus, setUpdateStatus] = useState<'idle' | 'checking' | 'downloading'>('idle');

  useEffect(() => {
    checkAuth();
    handleCheckUpdates();
  }, []);

  const handleCheckUpdates = async () => {
    setUpdateStatus('checking');
    try {
      const info = await invoke<UpdateInfo>('check_for_updates');
      setUpdateInfo(info);
    } catch (e) {
      console.error('[update] check failed:', e);
    }
    setUpdateStatus('idle');
  };

  const handleDownload = async () => {
    if (!updateInfo?.available) return;
    setUpdateStatus('downloading');
    try {
      await invoke('download_and_install_update');
    } catch (e) {
      console.error('[update] install failed:', e);
    }
    setUpdateStatus('idle');
  };

  if (status === 'authenticated' && user) {
    return (
      <div className="space-y-6">
        <div className="flex items-center gap-4 p-4 rounded-lg bg-[var(--bg-secondary)]">
          <img
            src={user.avatar_url}
            alt={user.login}
            className="w-12 h-12 rounded-full"
          />
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-[var(--text-primary)]">
              {user.name || user.login}
            </p>
            <p className="text-xs text-[var(--text-secondary)]">@{user.login}</p>
          </div>
          <button
            onClick={logout}
            className="px-3 py-1.5 rounded-lg text-sm text-red-500 border border-red-500/30 hover:bg-red-500/10"
          >
            Sign out
          </button>
        </div>

        <UpdateSection
          updateInfo={updateInfo}
          updateStatus={updateStatus}
          onCheck={handleCheckUpdates}
          onDownload={handleDownload}
        />
      </div>
    );
  }

  if (status === 'polling' && userCode) {
    return (
      <div className="space-y-6">
        <div className="p-4 rounded-lg bg-[var(--bg-secondary)] text-center space-y-4">
          <p className="text-sm text-[var(--text-secondary)]">
            Enter this code on GitHub:
          </p>
          <p className="text-3xl font-mono font-bold tracking-widest text-[var(--text-primary)]">
            {userCode}
          </p>
          {verificationUri && (
            <button
              onClick={() => invoke('github_open_url', { url: verificationUri })}
              className="px-4 py-2 rounded-lg bg-accent-500 text-white text-sm hover:bg-accent-600"
            >
              Open GitHub
            </button>
          )}
          <p className="text-xs text-[var(--text-tertiary)]">
            Waiting for authorization...
          </p>
        </div>
        <button
          onClick={cancelFlow}
          className="w-full px-3 py-1.5 rounded-lg text-sm text-[var(--text-secondary)] border border-[var(--border-color)] hover:bg-[var(--bg-secondary)]"
        >
          Cancel
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-sm font-semibold text-[var(--text-primary)] mb-1">GitHub</h3>
        <p className="text-xs text-[var(--text-secondary)] mb-4">
          Sign in with GitHub to enable updates from private or org repos.
        </p>
      </div>

      {error && (
        <p className="text-xs text-red-500">{error}</p>
      )}

      <button
        onClick={() => startDeviceFlow()}
        disabled={status === 'awaiting_code'}
        className="w-full px-4 py-2.5 rounded-lg text-sm font-medium transition-colors bg-[#24292f] text-white hover:bg-[#32383f]"
      >
        {status === 'awaiting_code' ? 'Starting...' : 'Sign in with GitHub'}
      </button>

      <UpdateSection
        updateInfo={updateInfo}
        updateStatus={updateStatus}
        onCheck={handleCheckUpdates}
        onDownload={handleDownload}
      />
    </div>
  );
}

function UpdateSection({
  updateInfo,
  updateStatus,
  onCheck,
  onDownload,
}: {
  updateInfo: UpdateInfo | null;
  updateStatus: 'idle' | 'checking' | 'downloading';
  onCheck: () => void;
  onDownload: () => void;
}) {
  return (
    <div className="p-4 rounded-lg bg-[var(--bg-secondary)] space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-[var(--text-primary)]">Updates</h3>
        <button
          onClick={onCheck}
          disabled={updateStatus === 'checking'}
          className="text-xs text-accent-400 hover:text-accent-300 disabled:opacity-50"
        >
          {updateStatus === 'checking' ? 'Checking...' : 'Check now'}
        </button>
      </div>

      {updateInfo && (
        <div className="space-y-2">
          <p className="text-xs text-[var(--text-secondary)]">
            Current version: {updateInfo.current_version}
          </p>
          {updateInfo.available && updateInfo.latest_version ? (
            <>
              <p className="text-xs text-green-400">
                Update available: v{updateInfo.latest_version}
              </p>
              {updateInfo.release_notes && (
                <p className="text-xs text-[var(--text-tertiary)] line-clamp-3">
                  {updateInfo.release_notes}
                </p>
              )}
              <button
                onClick={onDownload}
                disabled={updateStatus === 'downloading'}
                className="px-3 py-1.5 rounded-lg bg-accent-500 text-white text-sm hover:bg-accent-600 disabled:opacity-50"
              >
                {updateStatus === 'downloading' ? 'Installing…' : 'Update & Restart'}
              </button>
            </>
          ) : (
            <p className="text-xs text-[var(--text-tertiary)]">You're up to date.</p>
          )}
        </div>
      )}
    </div>
  );
}

function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-[var(--text-primary)]">{label}</p>
        {description && (
          <p className="text-xs text-[var(--text-secondary)] mt-0.5">{description}</p>
        )}
      </div>
      <div className="flex-shrink-0">{children}</div>
    </div>
  );
}

function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!checked)}
      className={clsx(
        'relative inline-flex h-6 w-11 items-center rounded-full transition-colors',
        checked ? 'bg-accent-500' : 'bg-[var(--bg-tertiary)]'
      )}
    >
      <span
        className={clsx(
          'inline-block h-4 w-4 transform rounded-full bg-white transition-transform',
          checked ? 'translate-x-6' : 'translate-x-1'
        )}
      />
    </button>
  );
}
