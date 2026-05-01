import { create } from 'zustand';
import type { Mission } from '../lib/tauri';

interface MissionState {
  missions: Mission[];
  selectedMissionId: string | null;
  setMissions: (missions: Mission[]) => void;
  prependMission: (mission: Mission) => void;
  upsertMission: (mission: Mission) => void;
  selectMission: (missionId: string | null) => void;
}

export const useMissionStore = create<MissionState>((set) => ({
  missions: [],
  selectedMissionId: null,
  setMissions: (missions) =>
    set((state) => ({
      missions,
      selectedMissionId:
        state.selectedMissionId && missions.some((mission) => mission.id === state.selectedMissionId)
          ? state.selectedMissionId
          : missions[0]?.id ?? null,
    })),
  prependMission: (mission) =>
    set((state) => ({
      missions: [mission, ...state.missions],
      selectedMissionId: mission.id,
    })),
  upsertMission: (mission) =>
    set((state) => ({
      missions: [mission, ...state.missions.filter((item) => item.id !== mission.id)],
      selectedMissionId: mission.id,
    })),
  selectMission: (selectedMissionId) => set({ selectedMissionId }),
}));
