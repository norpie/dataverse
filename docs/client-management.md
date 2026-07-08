# Client Management

Client Management is where Dataverse TUI stores accounts, environments, and OAuth tokens. Apps use the currently active account/environment pair as the active client.

Open it with `alt+m`, or click the **Client** section in the taskbar.

## Add an active OAuth client

### 1. Add an account

1. Open **Client Management**.
2. Go to **Accounts** (`3`).
3. Select **Add**.
4. Enter:
   - **Client ID** — the Azure app/client ID used for OAuth.
   - **Tenant ID** — optional in the UI, but **required for VAF**.
   - **Name** — a friendly label for this account/client.
5. Select **Add**.

### 2. Add an environment

1. Go to **Environments** (`2`).
2. Select **Add**.
3. Enter:
   - **URL** — the Dataverse environment URL, for example `https://org.crm.dynamics.com`.
   - **Name** — a friendly environment label.
4. Select **Add**.

### 3. Authenticate the account/environment pair

1. Go to **Active** (`1`).
2. Select the environment.
3. Select the account.
4. Select **Connect**.
5. Complete the OAuth flow in the browser.

The browser should open automatically. If it does not, use **Open Browser** in the authentication modal. When OAuth succeeds, the token is saved and this account/environment pair becomes the active session.

## Re-authentication

If tokens already exist for the selected account/environment, the Active tab shows the connection status:

- **Connected** — tokens are available.
- **Needs re-auth** — tokens are expired or close to expiry.
- **Not connected** — no tokens are stored for that pair.

For connected or expired pairs, the button changes to **Re-authenticate**. Use it to run the browser OAuth flow again and replace the saved tokens.

## Taskbar status

The taskbar has a **Client** section:

- green `Connected` status when an active session exists,
- muted `Not connected` status when no active session exists,
- the active environment name, account name, and environment URL when connected.

Clicking the Client section opens Client Management.
