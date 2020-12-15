package appdialogs

import (
	"path/filepath"

	"github.com/OpenPeeDeeP/xdg"
	"github.com/theparanoids/aterm/dialog"
	"github.com/theparanoids/aterm/fancy"
)

func queryWithDefault(prompt string, guessValue *string, bailFunc func()) dialog.QueryResponse {
	if guessValue != nil && *guessValue != "" {
		prompt += " [" + fancy.AsBlue(*guessValue) + "]"
	}

	resp := HandleUserQuery(prompt, nil, bailFunc)
	if resp.Err != nil {
		return resp
	}
	if resp.Value == nil {
		resp.Value = strPtr("")
	}

	if *resp.Value == "" && guessValue != nil {
		resp.Value = guessValue
	}
	return resp
}

// askFor creates a pretty message, then prompts the user to respond with a free-text field
// Requires an AskForTemplateFields. If AskForTemplateFields.WithPrompt is false, no pretty message
// appears. Instead, only the prompt is provided
func askFor(msg AskForTemplateFields, guessValue *string, bailFunc func()) dialog.QueryResponse {
	if msg.WithPreamble {
		askForTemplate.Execute(medium, msg)
	}

	return queryWithDefault(msg.Prompt, guessValue, bailFunc)
}

// thisOrThat provides a mechansim to return either the provided "this", or if nil (or empty string),
// the provided "that", converted to a string pointer
func thisOrThat(this *string, that string) *string {
	if this == nil || *this == "" {
		return strPtr(that)
	}
	return this
}

func strPtr(s string) *string {
	return &s
}

func realize(s *string) string {
	if s == nil {
		return ""
	}
	return *s
}

// defaultRecordingHome represents the path to what a first time user would be suggested as a location
// to store recordings.
var defaultRecordingHome = filepath.Join(xdg.DataHome(), "aterm", "recordings")
