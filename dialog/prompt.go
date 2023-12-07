package dialog

import (
	"errors"
	"io"

	"github.com/manifoldco/promptui"
)

// UserQuery presents a free-answer dialog to the user with the given question (and default value,
// if one is provided). Returns the answer to the question, or an error if one is encountered.
func UserQuery(question string, defaultValue *string, inputStream io.ReadCloser) (string, error) {
	p := promptui.Prompt{
		Stdin:   inputStream,
		Label:   question,
		Pointer: promptui.PipeCursor,
	}
	if defaultValue != nil {
		p.Default = *defaultValue
	}
	return p.Run()
}

// QueryResponse condenses the response from a free text prompt. See HandleUserQuery for more details.
// This is also mirrored by SelectResponse for multiple-choice user queries.
type QueryResponse struct {
	Value  *string
	Action UserAction
	Err    error
}

// IsKillSignal checks to see if the user action was either a UserActionExit or UserActionCancel --
// i.e. the user tried to back out of the interaction point
func (resp *QueryResponse) IsKillSignal() bool {
	return resp.Action == UserActionExit || resp.Action == UserActionCancel
}

// SafeValue returns back the actual value of the response, or an empty string if the Value is nil
func (resp *QueryResponse) SafeValue() string {
	if resp.Value == nil {
		return ""
	}
	return *resp.Value
}

// HandleUserQuery provides a small wrapper around UserQuery. This function will generate a
// free-text prompt, then interpret the results to check if some error was encountered, or if the user
// pressed ^d or ^c to exit out of the select menu. If so, bailFunc is executed. This data is all
// captured, then returned in a SelectResponse, which can be pulled apart to retrieve the selection
// or error as before
func HandleUserQuery(question string, defaultValue *string, inputStream io.ReadCloser, bailFunc func()) QueryResponse {
	answer, err := UserQuery(question, defaultValue, inputStream)
	var resp QueryResponse

	if errors.Is(err, promptui.ErrInterrupt) {
		resp = QueryResponse{Value: nil, Action: UserActionCancel}
	} else if errors.Is(err, promptui.ErrEOF) {
		resp = QueryResponse{Value: nil, Action: UserActionExit}
	} else if err != nil {
		resp = QueryResponse{Value: nil, Action: UserActionErrored, Err: err}
	} else {
		resp = QueryResponse{Value: &answer}
	}

	if resp.IsKillSignal() {
		bailFunc()
	}

	return resp
}
