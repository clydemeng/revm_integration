package main

/*
#cgo LDFLAGS: -L../rust_lib/target/release -lrust_lib
#include <stdint.h>

// Forward declaration of the Go function to be used as a callback
extern int goCallback(int value);

// Forward declaration of the Rust function
extern int process_data_in_rust(void* callback, int value);
*/
import "C"
import (
	"fmt"
	"unsafe"
)

//export goCallback
func goCallback(value C.int) C.int {
	fmt.Printf("[Go] Received value in callback: %d\n", value)
	return value * 2
}

func main() {
	fmt.Println("[Go] Starting application...")
	value := 10

	// Call the Rust function with the Go callback
	fmt.Println("[Go] Calling Rust function...")
	result := C.process_data_in_rust(unsafe.Pointer(C.goCallback), C.int(value))

	fmt.Printf("[Go] Received final result from Rust: %d\n", result)
	fmt.Println("[Go] Application finished.")
} 