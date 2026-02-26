package cli

import (
	"fmt"
	"runtime"

	"github.com/spf13/cobra"
)

// Build-time variables, set via ldflags:
//
//	go build -ldflags "-X github.com/wvandaal/ddis/internal/cli.Version=v1.0.0
//	  -X github.com/wvandaal/ddis/internal/cli.Commit=abc1234
//	  -X github.com/wvandaal/ddis/internal/cli.Date=2026-02-26"
var (
	Version = "dev"
	Commit  = "unknown"
	Date    = "unknown"
)

var versionCmd = &cobra.Command{
	Use:   "version",
	Short: "Print build version information",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Printf("ddis %s (%s) built %s %s/%s\n", Version, Commit, Date, runtime.GOOS, runtime.GOARCH)
	},
}
