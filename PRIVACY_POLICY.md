# Annie Mei Privacy Policy

Effective date: 2026-04-27

This Privacy Policy explains what information Annie Mei collects, how it is used, and how you can request deletion. Annie Mei is a Discord bot with a companion AniList OAuth service.

## Information Annie Mei processes

### Discord information

When you use Annie Mei in Discord, the bot receives information Discord provides for the interaction, such as:

- Your Discord user ID.
- The server, channel, and interaction context needed to respond to commands.
- Slash command names and command inputs, such as anime, manga, character, or song search terms.

### AniList account linking information

If you choose to link your AniList account with `/register`, Annie Mei and its auth service process and store information needed to maintain that link, including:

- Your Discord user ID.
- Your AniList user ID and AniList username.
- AniList OAuth access tokens, refresh tokens when provided, token expiry timestamps, and relink status metadata.
- Temporary OAuth session state used to protect the login flow.

AniList linking is optional. Commands that do not require an AniList link can still be used without linking an account.

### Logs, diagnostics, and error reports

Annie Mei may collect operational logs, diagnostics, and error reports to keep the service reliable and investigate failures. Discord user IDs are hashed or fingerprinted before being attached to application logs or Sentry diagnostics. Error reports may include command names, high-level execution context, and sanitized error details. Credential-bearing URLs are redacted before being sent to application logs or Sentry.

### Cache and infrastructure data

Annie Mei may cache API responses or derived data in Redis to improve performance and reduce third-party API usage. The bot also stores application data in Postgres and uses managed infrastructure providers to operate the service.

## How information is used

Annie Mei uses the information above to:

- Respond to Discord slash commands.
- Link Discord users to AniList accounts when requested.
- Fetch anime, manga, character, user list, and theme song information.
- Display linked-account context such as AniList usernames or scores where supported.
- Prevent OAuth replay or misuse.
- Debug errors, monitor reliability, and protect the service from abuse.

## Third-party services

Annie Mei uses third-party services to operate and provide features, including:

- Discord for bot interactions.
- AniList for anime, manga, character, user, and OAuth data.
- MyAnimeList and Spotify for theme song metadata and links.
- Sentry for error reporting and diagnostics.
- Postgres hosting, Redis hosting, secret management, and deployment providers for infrastructure operations.

These services process data under their own terms and privacy policies.

## Data sharing

Annie Mei does not sell personal information. Information is shared only as needed to operate the bot, provide requested features, comply with legal obligations, protect the service, or use the third-party providers listed above.

## Data retention

- AniList OAuth session state is temporary and expires after a short period.
- AniList account link records are kept until they are no longer needed to provide the linked-account feature or until deletion is requested.
- Logs, diagnostics, caches, and backups may be retained for operational, security, and reliability purposes and then deleted or rotated according to provider and maintainer practices.

## Your choices and deletion requests

You can avoid optional AniList account processing by not using `/register`. If you have linked an AniList account, you may revoke access from AniList where supported.

To request deletion of stored Annie Mei account-link data, open an issue at:

https://github.com/annie-mei/annie-mei/issues

Do not include your numeric Discord ID, OAuth tokens, secrets, or other sensitive information in a public issue. Ask for private deletion support and the maintainer will coordinate a private way to verify the account involved.

## Security

Annie Mei uses technical measures intended to protect stored data, including secret-managed OAuth configuration, hashed or fingerprinted identifiers in diagnostics, and redaction of credential-bearing URLs before application logging or Sentry reporting. No system is perfectly secure, and Annie Mei cannot guarantee absolute security.

## Children's privacy

Annie Mei is intended for users who are allowed to use Discord under Discord's own terms. Annie Mei is not directed at children below the age permitted by Discord.

## Changes to this policy

This policy may be updated from time to time. Continued use of Annie Mei after changes are published means you accept the updated policy.

## Contact

For privacy questions, support, or deletion requests, open an issue at:

https://github.com/annie-mei/annie-mei/issues
