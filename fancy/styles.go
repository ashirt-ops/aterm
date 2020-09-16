// Copyright 2020, Verizon Media
// Licensed under the terms of the MIT. See LICENSE file in project root for terms.

package fancy

import (
	"strings"
)

// see https://misc.flogisoft.com/bash/tip_colors_and_formatting
// see also: https://www.systutorials.com/docs/linux/man/4-console_codes/ (newer data)

const (
	Bold int64 = 1 << (iota)
	Dim
	Underlined
	Blink
	Reverse
	Hidden
	Black
	Red
	Green
	BrownOrange
	Blue
	Purple
	Cyan
	LightGray
	DarkGray
	LightRed
	LightGreen
	Yellow
	LightBlue
	LightPurple
	LightCyan
	White
	BlackBg
	RedBg
	GreenBg
	BrownOrangeBg
	BlueBg
	PurpleBg
	CyanBg
	LightGrayBg
	DarkGrayBg
	LightRedBg
	LightGreenBg
	YellowBg
	LightBlueBg
	LightPurpleBg
	LightCyanBg
	WhiteBg
)

func flagMap() map[int64]string {
	return map[int64]string{
		Bold:          sBold,
		Dim:           sDim,
		Underlined:    sUnderlined,
		Blink:         sBlink,
		Reverse:       sReverse,
		Hidden:        sHidden,
		Black:         cBlack,
		Red:           cRed,
		Green:         cGreen,
		BrownOrange:   cBrownOrange,
		Blue:          cBlue,
		Purple:        cPurple,
		Cyan:          cCyan,
		LightGray:     cLightGray,
		DarkGray:      cDarkGray,
		LightRed:      cLightRed,
		LightGreen:    cLightGreen,
		Yellow:        cYellow,
		LightBlue:     cLightBlue,
		LightPurple:   cLightPurple,
		LightCyan:     cLightCyan,
		White:         cWhite,
		RedBg:         cRedBg,
		GreenBg:       cGreenBg,
		BrownOrangeBg: cBrownOrangeBg,
		BlueBg:        cBlueBg,
		PurpleBg:      cPurpleBg,
		CyanBg:        cCyanBg,
		LightGrayBg:   cLightGrayBg,
		DarkGrayBg:    cDarkGrayBg,
		LightRedBg:    cLightRedBg,
		LightGreenBg:  cLightGreenBg,
		YellowBg:      cYellowBg,
		LightBlueBg:   cLightBlueBg,
		LightPurpleBg: cLightPurpleBg,
		LightCyanBg:   cLightCyanBg,
		WhiteBg:       cWhiteBg,
	}
}

func antiFlagMap() map[int64]string {
	return map[int64]string{
		Bold:       sNormalWeight,
		Dim:        sNormalWeight,
		Underlined: sUnUnderlined,
		Blink:      sUnBlink,
		Reverse:    sUnReverse,
		Hidden:     sUnHidden,

		Black:       cDefaultFg,
		Red:         cDefaultFg,
		Green:       cDefaultFg,
		BrownOrange: cDefaultFg,
		Blue:        cDefaultFg,
		Purple:      cDefaultFg,
		Cyan:        cDefaultFg,
		LightGray:   cDefaultFg,
		DarkGray:    cDefaultFg,
		LightRed:    cDefaultFg,
		LightGreen:  cDefaultFg,
		Yellow:      cDefaultFg,
		LightBlue:   cDefaultFg,
		LightPurple: cDefaultFg,
		LightCyan:   cDefaultFg,
		White:       cDefaultFg,

		RedBg:         cDefaultBg,
		GreenBg:       cDefaultBg,
		BrownOrangeBg: cDefaultBg,
		BlueBg:        cDefaultBg,
		PurpleBg:      cDefaultBg,
		CyanBg:        cDefaultBg,
		LightGrayBg:   cDefaultBg,
		DarkGrayBg:    cDefaultBg,
		LightRedBg:    cDefaultBg,
		LightGreenBg:  cDefaultBg,
		YellowBg:      cDefaultBg,
		LightBlueBg:   cDefaultBg,
		LightPurpleBg: cDefaultBg,
		LightCyanBg:   cDefaultBg,
		WhiteBg:       cDefaultBg,
	}
}

const escape = "\033["

// Plain resets the coloring to the default terminal value
const Plain = "\033[0m"

// Clear removes all text from the line
const Clear = "\033[2K"

const (
	sAllDefaults string = "0"
	sBold        string = "1"
	sDim         string = "2"
	sItal        string = "3"
	sUnderlined  string = "4"
	sBlink       string = "5"
	sReverse     string = "7"
	sHidden      string = "8"
	sStrikeout   string = "9"

	sNormalWeight string = "22" //anti bold _and_ anti dim
	sUnUnderlined string = "24"
	sUnBlink      string = "25"
	sUnReverse    string = "27"
	sUnHidden     string = "28"
	sUnStrikeout  string = "29"
)

