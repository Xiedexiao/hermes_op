/**
 * 应用状态管理
 */

import { create } from 'zustand';
import type { ActiveSessionSelection, AppSettings, RuntimeSettings } from '../lib/tauri';

interface AppState {
  // 主题
  theme: 'system' | 'light' | 'dark';
  setTheme: (theme: 'system' | 'light' | 'dark') => void;

  // 语言
  language: string;
  setLanguage: (lang: string) => void;

  // 初始化状态
  initialized: boolean;
  setInitialized: (value: boolean) => void;

  // 错误
  error: string | null;
  setError: (error: string | null) => void;

  // 应用设置
  appSettings: AppSettings | null;
  setAppSettings: (settings: AppSettings) => void;

  // 运行时设置
  runtimeSettings: RuntimeSettings | null;
  setRuntimeSettings: (settings: RuntimeSettings) => void;

  // 当前恢复中的 session
  activeSession: ActiveSessionSelection | null;
  setActiveSession: (session: ActiveSessionSelection | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  theme: 'system',
  setTheme: (theme) => set({ theme }),

  language: 'zh-CN',
  setLanguage: (language) => set({ language }),

  initialized: false,
  setInitialized: (initialized) => set({ initialized }),

  error: null,
  setError: (error) => set({ error }),

  appSettings: null,
  setAppSettings: (appSettings) => set({ appSettings }),

  runtimeSettings: null,
  setRuntimeSettings: (runtimeSettings) => set({ runtimeSettings }),

  activeSession: null,
  setActiveSession: (activeSession) => set({ activeSession }),
}));
