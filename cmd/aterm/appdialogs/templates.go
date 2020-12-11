package appdialogs

import (
	"text/template"

	"github.com/theparanoids/aterm/fancy"
)

// ***** Templates *****

var fancyTemplate = template.New("fancy").Funcs(template.FuncMap{
	"dim":        func(s string) string { return fancy.AsDim(s) },
	"bold":       func(s string) string { return fancy.AsBold(s) },
	"underlined": func(s string) string { return fancy.AsUnderlined(s) },
	"blue":       func(s string) string { return fancy.AsBlue(s) },
	"plain":      func() string { return fancy.Plain }, // all styled lines _should_ be terminated with plain
	"clear":      func() string { return fancy.Clear },
})

// askForTemplate generates an introduction the setting of a (free-text) field
var askForTemplate = template.Must(fancyTemplate.New("askFor").Parse(
	"{{clear}}\r" +
		"{{ .Preamble}}\n\r" +
		"{{ if .Examples }}" +
		"Examples:\n\r" +
		"{{range .Examples}}" +
		" * {{if .Name}}{{.Name | underlined}}: {{end}}{{.Text | bold }}{{plain}}\n\r" +
		"{{end}}" + // end range
		"{{end}}" + // end Examples check
		"\n\r",
))

var serverSettingsTemplate = template.Must(fancyTemplate.New("serverSetting").Parse(
	"{{clear}}\r" +
		"Name:       {{.ServerName | bold}}\n\r" +
		"Host Path:  {{.HostPath | bold}}\n\r" +
		"Access Key: {{.AccessKey | bold}}\n\r" +
		"Secret key: {{.SecretKey | bold}}\n\r",
))

// ***** template structures *****

// AskForTemplateFields is used with the askForTempate template
type AskForTemplateFields struct {
	WithPreamble bool
	Preamble     string
	Examples     []NamedExample
	Prompt       string
	// DoValidation bool
	// ValidateAnswer func() bool
}

type AskForTemplateModifiers struct {
	WithPreamble *bool
}

// NamedExample represents a single example text, and an optional name
type NamedExample struct {
	Name string
	Text string
}

// ***** Predefined Values & Generators *****

// AskForNoPreamble returns an AskForTemplateFields modified to not generate the preamble or examples
func AskForNoPreamble(base AskForTemplateFields) AskForTemplateFields {
	base.WithPreamble = false
	return base
}

// edit alters the given template with the provided mods. All non-nil mods will be applied to
// the template
func edit(base AskForTemplateFields, mods AskForTemplateModifiers) AskForTemplateFields {
	if mods.WithPreamble != nil {
		base.WithPreamble = *mods.WithPreamble
	}
	return base
}

var savePathFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "Where shall I save the recordings? This can be anywhere on your computer but typically resides within the home directory.",
	Examples: []NamedExample{
		NamedExample{Name: "Mac", Text: "/Users/jsmith/ashirt/recordings"},
		NamedExample{Name: "Linux", Text: "/home/jsmith/ashirt/recordings"},
		NamedExample{Name: "System Recommendation", Text: defaultRecordingHome},
	},
	Prompt: "Enter the save path",
}

var shellFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "Which shell should I use to create the recordings? This should be the absolute path to shell application.",
	Examples: []NamedExample{
		NamedExample{Name: "Mac Bash", Text: "/bin/bash"},
		NamedExample{Name: "Mac Zsh", Text: "/bin/zsh"},
		NamedExample{Name: "Linux Bash", Text: "/usr/bin/bash"},
	},
	Prompt: "Enter the path to the shell",
}

var accessKeyFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "An Access key is a short string of random letters.",
	Examples: []NamedExample{
		NamedExample{Text: "aiH6Y7z8IV_6KymbMip8b47U"},
	},
	Prompt: "Enter the Access Key",
}

var secretKeyFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "A Secret key is a long base-64 string (Only letters, numbers, +, /, and = signs).",
	Examples: []NamedExample{
		NamedExample{Text: "V42yvFX/b+zuh5Lqk8ZJId/OwIjL3dt88W0q/8E/nF4KZOBj4OTyI31FWMUi28RhkWcW4rC/a2Tb6AOAem1ouw=="},
	},
	Prompt: "Enter the Secret Key",
}

var apiURLFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "Where are the ASHIRT servers located? If you don't know, please contact your administrator.",
	Prompt:       "Enter the API URL",
}

var serverNameFields = AskForTemplateFields{
	WithPreamble: true,
	Preamble:     "What is the name of the server? This can be any value, and is here to help you identify the server later.",
	Prompt:       "Enter the server name",
}

var upgradeNoticeTemplate = template.Must(fancyTemplate.New("upgradeNotice").Parse(
	"{{clear}}The latest {{.ReleaseType}} release ({{.Version | bold}}) can be found here: {{.URL | underlined}}" +
		"\n\r",
))

type UpgradeNoticeTemplateFields struct {
	ReleaseType string
	Version     string
	URL         string
}

func NewUpgrade(releaseType, tag, url string) UpgradeNoticeTemplateFields {
	return UpgradeNoticeTemplateFields{ReleaseType: releaseType, Version: tag, URL: url}
}
