// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package recording

import (
	"fmt"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"

	"github.com/creack/pty"
	"github.com/jonboulle/clockwork"
	"github.com/theparanoids/aterm/cmd/aterm/config"
	"github.com/theparanoids/aterm/common"
	"github.com/theparanoids/aterm/errors"
	"github.com/theparanoids/aterm/eventers"
	"github.com/theparanoids/aterm/fancy"
	"github.com/theparanoids/aterm/formatters"
	"github.com/theparanoids/aterm/recorders"
	"github.com/theparanoids/aterm/systemstate"
	"github.com/theparanoids/aterm/write"
	"golang.org/x/crypto/ssh/terminal"
)

// RecordingInput is a small structure for holding all configuration details for starting up
// a recording.
//
// This structure contains the following fields:
// FileDir: Where the file should be stored
// Shell: What shell to use for the PTY
// EventMiddleware: How to transform events that come through
// OnRecordingStart: A hook into the recording process just before actual recording starts
//   This is intended allow the user to provide messaging to the user
type RecordingInput struct {
	FileDir          string
	Shell            string
	TermInput        io.Reader
	EventMiddleware  []eventers.EventMiddleware
	OnRecordingStart func(RecordingOutput)
}

// RecordingOutput is a small structure for communicating in-progress or completed recording details
type RecordingOutput struct {
	FilePath string
}

type recordingConfiguration struct {
	writeTarget  int
	isCopying    bool
	ptyReader    io.ReadCloser
	ptyWriter    io.WriteCloser
	dialogReader io.ReadCloser
	dialogWriter io.WriteCloser
}

var recConfig recordingConfiguration

func DialogReader() io.ReadCloser {
	return recConfig.dialogReader
}

func InitializeRecordings() {
	recConfig.ptyReader, recConfig.ptyWriter = io.Pipe()
	recConfig.dialogReader, recConfig.dialogWriter = io.Pipe()
}

// StartRecording takes control of the terminal and starts a subshell to record input.
func StartRecording(opSlug string) (RecordingOutput, error) {
	if recConfig.ptyReader == nil {
		return RecordingOutput{}, errors.ErrNotInitialized
	}

	recOpts := RecordingInput{
		FileDir:   filepath.Join(config.OutputDir(), opSlug),
		Shell:     config.RecordingShell(),
		TermInput: recConfig.ptyReader,
		OnRecordingStart: func(output RecordingOutput) {
			// These Println occur while the terminal is in a raw state. CRs need to be manually added.
			fmt.Println("Recording to " + fancy.WithBold(output.FilePath) + "\n\r")
			fmt.Println(fancy.WithBold("Recording now live!\r", fancy.Reverse|fancy.LightGreen))
		},
	}

	if size, err := pty.GetsizeFull(os.Stdin); err == nil {
		systemstate.UpdateTermHeight(size.Rows)
		systemstate.UpdateTermWidth(size.Cols)
	}

	if !recConfig.isCopying {
		go func() {
			copyRouter([]io.Writer{recConfig.ptyWriter, recConfig.dialogWriter}, os.Stdin, &recConfig.writeTarget)
			// the above shouldn't end, but just in case, this should help it start back up on next recording.
			recConfig.isCopying = false
		}()
		recConfig.isCopying = true
	}

	recConfig.writeTarget = 0
	// switch to raw input, to stream to pty
	termState, err := terminal.MakeRaw(int(os.Stdin.Fd()))
	if err != nil {
		return RecordingOutput{}, err
	}
	defer func() {
		terminal.Restore(int(os.Stdin.Fd()), termState)
		recConfig.writeTarget = 1
		recConfig.ptyWriter.Write([]byte("a")) // feed a dummy character to force the write target to switch over to the dialog writer
	}()

	return record(recOpts)
}

// copyRouter is based off of io.Copy (and by extension, copyBuffer. This simplifies the implementation
// by always making a buffer, and complicates it by allowing multiple destinations. This allows key
// presses to be routed multiple destinations, in our case allowing one stream to route key presses
// between the subshell and the user interface
func copyRouter(dsts []io.Writer, src io.Reader, target *int) (written int64, err error) {
	size := 32 * 1024
	if l, ok := src.(*io.LimitedReader); ok && int64(size) > l.N {
		if l.N < 1 {
			size = 1
		} else {
			size = int(l.N)
		}
	}
	buf := make([]byte, size)
	for {
		nr, er := src.Read(buf)
		if nr > 0 {
			nw, ew := dsts[*target].Write(buf[0:nr])
			if nw > 0 {
				written += int64(nw)
			}
			if ew != nil {
				err = ew
				break
			}
			if nr != nw {
				err = io.ErrShortWrite
				break
			}
		}
		if er != nil {
			if er != io.EOF {
				err = er
			}
			break
		}
	}
	return written, err
}

func record(ri RecordingInput) (RecordingOutput, error) {
	var result RecordingOutput
	tw, err := write.NewStreamingFileWriter(ri.FileDir, "", formatters.ASCIICast, true)

	if err != nil {
		return result, errors.Wrap(err, "Unable to create file writer")
	}
	result.FilePath = tw.Filepath()

	recorder := recorders.NewStreamingRecorder(tw, clockwork.NewRealClock(), ri.Shell)
	eventWriter := eventers.NewEventWriter(&recorder, common.Output, ri.EventMiddleware...)
	wrappedStdOut := io.MultiWriter(os.Stdout, eventWriter)

	tracker := NewPtyTracker(wrappedStdOut, ioutil.Discard, ri.TermInput, func() { ri.OnRecordingStart(result) })

	err = tracker.Run(ri.Shell)
	if err != nil {
		return result, errors.Wrap(err, `Unable to start the recording. Shell path: "`+ri.Shell+`"`)
	}
	return result, errors.MaybeWrap(tw.Close(), "Issue closing file writer")
}
