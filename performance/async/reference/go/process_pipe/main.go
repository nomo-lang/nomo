package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
)

const (
	exchanges = 256
	payload   = "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ\n"
)

func fail(message string, arguments ...any) {
	fmt.Fprintf(os.Stderr, message+"\n", arguments...)
	os.Exit(1)
}

func main() {
	program := os.Getenv("NOMO_PROCESS_FIXTURE")
	if program == "" {
		fail("missing controlled process fixture")
	}

	command := exec.Command(program, "pipe")
	stdin, err := command.StdinPipe()
	if err != nil {
		fail("stdin pipe: %v", err)
	}
	stdout, err := command.StdoutPipe()
	if err != nil {
		fail("stdout pipe: %v", err)
	}
	stderr, err := command.StderrPipe()
	if err != nil {
		fail("stderr pipe: %v", err)
	}
	if err := command.Start(); err != nil {
		fail("start: %v", err)
	}

	stdoutReader := bufio.NewReader(stdout)
	stderrReader := bufio.NewReader(stderr)
	for index := 0; index < exchanges; index++ {
		if _, err := stdin.Write([]byte(payload)); err != nil {
			fail("write exchange %d: %v", index, err)
		}
		out, err := stdoutReader.ReadString('\n')
		if err != nil {
			fail("stdout exchange %d: %v", index, err)
		}
		errOut, err := stderrReader.ReadString('\n')
		if err != nil {
			fail("stderr exchange %d: %v", index, err)
		}
		if out != "O:"+payload || errOut != "E:"+payload {
			fail("payload mismatch at exchange %d", index)
		}
	}
	if err := stdin.Close(); err != nil {
		fail("close stdin: %v", err)
	}
	if err := command.Wait(); err != nil {
		fail("wait: %v", err)
	}

	fmt.Printf("process-pipe %d %d\n", exchanges, len(payload))
}
