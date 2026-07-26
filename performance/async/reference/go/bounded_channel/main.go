package main

import "fmt"

func main() {
	channel := make(chan uint64, 8)
	done := make(chan struct{}, 2)

	go func() {
		for value := uint64(0); value < 32; value++ {
			channel <- value
		}
		done <- struct{}{}
	}()
	go func() {
		for index := 0; index < 32; index++ {
			<-channel
		}
		done <- struct{}{}
	}()

	<-done
	<-done
	close(channel)
	fmt.Println("bounded-channel 32")
}
