/** `/`: the round-2 shell has a Home across types; with spaces it opens the current space. */
import { Navigate } from 'react-router-dom'
import { kindById } from '~/kinds'
import { useProto } from '~/proto-state'
import HomePage from './home'

export default function RootPage() {
  const { nav, space } = useProto()
  if (nav === 'types') return <HomePage />
  return <Navigate to={kindById[space].spacePages[0].path + location.hash.replace(/^#\/?[^?]*/, '')} replace />
}
