# Complete Review-Thread Retrieval and Verification

GitHub's REST review-comment list does not preserve the complete thread model. Use GraphQL and paginate both connections: all review threads, then all comments in any thread whose first comment page is incomplete.

## Retrieve every thread

Set the repository and PR values, then retrieve all thread pages. `--paginate` supplies each successive `$endCursor`; `--slurp` preserves all pages in one JSON array.

```bash
OWNER=annie-mei
REPO=annie-mei
PR=<number>

gh api graphql --paginate --slurp \
  -f owner="$OWNER" \
  -f repo="$REPO" \
  -F number="$PR" \
  -f query='query($owner: String!, $repo: String!, $number: Int!, $endCursor: String) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $number) {
        headRefOid
        reviewThreads(first: 100, after: $endCursor) {
          nodes {
            id
            isResolved
            isOutdated
            path
            line
            originalLine
            comments(first: 100) {
              nodes {
                id
                databaseId
                author { login }
                body
                url
                createdAt
                path
                line
                originalLine
                commit { oid }
              }
              pageInfo { hasNextPage endCursor }
            }
          }
          pageInfo { hasNextPage endCursor }
        }
      }
    }
  }' > /tmp/annie-pr-review-threads.json
```

The output contains one top-level array item per review-thread page. Preserve all items; do not inspect only `[0]`.

## Finish paginating comments

Find every thread whose embedded comments connection has another page:

```bash
jq -r '.[] | .data.repository.pullRequest.reviewThreads.nodes[] |
  select(.comments.pageInfo.hasNextPage) |
  [.id, .comments.pageInfo.endCursor] | @tsv' \
  /tmp/annie-pr-review-threads.json
```

For **each** emitted thread ID, retrieve the remaining comment pages. Keep each output with the initial data; these pages are additional comments, not replacement threads.

```bash
THREAD_ID=<graphql-thread-id>
CURSOR=<comments-end-cursor>

gh api graphql --paginate --slurp \
  -f threadId="$THREAD_ID" \
  -f endCursor="$CURSOR" \
  -f query='query($threadId: ID!, $endCursor: String) {
    node(id: $threadId) {
      ... on PullRequestReviewThread {
        comments(first: 100, after: $endCursor) {
          nodes {
            id
            databaseId
            author { login }
            body
            url
            createdAt
            path
            line
            originalLine
            commit { oid }
          }
          pageInfo { hasNextPage endCursor }
        }
      }
    }
  }' > "/tmp/annie-pr-thread-${THREAD_ID}.json"
```

Do not omit resolved or outdated threads. They remain relevant history and can contain findings that still apply elsewhere.

## Verify against the current head

1. Read `headRefOid` from the retrieval output and independently refresh it with:

   ```bash
   HEAD_SHA=$(gh pr view "$PR" --json headRefOid --jq .headRefOid)
   git fetch origin "pull/$PR/head:refs/remotes/origin/pr/$PR"
   test "$(git rev-parse "refs/remotes/origin/pr/$PR")" = "$HEAD_SHA"
   ```

2. For every substantive finding, inspect the referenced path and surrounding implementation at `HEAD_SHA` (for example, `git show "$HEAD_SHA:<path>"`). Trace related callers/tests when the claim depends on behavior beyond the commented line.
3. Classify each finding as **applies**, **already fixed**, **outdated but still applies**, or **cannot verify**, and cite current-head evidence. Resolution state and `commit.oid` are context only.
4. Refresh `headRefOid` after verification. If it differs from `HEAD_SHA`, discard the stale assessment and repeat against the new SHA.
5. Only after verification may an authorized action respond to a thread, submit a review, update code/body, merge, or close. Each remains separately authorized under `AGENTS.md`.
