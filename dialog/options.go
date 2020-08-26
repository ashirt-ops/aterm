// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package dialog

import "io"

// InvalidSelection is a predefined selection for 
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
