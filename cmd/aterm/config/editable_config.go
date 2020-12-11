package config

type EditableConfig struct {
	RecordingShell string
	OutputDir      string
}

func (c EditableConfig) Serialize() []byte {
	return []byte{}
}

func (c EditableConfig) GetConfigVersion() int64 {
	return -1
}

func (c EditableConfig) GetRecordingShell() string {
	return c.RecordingShell
}

func (c EditableConfig) GetOutputDir() string {
	return c.OutputDir
}

func (c EditableConfig) GetHostPath() string {
	return ""
}

func (c EditableConfig) GetAccessKey() string {
	return ""
}

func (c EditableConfig) GetSecretKey() string {
	return ""
}

func (c EditableConfig) PreviewConfigUpdates(changes EditableConfig) {
}
