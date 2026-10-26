import type { JSX } from 'react';
import PermissionBanner from './components/PermissionBanner';
import Sidebar from './components/Sidebar';
import { ToastProvider } from './components/Toasts';
import { useHash } from './lib/hooks';
import Ai from './pages/Ai';
import History from './pages/History';
import Models from './pages/Models';
import Onboarding from './pages/Onboarding';
import Settings from './pages/Settings';
import Snippets from './pages/Snippets';
import Style from './pages/Style';

const PAGES: Record<string, () => JSX.Element> = {
  '/models': Models,
  '/settings': Settings,
  '/snippets': Snippets,
  '/style': Style,
  '/ai': Ai,
};

export default function App() {
  const route = useHash();
  return (
    <ToastProvider>
      {route === '/onboarding' ? (
        <>
          <div className="content-drag onb-drag" data-tauri-drag-region />
          <Onboarding />
        </>
      ) : (
        <div className="shell">
          <Sidebar route={route} />
          <main className="content">
            <div className="content-drag" data-tauri-drag-region />
            <div className="content-body">
              <PermissionBanner />
              {(() => { const Page = PAGES[route] ?? History; return <Page />; })()}
            </div>
          </main>
        </div>
      )}
    </ToastProvider>
  );
}
