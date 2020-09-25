# ASHIRT Terminal Recorder (ATerm)

ATerm provides the ability to record a terminal session in a separate pty. After recording, you can upload the file to an ASHIRT server.

## Overview / User's Guide

The terminal recorder can be started via the `aterm` binary.

There are a handful of modes and options that can be supplied at startup. The application attempts to describe what it is doing, and the menus try to be intuative. This overview tries to provide some basic guidance without being overly thorough.

### First Run

On first run, a small dialog will run asking for various required details (API URL, Access Key, Secret Key, etc). This data is saved, though first run can be triggered again via command line options. More details on this below.

### Navigating Menus

Menus primarily navigate through arrow keys and `j`, `k` keys. Pressing `/` will allow you to search/filter the options by name. Any case-insenstive substring will be included post-filter. Pressing `/` once more will leave search.

### Starting a recording

A normal start of the `aterm` binary will attempt to start a new recording. The application will prompt you to select an operation to associate with the recording. Select an operation from the list, and the psuedo terminal will start. The terminal should behave exactly as normal.

To exit a recording, try entering `exit` or pressing `ctrl+D` on an empty prompt.

### Uploading a recording

After each recording, a small menu is presented with available options:

1. Upload Recording
   * The primary intent after recording is to upload that recording. A small guide will prompt you to supply a description and select valid tags for this recording. After this data has been collected, you may submit this to the server. A successful submit will save the recorded metadata (e.g. description and tags) and send you to the main menu.
2. Rename Recording File
   * For certain cases, you may want to make the recording file a bit more permanent/memorable. In these cases, you can opt to rename the recording to any name, normal filename rules still apply.
3. Discard Recording
   * In sitatutions where the recording was unfruitful, you can opt to delete the recording.
4. Return to Main Menu
   * As the name implies, you can return to the normal menu. You can exit from here. Returning to the main menu saves the recording metadata as well.

### Configuration

This binary supports a few configuration options, and will attempt to load from each configuration level in order to come up with a complete view of how the interaction should be handled. The configuration levels are as follows: First, load from the config file, then replace with defined values from the env vars, then replace with command line switches.

Additionally, on first run, if you are using the ASHIRT application, then some configuration details can be pulled from its configuration file.

The configuration file adheres to the XDG standard, where applicable. If you have XDG_CONFIG_HOME set, then your config file will be found under the `ashirt` directory. If this value is not set, then it will likely be saved to `/home/{who}/ashirt/aterm.yaml`. However, most settings can be tweaked in the application itself, by going to the main menu and choosing "Update Settings". A small guide will take you through common configuration values

| Config File Parameter | Env Parameter                         | CLI flag          | Meaning                                                                                               |
| --------------------- | ------------------------------------- | ----------------- | ----------------------------------------------------------------------------------------------------- |
| outputDir             | ASHIRT_TERM_RECORDER_OUTPUT_DIR       |                   | Determines where to store recording files. Defaults to home directory                                 |
| recordingShell        | ASHIRT_TERM_RECORDER_RECORDING_SHELL  | -shell       -s   | Which shell to use when starting up (defaults to env's SHELL)                                         |
| operationSlug         | ASHIRT_TERM_RECORDER_OPERATION_SLUG   | -operation        | Which operation to upload to (by default -- can be selected prior to recording)                       |
| apiURL                | ASHIRT_TERM_RECORDER_API_URL          |                   | Where the **backend** service is located.                                                             |
| N/A                   | ASHIRT_TERM_RECORDER_OUTPUT_FILE_NAME | --name -n         | What filename to use when writing the file locally (and remotely as well)                             |
| accessKey             | ASHIRT_TERM_RECORDER_ACCESS_KEY       | N/A               | The Access Key needed to connect with the backend (created on the frontend)                           |
| secretKey             | ASHIRT_TERM_RECORDER_SECRET_KEY       | N/A               | The Secret Key needed to connect with the backend (created on the frontend). This is a base-64 value  |
|                       |                                       | -menu -m          | Starts in the main menu                                                                               |
|                       |                                       | -pring-config -pc | Prints the loaded configuration, then exits                                                           |
|                       |                                       | -help -h          | Opens the help menu                                                                                   |
|                       |                                       | -shell -s         | Launches the recoder with the specified shell. This should be the path to the binary                  |
|                       |                                       | -reset            | Launches first-run to set up initial values. Uses the existing values as a base.                      |
|                       |                                       | -reset-hard       | Launches first-run to set up initial config values. Does not use the existing configuration as a base |

### Known Issues

1. pressing the delete (not backspace) key generates a `^d` signal, causing input to fail
2. Exiting a recording with `^d` prevents arrow keys from working when the terminal returns. this is due to the terminal being placed into application mode (rather than interactive mode). Navigation with the `j` and `k` keys will continue to work. Entering a new shell and exiting with `exit` will return the application to interactive mode.
