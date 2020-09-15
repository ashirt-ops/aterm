# Ashirt Terminal Recorder

It records your terminal, then lets you upload the result.

## Overview / User's Guide

The terminal recorder can be started via the `aterm` binary. Once the application starts, a terminal recording file will be generated per your configuration. While this terminal is active, all _output_ events will be stored in this file, so there is no need to worry about passwords, etc being stored from keyboard input.
_To exit the terminal_: Use any standard way to exit a terminal: namely, `exit` or `ctrl+D` (on an empty prompt).

After the recording, you will be prompted to either upload the file, or exit. If you choose to upload, you will need to supply an operation, file, and description for the event. This will create a new piece of evidence inside the Ashirt application. During this interface, you can navigate with up and down arrows, and can exit some menus with `ctrl+c`. Additionally, many menus support searching for options by pressing `/` and then adding a search term.

### Configuration

This binary supports a few configuration options, and will attempt to load from each configuration level in order to come up with a complete view of how the interaction should be handled. The configuration levels are as follows: First, load from the config file, then replace with defined values from the env vars, then replace with command line switches.

Currently, the configuration file is expected to be found here: `$(HOME)/.config/ashirt/term-recorder.yaml`, however this may be changed in the future on a per-os level. If this file is not found, one will be generated with default (mostly empty) values. Ultimately, the code can be reviewed here for the current location. This default location can be found in `cmd/aterm/config/config.go`.

| Config File Parameter | Env Parameter                         | CLI flag         | Meaning                                                                     |
| --------------------- | ------------------------------------- | ---------------- | --------------------------------------------------------------------------- |
| outputDir             | ASHIRT_TERM_RECORDER_OUTPUT_DIR       | --output-dir     | Determines where to store recording files. Defaults to OS temp directory    |
| recordingShell        | ASHIRT_TERM_RECORDER_RECORDING_SHELL  | --shell       -s | Which shell to use when starting up (defaults to env's SHELL)               |
| operationID           | ASHIRT_TERM_RECORDER_OPERATION_ID     | --operation      | Which operation to upload to (by default -- can be selected during upload)  |
| apiURL                | ASHIRT_TERM_RECORDER_API_URL          | --svc            | Where the **backend** service is located.                                   |
| N/A                   | ASHIRT_TERM_RECORDER_OUTPUT_FILE_NAME | --output-file -o | What filename to use when writing the file locally (and remotely as well)   |
| accessKey             | ASHIRT_TERM_RECORDER_ACCESS_KEY       | N/A              | The Access Key needed to connect with the backend (created on the frontend) |
| secretKey             | ASHIRT_TERM_RECORDER_SECRET_KEY       | N/A              | The Secret Key needed to connect with the backend (created on the frontend) |

### Known Issues

1. It is not "natural" to upload a file that wasn't just recorded.
2. Not possible to re-name files on upload
