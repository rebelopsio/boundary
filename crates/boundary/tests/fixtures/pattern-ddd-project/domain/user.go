package domain

// UserRepository is a port (abstract).
type UserRepository interface {
	FindByID(id string) (*User, error)
}

// User is a domain entity (concrete).
type User struct {
	ID   string
	Name string
}
