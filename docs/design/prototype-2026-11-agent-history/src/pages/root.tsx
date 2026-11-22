/** `/` opens the first page of the current space. */
import { Navigate } from 'react-router-dom'
import { kindById } from '~/kinds'
import { useProto } from '~/proto-state'

export default function RootPage() {
  const { space } = useProto()
  return <Navigate to={kindById[space].spacePages[0].path + location.hash.replace(/^#\/?[^?]*/, '')} replace />
}
