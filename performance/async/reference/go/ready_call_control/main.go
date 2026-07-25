package main

import "fmt"

//go:noinline
func step(value int64) int64 {
	return value + 1
}

func runIterations() int64 {
	var total int64
	for i := uint64(0); i < 100000; i++ {
		total = step(total)
	}
	return total
}

func main() {
	fmt.Println("p0-ready-control", runIterations())
}
