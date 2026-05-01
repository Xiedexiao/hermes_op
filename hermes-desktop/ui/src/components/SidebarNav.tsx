import { NavLink } from 'react-router-dom';
import './SidebarNav.css';

const navItems = [
  { path: '/home', label: 'Home' },
  { path: '/missions', label: 'Missions' },
  { path: '/notifications', label: 'Notifications' },
  { path: '/operate', label: 'Operate' },
  { path: '/knowledge', label: 'Knowledge' },
  { path: '/simulation', label: 'Simulation' },
  { path: '/skills', label: 'Skills' },
  { path: '/agent-exchange', label: 'Agent Exchange' },
  { path: '/voice', label: 'Voice' },
  { path: '/sessions', label: 'Sessions' },
  { path: '/runtime', label: 'Runtime' },
  { path: '/settings', label: 'Settings' },
];

export function SidebarNav() {
  return (
    <nav className="sidebar-nav">
      <ul>
        {navItems.map((item) => (
          <li key={item.path}>
            <NavLink
              to={item.path}
              className={({ isActive }) =>
                `nav-item ${isActive ? 'nav-item-active' : ''}`
              }
            >
              {item.label}
            </NavLink>
          </li>
        ))}
      </ul>
    </nav>
  );
}
