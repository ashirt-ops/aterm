# Developer Notes

## Overview

This program is actually two programs mascarading as one, and perhaps there's some value in splitting them up. The first sub-application is a somewhat complicated pty recorder. What this does is present a terminal for the user, then captures all _output_ generated from that pty session. The second sub-application is a pretty straight forward file uploader, with some somewhat complex cli interaction. Luckily, in both cases, a lot of the true complexity lies in the libraries being used. That said, there's a still a bit of wiring to go over.

## Requirements

This application has only a few requirements:

* Operating System is Linux or Mac OSX.
* Go 1.14

There's a soft requirement of Make as well, though this can be omitted if the make commands are run directly instead.

## Building

Executables can be created via the stanard `go build` tool from the `cmd/aterm` directory.

## Project Structure

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

## Known Dev Issues
