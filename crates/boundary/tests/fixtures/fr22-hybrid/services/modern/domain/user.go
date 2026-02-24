package domain

// User is a pure DDD domain entity with no infrastructure imports.
type User struct {
	ID   string
	Name string
}

// UserRepository is a domain port.
type UserRepository interface {
	FindByID(id string) (*User, error)
}
