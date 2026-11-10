# Connect Ulož.to Disk to Backup Sync

This guide walks through creating a Ulož.to account, generating a personal application token and signing in through Backup Sync. The application currently uses Slovak labels, which are included alongside the English instructions.

## 1. Create or sign in to your Ulož.to account

1. Open [Ulož.to Disk](https://uloz.to/).
2. Register for an account if you are a new user, then sign in.
3. Find your Ulož.to username. You will enter this username in Backup Sync.

## 2. Generate an application token on Ulož.to

1. Open your account **Settings** on the Ulož.to website.
2. Find the **Application token** section. Depending on the website language, it may be labelled **Aplikačný token** or **Aplikační token**.
3. Create a new application token. If the website asks for a name, use a recognisable label such as **Backup Sync**.
4. Copy the generated token for the next step.

This personal token authorises Backup Sync to access your Ulož.to account.

## 3. Sign in through Backup Sync

1. Open **Backup Sync**.
2. Go to **Settings** (`Nastavenia`).
3. Scroll to the **Ulož.to Disk** section.
4. Fill in the following fields.

| Field in Backup Sync | What to enter |
| --- | --- |
| **Login** | Your username from your Ulož.to account |
| **Application token** (`Aplikačný token`) | The personal application token you generated on the Ulož.to website |
| **Application API key (optional)** (`API kľúč aplikácie (voliteľný)`) | Leave this empty for the standard setup |

5. Click **Sign in** (`Prihlásiť`).
6. Confirm that the section shows **Connected** (`Pripojené`) with your account name.

The optional application API key identifies Backup Sync when it communicates with Ulož.to. An empty field uses the public test application key bundled with the app. A custom application API key can be obtained from Ulož.to support.

Your personal application token belongs in **Application token** (`Aplikačný token`). Backup Sync stores account tokens locally in Windows Credential Manager.

## 4. Back up a game or folder

1. Open **Games** (`Hry`) or **Folders** (`Priečinky`).
2. Add the game or folder and choose **Ulož.to Disk** as its cloud provider.
3. For a game, select **Upload to cloud** (`Nahrať na cloud`). For a folder, select **Back up now** (`Zálohovať teraz`).
4. Wait for the operation to finish and check the item's status.

Automatic cloud uploads are off by default in a fresh configuration. Enable automatic uploads for the item or in Settings if you want changed files to upload automatically while the app is running.

## Troubleshooting

| Problem | What to check |
| --- | --- |
| Sign-in is rejected | Check your Ulož.to username and copy the complete generated token into **Application token** (`Aplikačný token`). |
| Ulož.to reports too many sign-in attempts | Wait a few minutes before trying again. Backup Sync reuses a saved session after a successful sign-in. |
| A previously working token stops working | Check the token in your Ulož.to account settings. If it was revoked, generate a replacement and sign in again in Backup Sync. |
| You need to use a different account | Click **Sign out** (`Odhlásiť`) in the Ulož.to Disk section, then sign in with the other account's username and application token. |

[Back to the application guide](../README.md) · [Documentation index](README.md)
