package secrets

import (
	"bufio"
	"errors"
	"fmt"
	"os"
	"strings"
)

// fromDotEnv remembers which variables LoadDotEnv set, so that `secret status`
// can say that a key comes from a development file.
var fromDotEnv = map[string]dotEnvValue{}

type dotEnvValue struct{ file, value string }

// LoadDotEnv reads KEY=VALUE lines from a .env file into the process environment.
// It is a convenience for development: the file has to be git-ignored, and anything
// that is already set in the real environment wins, so a deployment that configures
// secrets properly is never affected by a stray file. A missing file is not an error.
//
// Supported: blank lines, # comments, an optional "export " prefix, values in single
// or double quotes. There is no variable interpolation.
func LoadDotEnv(path string) ([]string, error) {
	f, err := os.Open(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var set []string
	scanner := bufio.NewScanner(f)
	for line := 1; scanner.Scan(); line++ {
		text := strings.TrimSpace(scanner.Text())
		if text == "" || strings.HasPrefix(text, "#") {
			continue
		}
		text = strings.TrimSpace(strings.TrimPrefix(text, "export "))
		name, value, ok := strings.Cut(text, "=")
		name = strings.TrimSpace(name)
		if !ok || name == "" || strings.ContainsAny(name, " \t\"'") {
			return set, fmt.Errorf("%s:%d: expected NAME=value", path, line)
		}
		value = strings.TrimSpace(value)
		if len(value) >= 2 && (value[0] == '"' || value[0] == '\'') && value[len(value)-1] == value[0] {
			value = value[1 : len(value)-1]
		} else if i := strings.Index(value, " #"); i >= 0 {
			value = strings.TrimSpace(value[:i]) // trailing comment of an unquoted value
		}
		if _, exists := os.LookupEnv(name); exists {
			continue
		}
		if err := os.Setenv(name, value); err != nil {
			return set, err
		}
		fromDotEnv[name] = dotEnvValue{file: path, value: value}
		set = append(set, name)
	}
	return set, scanner.Err()
}
