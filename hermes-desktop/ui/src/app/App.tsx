import { Routes, Route, Navigate } from 'react-router-dom';
import { AppShell } from './AppShell';
import { AgentExchangePage } from '../routes/AgentExchangePage';
import { HomePage } from '../routes/HomePage';
import { KnowledgePage } from '../routes/KnowledgePage';
import { MissionsPage } from '../routes/MissionsPage';
import { NotificationsPage } from '../routes/NotificationsPage';
import { OperatePage } from '../routes/OperatePage';
import { RuntimePage } from '../routes/RuntimePage';
import { SimulationPage } from '../routes/SimulationPage';
import { SkillsPage } from '../routes/SkillsPage';
import { SessionsPage } from '../routes/SessionsPage';
import { SettingsPage } from '../routes/SettingsPage';
import { VoicePage } from '../routes/VoicePage';

function App() {
  return (
    <Routes>
      <Route path="/" element={<AppShell />}>
        <Route index element={<Navigate to="/home" replace />} />
        <Route path="home" element={<HomePage />} />
        <Route path="missions" element={<MissionsPage />} />
        <Route path="notifications" element={<NotificationsPage />} />
        <Route path="operate" element={<OperatePage />} />
        <Route path="knowledge" element={<KnowledgePage />} />
        <Route path="simulation" element={<SimulationPage />} />
        <Route path="skills" element={<SkillsPage />} />
        <Route path="agent-exchange" element={<AgentExchangePage />} />
        <Route path="voice" element={<VoicePage />} />
        <Route path="sessions" element={<SessionsPage />} />
        <Route path="runtime" element={<RuntimePage />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}

export default App;
