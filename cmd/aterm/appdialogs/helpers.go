package appdialogs

import (
	"errors"
	"path/filepath"

	"github.com/OpenPeeDeeP/xdg"
	"github.com/manifoldco/promptui"
	"github.com/theparanoids/aterm/fancy"
)

type UserAction string

const (
	UserActionCancel  UserAction = "cancel"
	UserActionExit    UserAction = "exit"
	UserActionEntered UserAction = ""
	UserActionErrored UserAction = "error"
)

type QueryResponse struct {
	Value  *string
	Action UserAction
	Err    error
}

func (resp *QueryResponse) IsKillSignal() bool {
	return resp.Action == UserActionExit || resp.Action == UserActionCancel
}

func queryWithDefault(prompt string, guessValue *string) (string, error) {
	if guessValue != nil && *guessValue != "" {
		prompt += " [" + fancy.AsBlue(*guessValue) + "]"
	}

	answer, err := UserQuery(prompt, nil)
	if err != nil {
		return "", err
	}
	if answer == "" && guessValue != nil {
		return *guessValue, nil
	}
	return answer, nil
}

// askFor creates a pretty message, then prompts the user to respond with a free-text field
// Requires an AskForTemplateFields. If AskForTemplateFields.WithPrompt is false, no pretty message
// appears. Instead, only the prompt is provided
func askFor(msg AskForTemplateFields, guessValue *string) QueryResponse {
	if msg.WithPreamble {
		askForTemplate.Execute(medium, msg)
	}

	val, err := queryWithDefault(msg.Prompt, guessValue)
	if errors.Is(err, promptui.ErrInterrupt) {
		return QueryResponse{Value: guessValue, Action: UserActionCancel}
	} else if errors.Is(err, promptui.ErrEOF) {
		return QueryResponse{Value: guessValue, Action: UserActionExit}
	} else if err != nil {
		return QueryResponse{Value: guessValue, Action: UserActionErrored, Err: err}
	}
	return QueryResponse{Value: &val}
}

// thisOrThat provides a mechansim to return either the provided "this", or if nil, the provided
// "that", converted to a string pointer
func thisOrThat(this *string, that string) *string {
	if this == nil {
		return strPtr(that)
	}
	return this
}

func strPtr(s string) *string {
	return &s
}

// defaultRecordingHome represents the path to what a first time user would be suggested as a location
// to store recordings.
var defaultRecordingHome = filepath.Join(xdg.DataHome(), "aterm", "recordings")
