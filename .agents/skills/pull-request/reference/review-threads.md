# Complete PR Feedback Retrieval and Verification

No single GitHub comment list contains all review feedback. Use separate paginated GraphQL queries for inline review threads, top-level review bodies, and general PR conversation comments. Separate queries give `gh --paginate` exactly one unaliased `pageInfo` connection to follow. Inline thread comments use an alias in the outer query and a separate continuation query when needed.

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
              commentsPageInfo: pageInfo { hasNextPage endCursor }
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
  select(.comments.commentsPageInfo.hasNextPage) |
  [.id, .comments.commentsPageInfo.endCursor] | @tsv' \
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

## Retrieve top-level review bodies

Review bodies can contain findings that do not appear in inline threads. Retrieve every review, including empty bodies so review state and commit context remain complete:

```bash
gh api graphql --paginate --slurp \
  -f owner="$OWNER" \
  -f repo="$REPO" \
  -F number="$PR" \
  -f query='query($owner: String!, $repo: String!, $number: Int!, $endCursor: String) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $number) {
        headRefOid
        reviews(first: 100, after: $endCursor) {
          nodes {
            id
            databaseId
            author { login }
            body
            url
            createdAt
            submittedAt
            updatedAt
            state
            commit { oid }
          }
          pageInfo { hasNextPage endCursor }
        }
      }
    }
  }' > /tmp/annie-pr-reviews.json
```

## Retrieve conversation comments

General PR conversation comments can also contain actionable findings. Retrieve every comment:

```bash
gh api graphql --paginate --slurp \
  -f owner="$OWNER" \
  -f repo="$REPO" \
  -F number="$PR" \
  -f query='query($owner: String!, $repo: String!, $number: Int!, $endCursor: String) {
    repository(owner: $owner, name: $repo) {
      pullRequest(number: $number) {
        headRefOid
        comments(first: 100, after: $endCursor) {
          nodes {
            id
            databaseId
            author { login }
            body
            url
            createdAt
            updatedAt
          }
          pageInfo { hasNextPage endCursor }
        }
      }
    }
  }' > /tmp/annie-pr-comments.json
```

## Confirm a consistent feedback snapshot

Each file is an outer array of pages. Confirm every page from all three top-level channels reports one shared head SHA before evaluating findings:

```bash
jq -e -s '
  [.[][] | .data.repository.pullRequest.headRefOid] |
  unique | length == 1
' /tmp/annie-pr-review-threads.json \
  /tmp/annie-pr-reviews.json \
  /tmp/annie-pr-comments.json
```

If this fails, the PR changed during retrieval. Discard every file and repeat all three queries. Treat substantive text from inline comments, review bodies, and conversation comments as findings to verify; metadata-only bot comments and empty review bodies still provide context but are not findings.

## Verify against the current head

1. Read the shared `headRefOid` from the complete feedback snapshot and independently refresh it. Fetch only with explicit fetch authorization:

   ```bash
   HEAD_SHA=$(gh pr view "$PR" --json headRefOid --jq .headRefOid)
   HEAD_REF="refs/remotes/origin/pr/$PR/head"
   git fetch origin "pull/$PR/head:$HEAD_REF"
   test "$(git rev-parse "$HEAD_REF")" = "$HEAD_SHA"
   ```

   The three GraphQL feedback queries require no Git fetch. When fetching code is not authorized, inspect files at the captured SHA through the read-only contents API, for example `gh api "repos/$OWNER/$REPO/contents/<path>?ref=$HEAD_SHA" --jq .content | base64 -d`. Use `gh pr diff "$PR"` and commit APIs for broader context, then refresh `headRefOid`. Report anything that cannot be verified equivalently without a fetch.

2. For every substantive finding, inspect the referenced path and surrounding implementation at `HEAD_SHA` through the fetched `HEAD_REF` (for example, `git show "$HEAD_REF:<path>"`) or the API fallback. Trace related callers/tests when the claim depends on behavior beyond the commented line.
3. Classify each finding as **applies**, **already fixed**, **outdated but still applies**, or **cannot verify**, and cite current-head evidence. Resolution state and `commit.oid` are context only.
4. Refresh `headRefOid` after all-channel retrieval and verification. If it differs from `HEAD_SHA`, discard the entire feedback snapshot and assessment, then repeat every channel against the new SHA.
5. Only after verification may an authorized action respond to a thread, submit a review, update code/body, merge, or close. Each remains separately authorized under `AGENTS.md`.
