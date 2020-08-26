package appdialogs

import "github.com/theparanoids/aterm/dialog"

// UserQuery is a re-packaging of dialog.UserQuery with inputStream pre-provided
func UserQuery(question string, defaultValue *string) (string, error) {
	return dialog.UserQuery(question, defaultValue, internalMenuState.DialogInput)
}

// PlainSelect is a re-packaing of the dialog.PlainSelect with inputStream pre-provided
func PlainSelect(label string, options []dialog.SimpleOption) (dialog.SimpleOption, error) {
	return dialog.PlainSelect(label, options, internalMenuState.DialogInput)
}
