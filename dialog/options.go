// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package dialog

import (
	"errors"
	"io"

	"github.com/manifoldco/promptui"
)

// InvalidSelection is a predefined selection representing choices that errored out.
var InvalidSelection = SimpleOption{invalid: true}

type SimpleOption struct {
	invalid bool
	Label   string
	Data    interface{}
}

func (s *SimpleOption) IsValid() bool {
	return !s.invalid
}

func simpleOptionSliceToStrings(opts []SimpleOption) []string {
	rtn := make([]string, len(opts))
	for i, v := range opts {
		rtn[i] = v.Label
	}
	return rtn
}

// PlainSelect constructs a question, with pre-defined answers. Users must "select" an answer.
// This version works off of SimpleOption, and returns back the selected option and what error
// occurred while making that selection, if any.
func PlainSelect(label string, options []SimpleOption, inputStream io.ReadCloser) (SimpleOption, error) {
	p := MkBasicSelect(inputStream)
	p.Label = label
	p.Searcher = SearcherContainsCI(simpleOptionSliceToStrings(options))

	items := make([]string, len(options))
	for i, o := range options {
		items[i] = o.Label
	}
	p.Items = items

	selectedItem, _, err := p.Run()
	if err != nil {
		return InvalidSelection, err
	}
	return options[selectedItem], nil
}

// SelectResponse captures the net result from a Select Menu selection. See HandlePlainSelect for
// more details. Also note that this struture is mirrored by QueryResponse, for free text entry.
type SelectResponse struct {

	// Selection represents the specfic selection that was made by a user in the select menu.
	Selection SimpleOption

	// Action represents how the user interacted with the select menu -- specifically, did they enter
	// a value, or did they try to avoid entering value, or did the select menu generate an error
	Action UserAction

	// Err is the actual error encountered by the user, if any. Note that the underlying system treats
	// ^c and ^d as errors, whereas these errors are instead translated into actions for
	// this system, and are not represented as errors here.
	Err error
}

// IsKillSignal checks to see if the user action was either a UserActionExit or UserActionCancel --
// i.e. the user tried to back out of the interaction point
func (resp *SelectResponse) IsKillSignal() bool {
	return resp.Action == UserActionExit || resp.Action == UserActionCancel
}

// UserAction is an effective enum representing the various states a user can get into when prompted
// for a response (referred to in documentation as an "interaction point")
type UserAction string

const (
	// UserActionCancel represents a user pressing ^c (Interrupt) when given an interaction point
	UserActionCancel UserAction = "cancel"
	// UserActionExit represents a user pressing ^d (EOF) when given an interaction point
	UserActionExit UserAction = "exit"
	// UserActionEntered represents a user providing a value when given an interaction point (i.e. the normal expectation)
	UserActionEntered UserAction = ""
	// UserActionErrored represents a user encountering an error during an interaction point
	UserActionErrored UserAction = "error"
)

// HandlePlainSelect provides a small wrapper around PlainSelect. This function will generate a CLI
// Select menu, then interpret the results to check if some error was encountered, or if the user
// pressed ^d or ^c to exit out of the select menu. If so, bailFunc is executed, and the caller may
// supply a replacement value in these sencarios. This data is all captured, then returned in a
// SelectResponse, which can be pulled apart to retrieve the selection or error as before
func HandlePlainSelect(label string, options []SimpleOption, inputStream io.ReadCloser, bailFunc func() SimpleOption) SelectResponse {
	selection, err := PlainSelect(label, options, inputStream)
	var resp SelectResponse

	if errors.Is(err, promptui.ErrInterrupt) {
		resp = SelectResponse{Selection: selection, Action: UserActionCancel}
	} else if errors.Is(err, promptui.ErrEOF) {
		resp = SelectResponse{Selection: selection, Action: UserActionExit}
	} else if err != nil {
		resp = SelectResponse{Selection: selection, Action: UserActionErrored, Err: err}
	} else {
		resp = SelectResponse{Selection: selection}
	}
	if resp.IsKillSignal() {
		resp.Selection = bailFunc()
	}

	return resp
}
