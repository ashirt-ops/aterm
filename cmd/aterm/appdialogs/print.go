package appdialogs

import (
	"fmt"
	"os"
	"strings"
)

var medium = os.Stdout

// printline avoid using println, but does the same as fmt.Println
func printline(s ...string) {
	fmt.Println(strings.Join(s, " ") + "\r")
}

func sprintf(s string, vals ...interface{}) string {
	return fmt.Sprintf(s, vals...)
}

func printf(s string, vals ...interface{}) {
	fmt.Printf(s+"\r", vals...)
}

func printfln(s string, vals ...interface{}) {
	fmt.Println(sprintf(s, vals...) + "\n\r")
}
