import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import clsx from 'clsx';
import { useSettingsStore, type ContentType } from '@/stores/settingsStore';
import { useClipboardStore } from '@/stores/clipboardStore';

const TABS = [
  { id: 'general', label: 'General' },
  { id: 'exclusions', label: 'Exclusions' },
  { id: 'appearance', label: 'Appearance' },
] as const;

type TabId = (typeof TABS)[number]['id'];

export function SettingsPanel() {
  const { isSettingsOpen, closeSettings } = useSettingsStore();
  const [activeTab, setActiveTab] = useState<TabId>('general');

  if (!isSettingsOpen) return null;

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="w-[640px] h-[520px] rounded-xl overflow-hidden bg-[#1c1c1e] shadow-xl flex flex-col border border-[rgba(255,255,255,0.1)]">
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--border-color)]">
          <h2 className="text-lg font-semibold text-[var(--text-primary)]">Settings</h2>
          <button
            onClick={closeSettings}
            className="p-1.5 rounded-lg hover:bg-[var(--bg-secondary)] transition-colors"
            aria-label="Close settings"
          >
            <svg className="w-5 h-5 text-[var(--text-secondary)]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div className="flex flex-1 overflow-hidden">
          <div className="w-40 border-r border-[var(--border-color)] py-2">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={clsx(
                  'w-full px-4 py-2 text-left text-sm transition-colors',
                  activeTab === tab.id
                    ? 'bg-accent-100 dark:bg-accent-900/30 text-accent-700 dark:text-accent-300 font-medium'
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
          </div>
        </div>
      </div>
    </div>
  );
}

function GeneralTab() {
  const { settings, setHotkey, updateSettings } = useSettingsStore();
  const { clearHistory } = useClipboardStore();
  const [hotkeyInput, setHotkeyInput] = useState(settings.hotkey);

  useEffect(() => setHotkeyInput(settings.hotkey), [settings.hotkey]);

  return (
    <div className="space-y-6">
      <Row label="Panel hotkey" description="Opens Yoink and lets you cycle with V">
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={hotkeyInput}
            onChange={(e) => setHotkeyInput(e.target.value)}
            className="px-3 py-1.5 rounded-lg w-48 bg-[var(--bg-secondary)] text-[var(--text-primary)] border border-[var(--border-color)] focus:outline-none focus:ring-2 focus:ring-accent-500"
            placeholder="Command+Shift+V"
          />
          <button
            onClick={() => setHotkey(hotkeyInput)}
            className="px-3 py-1.5 rounded-lg bg-accent-500 text-white text-sm hover:bg-accent-600"
          >
            Set
          </button>
        </div>
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

      <Row
        label="Intercept Cmd+V"
        description="Plain Cmd+V silently pastes the latest history item (overrides system clipboard)"
      >
        <Toggle
          checked={settings.intercept_paste}
          onChange={(checked) => updateSettings({ intercept_paste: checked })}
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
