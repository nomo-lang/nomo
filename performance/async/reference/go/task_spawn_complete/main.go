package main

import (
	"fmt"
	"sync"
)

func main() {
	var completed sync.WaitGroup
	completed.Add(32)
	for range 32 {
		go completed.Done()
	}
	completed.Wait()
	fmt.Println("spawn-complete", 32)
}
