/**
 * 运行时状态管理
 */

import { create } from 'zustand';
import type { AppRuntimeStatus, EngineStatus, ForegroundSnapshot } from '../lib/tauri';

interface RuntimeState {
  // Agent Engine 状态
  engine: EngineStatus;
  setEngineStatus: (status: EngineStatus) => void;

  // 应用运行时状态
  appRuntime: AppRuntimeStatus;
  setAppRuntimeStatus: (status: AppRuntimeStatus) => void;

  // 前台运行快照
  foreground: ForegroundSnapshot;
  setForegroundStatus: (status: ForegroundSnapshot) => void;

  // 活跃任务数
  activeMissionCount: number;
  setActiveMissionCount: (count: number) => void;

  // 待审批数
  pendingApprovalCount: number;
  setPendingApprovalCount: (count: number) => void;

  // 加载状态
  loading: boolean;
  setLoading: (loading: boolean) => void;

  // 操作进行中
  actionInProgress: string | null;
  setActionInProgress: (action: string | null) => void;
}

export const useRuntimeStore = create<RuntimeState>((set) => ({
  engine: {
    running: false,
    profile: null,
    pid: null,
    last_error: null,
  },
  setEngineStatus: (engine) => set({ engine }),

  appRuntime: {
    installed: false,
    running: false,
    version: null,
  },
  setAppRuntimeStatus: (appRuntime) => set({ appRuntime }),

  foreground: {
    active: false,
    state: 'idle',
    session_id: null,
    run_id: null,
    cancel_state: null,
    pending_count: 0,
    interrupt_count: 0,
    updated_at: '',
  },
  setForegroundStatus: (foreground) => set({ foreground }),

  activeMissionCount: 0,
  setActiveMissionCount: (activeMissionCount) => set({ activeMissionCount }),

  pendingApprovalCount: 0,
  setPendingApprovalCount: (pendingApprovalCount) => set({ pendingApprovalCount }),

  loading: false,
  setLoading: (loading) => set({ loading }),

  actionInProgress: null,
  setActionInProgress: (actionInProgress) => set({ actionInProgress }),
}));