const (
	cClear string = "2K"

	cBlack       string = "30"
	cRed         string = "31"
	cGreen       string = "32"
	cBrownOrange string = "33"
	cBlue        string = "34"
	cPurple      string = "35"
	cCyan        string = "36"
	cLightGray   string = "37"
	cDefaultFg   string = "39"

	cBlackBg       string = "40"
	cRedBg         string = "41"
	cGreenBg       string = "42"
	cBrownOrangeBg string = "43"
	cBlueBg        string = "44"
	cPurpleBg      string = "45"
	cCyanBg        string = "46"
	cLightGrayBg   string = "47"
	cDefaultBg     string = "47"

	cDarkGray    string = "90"
	cLightRed    string = "91"
	cLightGreen  string = "92"
	cYellow      string = "93"
	cLightBlue   string = "94"
	cLightPurple string = "95"
	cLightCyan   string = "96"
	cWhite       string = "97"

	cDarkGrayBg    string = "100"
	cLightRedBg    string = "101"
	cLightGreenBg  string = "102"
	cYellowBg      string = "103"
	cLightBlueBg   string = "104"
	cLightPurpleBg string = "105"
	cLightCyanBg   string = "106"
	cWhiteBg       string = "107"
)

// WithPizzazz allows for text to be rendered with terminal styling (colors, weight, underline + other effects)
func WithPizzazz(s string, flags int64) string {
	return WithMorePizzazz(s, flags) + Plain
}

func WithMorePizzazz(s string, flags int64) string {
	prefix := ""
	suffix := ""
	codes := []string{}
	antiCodes := []string{}
	for k, v := range flagMap() {
		if flags&k > 0 {
			codes = append(codes, v)
		}
	}
	for k, v := range antiFlagMap() {
		if flags&k > 0 {
			antiCodes = append(antiCodes, v)
		}
	}
	if len(codes) > 0 {
		prefix = codify(codes...)
		suffix = codify(antiCodes...)
	}

	return prefix + s + suffix
}

// WithBold is a shorthand for WithPizzazz(s, flags | Bold)
func WithBold(s string, otherFlags ...int64) string {
	combinedFlags := Bold | mergeFlags(otherFlags)
	return WithPizzazz(s, combinedFlags)
}

func codify(codes ...string) string {
	return escape + strings.Join(codes, ";") + "m"
}

func AsUnderlined(s string) string {
	return codify(sUnderlined) + s + codify(sUnUnderlined)
}

// AsBold constructs a string where the contents of the string are in bold. A terminating "non-bold"
// character immediately follows, terminating the effect
// Note: Will remove Dim if also set
func AsBold(s string) string {
	return codify(sBold) + s + codify(sNormalWeight)
}

// AsDim constructs a string where the contents of the string are in "half-bright" (dim). A terminating "non-dim"
// character immediately follows, terminating the effect
// Note: Will remove Bold if also set
func AsDim(s string) string {
	return codify(sDim) + s + codify(sNormalWeight)
}

// AsBlue constructs a string that has it's foreground (letters) blue
// Note: this will remove any foreground color, if set
func AsBlue(s string) string {
	return codify(cBlue) + s + codify(cDefaultFg)
}

// AsGreen constructs a string that has it's foreground (letters) green
// Note: this will remove any foreground color, if set
func AsGreen(s string) string {
	return codify(cGreen) + s + codify(cDefaultFg)
}

// AsRed constructs a string that has it's foreground (letters) red
// Note: this will remove any foreground color, if set
func AsRed(s string) string {
	return codify(cRed) + s + codify(cDefaultFg)
}

// AsWhite constructs a string that has it's foreground (letters) white
// Note: this will remove any foreground color, if set
func AsWhite(s string) string {
	return codify(cWhite) + s + codify(cDefaultFg)
}

// AsGreenBg constructs a string that has it's foreground (letters) green
// Note: this will remove any foreground color, if set
func AsGreenBg(s string) string {
	return codify(cGreenBg) + s + codify(cDefaultBg)
}

// AsRedBg constructs a string that has it's foreground (letters) red
// Note: this will remove any foreground color, if set
func AsRedBg(s string) string {
	return codify(cRedBg) + s + codify(cDefaultBg)
}

// ClearLine removes all text from the current line, resets the cursor to the start of the line,
// then calls WithPizzazz(s, flags)
func ClearLine(s string, flags ...int64) string {
	return WithPizzazz(Clear+"\r"+s, mergeFlags(flags))
}

// GreenCheck renders a green unicode checkmark
func GreenCheck() string {
	return WithPizzazz("✔", LightGreen)
}

// RedCross renders a red unicode cross / X mark
func RedCross() string {
	return WithPizzazz("✘", Red)
}

// Caution renders a message in yellow (and red) indicating that some issue occurred
func Caution(message string, err error) string {
	cautionMsg := WithPizzazz("! ", Bold|BrownOrange) + WithPizzazz(message, Black|BrownOrangeBg)
	if err != nil {
		cautionMsg += " : " + WithPizzazz(err.Error(), Red)
	}
	return cautionMsg
}

// Fatal generates a red cross and a message in red
func Fatal(message string, err error) string {
	errMsg := RedCross() + " " + WithPizzazz(message, Bold|Red)

	if err != nil {
		errMsg += " : " + WithPizzazz(err.Error(), Red)
	}
	return errMsg
}

func mergeFlags(flags []int64) int64 {
	rtn := int64(0)
	for _, val := range flags {
		rtn |= val
	}
	return rtn
}
