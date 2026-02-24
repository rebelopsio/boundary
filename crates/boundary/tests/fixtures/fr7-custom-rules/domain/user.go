package domain

import (
	"example.com/app/external"
)

// User is a domain entity — but it wrongly imports from the external package.
type User struct {
	ID   string
	Name string
}

// GetClient is a contrived method that uses the external package.
func GetClient() external.Client {
	return external.Client{}
}
