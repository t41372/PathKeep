/**
 * Check the workflow authorization boundary for the contents-only Actions token.
 * Responsibilities: compare the complete workflow subtree with the default branch.
 * Not responsible for storing credentials, granting scopes or publishing releases.
 */
async function workflowTree(api, ref) {
  const entry = (tree, path) => {
    if (tree?.truncated !== false || !Array.isArray(tree.tree)) {
      throw new Error(
        'Incomplete Git tree metadata for workflow authorization.',
      )
    }
    const found = tree.tree.find((item) => item.path === path)
    if (!found) return null
    if (
      found.type !== 'tree' ||
      !/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(found.sha ?? '')
    ) {
      throw new Error('Invalid workflow tree metadata.')
    }
    return found.sha
  }
  const root = await api.json(`/git/trees/${encodeURIComponent(ref)}`)
  const github = entry(root, '.github')
  if (github === null) return null
  return entry(await api.json(`/git/trees/${github}`), 'workflows')
}

/**
 * Workflow-changing release targets need authorization the default Actions token
 * cannot receive. Tree IDs cover every file, without the compare API's file limit.
 */
export async function workflowsMatchDefaultBranch(api, source) {
  const repository = await api.json('')
  if (
    typeof repository?.default_branch !== 'string' ||
    !repository.default_branch
  )
    throw new Error('Default branch metadata is required.')
  const [target, current] = await Promise.all([
    workflowTree(api, source),
    workflowTree(api, repository.default_branch),
  ])
  return target === current
}
