# Code signing policy

## Status

The project is applying for the SignPath Foundation open-source code-signing
program. Releases remain unsigned until the application is approved and the
verified signing workflow is enabled.

After approval, the following provider statement applies:

> Free code signing provided by SignPath.io, certificate by SignPath Foundation

Release binaries must be built from the public GitHub repository by a
GitHub-hosted runner. Every production signing request must be manually
approved before the signed executable is published.

## Team roles

- Committer and reviewer: [wenxingyu](https://github.com/wenxingyu)
- Signing approver: [wenxingyu](https://github.com/wenxingyu)

## Privacy

This program will not transfer any information to other networked systems
unless specifically requested by the user or the person installing or
operating it.

When the user launches ChatGPT, the launcher starts the locally installed
ChatGPT application with the proxy choice supplied by that user. ChatGPT and
the configured proxy are separate services and are governed by their own
privacy policies. The launcher does not collect analytics or telemetry.

The proxy configuration is stored locally in
`%APPDATA%\startChatGPT\config.txt`.

Starting the launcher also starts an account quota monitor. It uses the locally
installed and authenticated Codex CLI to request quota information from Codex
services every 45 seconds using the selected proxy. It does not start model
conversations or copy or log login tokens. The monitor remains active until
the user exits it from its tray menu.
