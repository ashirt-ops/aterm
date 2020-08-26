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

1. Starting second recording (and beyond) in the same session eats the first character typed.
2. pressing the delete (not backspace) key generates a `^d` signal, causing input to fail

## Development Overview

This program is actually two programs mascarading as one, and perhaps there's some value in splitting them up. The first sub-application is a somewhat complicated pty recorder. What this does is present a terminal for the user, then captures all _output_ generated from that pty session. The second sub-application is a pretty straight forward file uploader, with some somewhat complex cli interaction. Luckily, in both cases, a lot of the true complexity lies in the libraries being used. That said, there's a still a bit of wiring to go over.

### Requirements

This application has only a few requirements:

* Operating System is Linux or Mac OSX.
* Go 1.14

There's a soft requirement of Make as well, though this can be omitted if the make commands are run directly instead.

### Project Structure

### Phase 1: Terminal recording

The big idea with this phase is simply starting up the pty console in such a way as to store the result. The pty itself is managed in `cmd/aterm/ptystate.go`. However, in order to capture results, we need to wire up the recorder with a set of `io.Writer`s (one for stdin, one for stdout). Once we have a pair of writers, we need a way to generate events, and something to record those events somewhere. Finally, we need something to intepret those events and convert the raw event into something an interpreter will later be able to parse. Within the code, this is broken into the following components:

| Component / Package     | Role                                                                   | Notes                                                          |
| ----------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------- |
| Eventer                 | Converts io.Writer bytes into a raw event                              | Can be customized with middleware for more customized behavior |
| Recorders               | Provides mechanism to control output sections / manage stream metadata |                                                                |
| Formatter               | Converts the raw event into a particular format                        | (e.g. asciinema format)                                        |
| Terminal Writer (write) | Manages output mechanism                                               | (e.g. writing to a file)                                       |

As mentioned above, the pty is configured with a pair of io.Writer instances. The first, and primary, is a muxed/multiplexed stdout and eventer. This essentially alows the pty to communicate stdout events both to the user (via stdout) and to the recording system (via the configured eventer). The second writer is a feed off of stdin, allowing the underlying system to react to input-related events as needed. Note, however, that these events are passed unfilterd into the pty, so it is not currently possible to ignore key events.

With this knowledge, the flow then, for output related events is as follows:

pty generates stdout-bound event -> Eventer sees this, and runs through various middleware to generate a raw event -> Event is passed to recorder, which in turn wirtes to the terminal writer -> Terminal writer passes to formatter conform the event -> Terminal writer writes to it's output stream

### Phase 2: Uploading

The upload section is really just a collection of CLI dialogs. The upload dialog itself can be found in `cmd/aterm/appdialogs/upload_prompt.go`, but a lot of the underlying code used there is actually located under `dialogs`. Once the upload is triggered, the upload action is deferred to `network`.

### Building

Executables can be created via the stanard `go build` tool from the `cmd/aterm` directory.


### Known Dev Issues
