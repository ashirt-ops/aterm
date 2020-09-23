package appdialogs

import "github.com/theparanoids/aterm/dialog"

// UserQuery is a re-packaging of dialog.UserQuery with inputStream pre-provided
func UserQuery(question string, defaultValue *string) (string, error) {
	return dialog.UserQuery(question, defaultValue, internalMenuState.DialogInput)
}

// HandleUserQuery is a re-packaging of dialog.HandleUserQuery with inputStream pre-provided
func HandleUserQuery(question string, defaultValue *string, bailFunc func()) dialog.QueryResponse {
	return dialog.HandleUserQuery(question, defaultValue, internalMenuState.DialogInput, bailFunc)
}

// PlainSelect is a re-packaing of the dialog.PlainSelect with inputStream pre-provided
func PlainSelect(label string, options []dialog.SimpleOption) (dialog.SimpleOption, error) {
	return dialog.PlainSelect(label, options, internalMenuState.DialogInput)
}

// HandlePlainSelect is a re-packaging of dialog.HandlePlainSelect with inputStream pre-provided
func HandlePlainSelect(label string, options []dialog.SimpleOption, bailFunc func() dialog.SimpleOption) dialog.SelectResponse {
	return dialog.HandlePlainSelect(label, options, internalMenuState.DialogInput, bailFunc)
}

// YesNoSelect is a re-packaing of the dialog.YesNoPrompt with inputStream pre-provided
func YesNoSelect(label, details string) (bool, error) {
	return dialog.YesNoPrompt(label, details, internalMenuState.DialogInput)
}
