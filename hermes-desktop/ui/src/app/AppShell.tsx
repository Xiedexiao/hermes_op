import { Outlet } from 'react-router-dom';
import { ContextPanel } from '../components/ContextPanel';
import { SidebarNav } from '../components/SidebarNav';
import { TopBar } from '../components/TopBar';
import './AppShell.css';

export function AppShell() {
  return (
    <div className="app-shell">
      <TopBar />
      <div className="app-shell-body">
        <SidebarNav />
        <main className="main-content">
          <Outlet />
        </main>
        <ContextPanel />
      </div>
    </div>
  );
}
