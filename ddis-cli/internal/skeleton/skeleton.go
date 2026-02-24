package skeleton

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"text/template"
)

// Options controls skeleton generation.
type Options struct {
	Name    string
	Domains []string
	Output  string
}

// GenerateResult describes the generated skeleton.
type GenerateResult struct {
	OutputDir  string
	Files      []FileInfo
	TotalLines int
}

// FileInfo describes a single generated file.
type FileInfo struct {
	Path  string
	Lines int
}

// Generate creates a DDIS-conformant specification skeleton.
func Generate(opts Options) (*GenerateResult, error) {
	outDir := opts.Output
	if err := os.MkdirAll(outDir, 0755); err != nil {
		return nil, fmt.Errorf("create output dir: %w", err)
	}

	var files []FileInfo
	totalLines := 0

	// 1. Generate manifest.yaml
	manifestPath := filepath.Join(outDir, "manifest.yaml")
	lines, err := writeTemplate(manifestPath, manifestTmpl, opts)
	if err != nil {
		return nil, fmt.Errorf("write manifest: %w", err)
	}
	files = append(files, FileInfo{Path: "manifest.yaml", Lines: lines})
	totalLines += lines

	// 2. Generate constitution/system.md
	constDir := filepath.Join(outDir, "constitution")
	if err := os.MkdirAll(constDir, 0755); err != nil {
		return nil, fmt.Errorf("create constitution dir: %w", err)
	}
	constPath := filepath.Join(constDir, "system.md")
	lines, err = writeTemplate(constPath, constitutionTmpl, opts)
	if err != nil {
		return nil, fmt.Errorf("write constitution: %w", err)
	}
	files = append(files, FileInfo{Path: "constitution/system.md", Lines: lines})
	totalLines += lines

	// 3. Generate module files
	modDir := filepath.Join(outDir, "modules")
	if err := os.MkdirAll(modDir, 0755); err != nil {
		return nil, fmt.Errorf("create modules dir: %w", err)
	}
	for i, domain := range opts.Domains {
		modPath := filepath.Join(modDir, domain+".md")
		data := struct {
			Options
			Domain     string
			DomainIdx  int
			DomainUpper string
		}{opts, domain, i + 1, strings.ToUpper(domain[:1]) + domain[1:]}
		lines, err = writeTemplate(modPath, moduleTmpl, data)
		if err != nil {
			return nil, fmt.Errorf("write module %s: %w", domain, err)
		}
		files = append(files, FileInfo{Path: "modules/" + domain + ".md", Lines: lines})
		totalLines += lines
	}

	return &GenerateResult{OutputDir: outDir, Files: files, TotalLines: totalLines}, nil
}

func writeTemplate(path string, tmplStr string, data interface{}) (int, error) {
	funcMap := template.FuncMap{
		"upper": func(s string) string {
			if s == "" {
				return s
			}
			return strings.ToUpper(s[:1]) + s[1:]
		},
		"toUpper": strings.ToUpper,
		"add": func(a, b int) int {
			return a + b
		},
	}

	tmpl, err := template.New("").Funcs(funcMap).Parse(tmplStr)
	if err != nil {
		return 0, err
	}
	f, err := os.Create(path)
	if err != nil {
		return 0, err
	}
	defer f.Close()
	var buf strings.Builder
	if err := tmpl.Execute(&buf, data); err != nil {
		return 0, err
	}
	content := buf.String()
	if _, err := f.WriteString(content); err != nil {
		return 0, err
	}
	return strings.Count(content, "\n") + 1, nil
}
