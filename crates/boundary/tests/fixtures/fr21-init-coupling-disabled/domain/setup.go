package domain

import "github.com/example/fr21/infrastructure"

// User is a domain entity.
type User struct {
	ID   string
	Name string
}

func init() {
	// Hidden cross-layer coupling: domain init() reaches into infrastructure.
	infrastructure.Register("user-service")
}
