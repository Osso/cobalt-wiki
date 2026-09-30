# Cobalt account color profiles

## Approved choices

- Colors only; no layout profiles.
- Preference saved per account, across devices.
- Classic: historical cyan tabs with black text. Current: existing tab backgrounds; slightly darker label text.
- Switch Luridel to Classic after deployed support is available.

## Design

Use one `color_profile` account field with values `classic` and `current`. Existing and new accounts retain Current unless explicitly changed. Extend the existing user-edit path and My account settings with an Appearance section; resolve the account from the authenticated session, not submitted identity.

Members open My account → Appearance (`/-/settings#appearance`), select Current or Classic under Tab colors, then submit Save appearance. Reopening settings displays the saved choice.

Pass the private account preference through the existing server-rendered layout. Scope navigation fill, hover, and label color variables to the Wikidot layout's profile attribute. Classic uses `#a8d7ea`, `#d6f3ff`, and black; Current retains `#bcc9f2` and `#e0e6fb`, with darker navy labels `#0b1a38`. Preserve rounded corners, layout, content, and unrelated styling.

For invalid form values, refuse the save and show an Appearance error. Backend validation must also reject unsupported values. Saving unrelated settings must not reset the preference.

Backend integration, verification, deployment, and Luridel's post-deploy selection remain main's work; this design does not establish their completion.

Requirements and proof inventory live in the [feature spec](../specs/cobalt-color-profiles.md). No light/dark, System option, browser-local preference, or TOC repair is part of this profile change.
