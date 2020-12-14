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

This application supports multiple configuration areas. To complicate matters, some configuration techniques support versioning, while certain options are overridable at different levels. The charts below indicate what values are configurable, and what effect they'll have.

Note: values for the configuration files (config.yaml and servers.json) can be specified by using the application -- a fresh start will guide a user towards some initial setup.

#### Overriding configuration values

In general, there are 4 levels of configuration:

* Default values (used when no configuration is provided)
* Values specified in the config files
* Values specified with environment variables
* Values specified with command line arguments

Values are presented to the application in the order specified above, meaning that command line arguments override environment variables, which in turn override config file values, which ultimately override default values.

#### File Paths

This application looks for files based on the `XDG_CONFIG_HOME` value (if present), or alternatively what would be an appropriate path for operating systems that don't support the XDG standard. Below, file locations will be listed as `${XDG_CONFIG_HOME}`. That, in turn, means the following on the below operating systems

| OS      | Possible location               | Notes                                                                                                            |
| ------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Windows | `%LOCALAPPDATA%`                | Unsupported, but once it is, it will be located here                                                             |
| Mac OS  | `~/Library/Application Support` |                                                                                                                  |
| Linux   | typically `~/.config`           | Uses the actual XDG_CONFIG_HOME variable; this is configurable, so check `$XDG_CONFIG_HOME` on your own computer |

#### Configuration File

The configuration file has been versioned, and _currently_ contains 2 versions (numbered 1 and 2). Applications that currently have a version 1 config will be silently upgraded into a version 2 config on the next run.

Location: `${XDG_CONFIG_HOME}/aterm/config.yaml`

The configuration file adheres to the XDG standard, where applicable. If you have XDG_CONFIG_HOME set, then your config file will be found under the `aterm` directory. If this value is not set, then it will likely be saved to `/home/{who}/aterm/config.yaml`.

| Config V1 Parameter | Config V2 Parameter | Meaning                                                                               |
| ------------------- | ------------------- | ------------------------------------------------------------------------------------- |
| `configVersion`     | `configVersion`     | The actual version of the configuration file. This should not be changed by hand      |
| `recordingShell`    | `recordingShell`    | Which shell to use when starting up (defaults to env's SHELL)                         |
| `outputDir`         | `outputDir`         | Determines where to store recording files. Defaults to home directory                 |
| `apiURL`            | N/A                 | Where to access the backend. Moved to Servers                                         |
| `accessKey`         | N/A                 | How to access the backend. Akin to a username. Moved to Servers                       |
| `secretKey`         | N/A                 | How to access the backend. Akin to a password. Moved to Servers                       |
| `operationSlug`     | N/A                 | Obsolete. No current version exists to specify which operation slug to use on startup |

#### Servers File

The servers file is a `json` file format describing how to connect to each backend server.

Location: `${XDG_CONFIG_HOME}/aterm/servers.json`

| Field Name | Type           | Meaning                                                                                         |
| ---------- | -------------- | ----------------------------------------------------------------------------------------------- |
| version    | int            | Which version of the servers file is being used                                                 |
| servers    | servers-struct | A list containing the known servers. See the servers-struct section below for the specification |

##### Servers Struct

A single entry reflecting a server connection.

| Field Name | Type   | Meaning                                                                                                                                                |
| ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| id         | int    | The "order" of the servers. Ordered by (effectively) date-created                                                                                      |
| serverName | string | The name of the server.                                                                                                                                |
| serverUuid | string | A unique identifier for each server. This should not be hand edited                                                                                    |
| accessKey  | string | Part of the information on how to authenticate with the server. Akin to a username                                                                     |
| secretKey  | string | Part of the information on how to authenticate with the server. Akin to a password                                                                     |
| hostPath   | string | Where to find the backend. Typically looks like a standard URL                                                                                         |
| deleted    | bool   | Whether this entry has been "deleted". Servers that are maked as deleted will not be shown, but can be restored by changing this value back to `false` |

#### Environment Variables

Environment Variables have been disabled, due to lack of use.

#### Command Line Arguments

| Flag                   | Alt   | Meaning                                                                        |
| ---------------------- | ----- | ------------------------------------------------------------------------------ |
| `-shell path/to/shell` | `-s`  | Specifies the shell to use when recording                                      |
| `-menu`                | `-m`  | Starts at the main menu, rather than starting a new recording                  |
| `-pid`                 |       | Shows the pid on startup.                                                      |
| `-print-config`        | `-pc` | Shows the current configuration on startup                                     |
| `-reset`               |       | Show the first-run dialog on start (keeps current configuration values)        |
| `-reset-hard`          |       | Shows the first-run dialog on start. Discards the current configuration values |
| `-version`             | `-v`  | Prints the version, then exits                                                 |
| `-help`                | `-h`  | Shows the help menu, then exits                                                |

#### Settings File

The settings file is a `json` file format describing the values used during the last run, to make future runs more automatic.
Note that it is unnecessary to ever view or edit this value by hand, but it is provided here for clarity

Location: `${XDG_CONFIG_HOME}/aterm/settings.json`

| Field Name         | Type                  | Meaning                                                                                            |
| ------------------ | --------------------- | -------------------------------------------------------------------------------------------------- |
| selectedServerUUID | string                | Which server was last used when making a recording                                                 |
| serverState        | server-history-struct | A list of servers and their previous state. See the Server history struct for details on this type |

##### Server History Struct

| Field Name    | Type   | Meaning                                                                    |
| ------------- | ------ | -------------------------------------------------------------------------- |
| serverUuid    | string | An identifier for a given server (matched against the servers file entry)  |
| operationSlug | string | The operation last used when creating a recording for the indicated server |

### Known Issues

1. pressing the delete (not backspace) key generates a `^d` signal, causing input to fail
2. Exiting a recording with `^d` prevents arrow keys from working when the terminal returns. this is due to the terminal being placed into application mode (rather than interactive mode). Navigation with the `j` and `k` keys will continue to work. Entering a new shell and exiting with `exit` will return the application to interactive mode.
