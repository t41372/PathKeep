import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createHashRouter, RouterProvider } from 'react-router-dom'
import { TooltipProvider } from '@/components/ui/tooltip'
import './proto.css'
import AgentInsightsPage from './pages/agent/insights'
import ChatPage from './pages/agent/chat-page'
import ChatsPage from './pages/agent/chats'
import ProjectPage from './pages/agent/project'
import ProjectInsightsPage from './pages/agent/project-insights'
import ProjectsPage from './pages/agent/projects'
import AskPage from './pages/ask'
import BackupPage from './pages/backup'
import BrowsingHistoryPage from './pages/browsing'
import BrowsingInsightsPage from './pages/browsing-insights'
import SpaceBackupPage from './pages/backup/space-backup'
import BrowsingHomePage from './pages/browsing-home'
import RootPage from './pages/root'
import TasksPage from './pages/tasks'
import OnboardingPage from './pages/onboarding'
import PlaceholderKindPage from './pages/placeholder-kind'
import SettingsPage from './pages/settings/settings'
import { ProtoProvider } from './proto-state'
import { Frame } from './shell/frame'

const router = createHashRouter([
  { path: '/onboarding', element: <OnboardingPage /> },
  {
    path: '/',
    element: <Frame />,
    children: [
      { index: true, element: <RootPage /> },
      { path: 'browsing/home', element: <BrowsingHomePage /> },
      { path: 'browsing/backup', element: <SpaceBackupPage /> },
      { path: 'agent/backup', element: <SpaceBackupPage /> },
      { path: 'agent/projects/:projectId/insights', element: <ProjectInsightsPage /> },
      { path: 'agent/chats', element: <ChatsPage /> },
      { path: 'agent/chat/:chatId', element: <ChatPage /> },
      { path: 'agent/projects', element: <ProjectsPage /> },
      { path: 'agent/projects/:projectId', element: <ProjectPage /> },
      { path: 'agent/insights', element: <AgentInsightsPage /> },
      { path: 'browsing/history', element: <BrowsingHistoryPage /> },
      { path: 'browsing/insights', element: <BrowsingInsightsPage /> },
      { path: 'k/:kind', element: <PlaceholderKindPage /> },
      { path: 'ask', element: <AskPage /> },
      { path: 'tasks', element: <TasksPage /> },
      { path: 'backup', element: <BackupPage /> },
      { path: 'settings', element: <SettingsPage /> },
      { path: 'settings/:section', element: <SettingsPage /> },
    ],
  },
])

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ProtoProvider>
      <TooltipProvider delayDuration={400}>
        <RouterProvider router={router} />
      </TooltipProvider>
    </ProtoProvider>
  </StrictMode>,
)
