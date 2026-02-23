package domain

// User is the core domain entity.
type User struct {
	ID   string
	Name string
}

// UserRepository is a domain port.
type UserRepository interface {
	FindByID(id string) (*User, error)
}
